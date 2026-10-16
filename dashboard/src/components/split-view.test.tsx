import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import { App } from "../app";
import { startDaemon, type FakeDaemon } from "../fake-daemon/daemon";
import { frameOf, sessionInfo, tabInfo } from "../fake-daemon/messages";
import { navigate } from "../lib/route";

const shownApp = async (path = "/") => {
  const daemon = startDaemon([
    sessionInfo("work", [tabInfo("t1", { program: "vim", command: "vim notes" }), tabInfo("t2", { program: "make", command: "make all" })]),
    sessionInfo("play", [tabInfo("t1", { program: "htop", command: "htop" })]),
  ]);

  navigate(path);
  render(<App />);
  await daemon.serve();

  return daemon;
};

const layout = (label: string) => {
  fireEvent.click(screen.getByRole("button", { name: label }));
};

const commandLine = () => screen.getByRole("group", { name: "Command line" }).textContent;

const sections = () => Array.from(document.querySelectorAll<HTMLElement>(".split-grid > section"));

const cells = () => sections().map((cell) => cell.getAttribute("aria-label") ?? cell.textContent);

const region = (name: string) => screen.getByRole("region", { name });

const header = (name: string) => region(name).querySelector("[aria-pressed]");

const separators = () => document.querySelectorAll('#main-content > [role="none"]').length;

const pressedLayout = () => within(screen.getByRole("group", { name: "Layout" })).getByRole("button", { pressed: true }).textContent;

test("splitting a tab fills the grid from it, and the focused cell owns the toolbar and details", async () => {
  await shownApp();
  layout("Two terminals side by side");
  expect(location.pathname).toBe("/split/work:t1,work:t2");
  expect([document.title, screen.getByRole("heading", { level: 1 }).textContent]).toEqual(["Split · 2 terminals · agent-terminal", "Split view, 2 terminals"]);
  expect(cells()).toEqual(["work t1 vim", "work t2 make"]);
  expect(commandLine()).toBe("vim notes/home/me");
  fireEvent.click(screen.getByRole("region", { name: "work t2 make" }));
  expect(commandLine()).toBe("make all/home/me");
  expect(screen.getByRole("region", { name: "Details of work t2" }).getAttribute("aria-label")).toBe("Details of work t2");
  layout("Four terminals");
  expect(location.pathname).toBe("/split/work:t1,work:t2,_,_");
  expect(cells()).toEqual(["work t1 vim", "work t2 make", "Show a terminalworkt1vimshown, work t1t2makeshown, work t2playt1htop, play t1", "Show a terminalworkt1vimshown, work t1t2makeshown, work t2playt1htop, play t1"]);
  layout("Single terminal");
  expect(location.pathname).toBe("/s/work/t2");
});

test("a cell can be removed, refilled from the picker and maximized", async () => {
  await shownApp("/split/work:t1,work:t2");
  fireEvent.click(region("work t2 make"));
  fireEvent.click(screen.getByRole("button", { name: "Remove work t1 from the grid" }));
  expect([location.pathname, document.activeElement?.id]).toEqual(["/split/_,work:t2", "main-content"]);
  const picker = screen.getByRole("region", { name: "Show a terminal" });

  expect(within(picker).getByRole<HTMLButtonElement>("button", { name: /work t2/ }).disabled).toBe(true);
  fireEvent.click(within(picker).getByRole("button", { name: /play t1/ }));
  expect(location.pathname).toBe("/split/play:t1,work:t2");
  expect([document.activeElement, header("play t1 htop")?.getAttribute("aria-pressed")]).toEqual([header("play t1 htop"), "true"]);
  expect(fireEvent.click(screen.getByRole("link", { name: "Maximize work t2" }))).toBe(false);
  expect([location.pathname, document.title, document.activeElement?.id]).toEqual(["/s/work/t2", "work t2 · make · agent-terminal", "main-content"]);
});

test("a grid with nothing focused offers only the layout switch, and closing it goes home", async () => {
  await shownApp("/split/_,_,_,_");
  expect([separators(), pressedLayout()]).toEqual([1, "Four terminals"]);
  layout("Two terminals side by side");
  expect([screen.queryByRole("group", { name: "Command line" }), pressedLayout()]).toEqual([null, "Two terminals side by side"]);
  expect(screen.getAllByRole("region", { name: "Show a terminal" })).toHaveLength(2);
  layout("Two terminals side by side");
  expect(location.pathname).toBe("/split/_,_");
  layout("Single terminal");
  expect(location.pathname).toBe("/s/work/t1");
});

test("shrinking the grid past the focused cell moves focus to the first, and the sidebar fills the focused cell", async () => {
  await shownApp("/split/work:t1,work:t2,play:t1,_");
  fireEvent.click(screen.getByRole("region", { name: "play t1 htop" }));
  expect(commandLine()).toBe("htop/home/me");
  layout("Two terminals side by side");
  expect([location.pathname, commandLine()]).toEqual(["/split/work:t1,work:t2", "vim notes/home/me"]);
  await act(async () => {
    fireEvent.click(within(screen.getByRole("navigation", { name: "Sessions and tabs" })).getByRole("link", { name: /htop/ }));
    await Promise.resolve();
  });
  expect(location.pathname).toBe("/split/play:t1,work:t2");
});

test("splitting again from a single terminal focuses its cell, whatever was focused before", async () => {
  await shownApp("/split/work:t1,work:t2");
  fireEvent.click(screen.getByRole("region", { name: "work t2 make" }));
  layout("Single terminal");
  expect(location.pathname).toBe("/s/work/t2");
  await act(async () => {
    navigate("/s/work/t1");
    await Promise.resolve();
  });
  layout("Two terminals side by side");
  expect([location.pathname, commandLine()]).toEqual(["/split/work:t1,work:t2", "vim notes/home/me"]);
});

const PLAIN = "flex min-h-0 min-w-0 flex-col bg-background";

const FOCUSED = `${PLAIN} ring-1 ring-foreground/40 ring-inset`;

const streamOf = (session: string, tab: string) => `/api/session/${session}/${tab}/stream`;

const controlOf = (session: string, tab: string) => `/api/session/${session}/${tab}/control`;

const drawn = async (daemon: FakeDaemon, path: string) => {
  await daemon.connected(path);
  await daemon.push(path, frameOf(3));
};

test("the grid lays out two or four cells, and only the focused cell is pressed and ringed", async () => {
  const daemon = await shownApp("/split/work:t1,work:t2");
  const looks = () => sections().map((cell) => [cell.className, cell.querySelector("[aria-pressed]")?.getAttribute("aria-pressed")]);

  await drawn(daemon, streamOf("work", "t1"));
  expect([document.querySelector(".split-grid")?.className, separators()]).toEqual(["split-grid grid min-h-0 flex-1 grid-cols-2 gap-px bg-border grid-rows-1", 2]);
  expect(looks()).toEqual([[FOCUSED, "true"], [PLAIN, "false"]]);
  expect(await screen.findByRole("group", { name: "work t1 vim, live frame 3" })).toBeTruthy();
  fireEvent.click(region("work t2 make"));
  expect(looks()).toEqual([[PLAIN, "false"], [FOCUSED, "true"]]);
  expect([screen.getByRole("link", { name: "Maximize work t1" }).dataset["tooltip"], screen.getByRole("button", { name: "Remove work t1 from the grid" }).dataset["tooltip"]]).toEqual(["Maximize", "Remove"]);
  layout("Four terminals");
  expect(document.querySelector(".split-grid")?.className).toBe("split-grid grid min-h-0 flex-1 grid-cols-2 gap-px bg-border grid-rows-2");
});

const held = async (daemon: FakeDaemon, path: string) => {
  fireEvent.click(await screen.findByRole("button", { name: "Take control" }));
  await daemon.connected(path);
  await daemon.push(path, { type: "held" });
};

const hint = () => screen.queryByText(/Keys go to/)?.textContent;

test("a held cell takes keys only while it is focused and its program runs, and keeps its ring when it is not", async () => {
  const daemon = await shownApp("/split/work:t1,work:t2");

  await drawn(daemon, streamOf("work", "t1"));
  await drawn(daemon, streamOf("work", "t2"));
  await held(daemon, controlOf("work", "t1"));
  fireEvent.keyDown(screen.getByRole("application", { name: "Terminal work t1, you have control" }), { key: "a" });
  expect([daemon.sent(controlOf("work", "t1")), hint()]).toEqual([[{ typed: "a" }], "Keys go to vim · ctrl+p ctrl+q moves focus out"]);
  fireEvent.click(region("work t2 make"));
  expect([screen.queryByRole("application"), hint(), region("work t2 make").querySelector(".ring-warning")]).toEqual([null, undefined, null]);
  expect(region("work t1 vim").querySelector(".ring-warning")?.className).toBe("rounded-[2px] ring-2 ring-warning ring-offset-2 ring-offset-background");
  fireEvent.click(region("work t1 vim"));
  await daemon.push(streamOf("work", "t1"), frameOf(4, { exited: true }));
  expect((await within(region("work t1 vim")).findByRole("status")).className.split(" ").at(-1)).toBe("bottom-2");
  expect(hint()).toBeUndefined();
});

test("a cell that switches to another tab sends its keys to that tab", async () => {
  const daemon = await shownApp("/split/work:t1,work:t2");

  await act(async () => {
    fireEvent.click(within(screen.getByRole("navigation", { name: "Sessions and tabs" })).getByRole("link", { name: /htop/ }));
    await Promise.resolve();
  });
  await drawn(daemon, streamOf("play", "t1"));
  await held(daemon, controlOf("play", "t1"));
  fireEvent.keyDown(screen.getByRole("application", { name: "Terminal play t1, you have control" }), { key: "b" });
  expect(daemon.sent(controlOf("play", "t1"))).toEqual([{ typed: "b" }]);
});

test("removing a cell keeps the focused cell and focuses the main content without scrolling", async () => {
  await shownApp("/split/work:t1,work:t2,play:t1,_");
  fireEvent.click(region("work t2 make"));
  const focus = vi.spyOn(HTMLElement.prototype, "focus");

  fireEvent.click(screen.getByRole("button", { name: "Remove play t1 from the grid" }));
  expect([location.pathname, commandLine(), document.activeElement?.id, focus.mock.calls]).toEqual(["/split/work:t1,work:t2,_,_", "make all/home/me", "main-content", [[{ preventScroll: true }]]]);
});

test("a grid that shrank past its focused cell still focuses the first cell when it grows back", async () => {
  await shownApp("/split/work:t1,work:t2,play:t1,_");
  fireEvent.click(region("play t1 htop"));
  layout("Two terminals side by side");
  await act(async () => {
    navigate("/split/work:t1,work:t2,play:t1,_");
    await Promise.resolve();
  });
  expect(commandLine()).toBe("vim notes/home/me");
});
