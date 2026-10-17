import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import { App } from "../app";
import { startDaemon, type FakeDaemon } from "../fake-daemon/daemon";
import { frameOf, sessionInfo, tabInfo } from "../fake-daemon/messages";
import { controlButtonId } from "../lib/control";
import { navigate } from "../lib/route";
import type { TabAddress } from "../lib/tabs";

const shownApp = async (path = "/") => {
  const daemon = startDaemon([
    sessionInfo("work", [tabInfo("app", { program: "vim", command: "vim notes" }), tabInfo("api", { program: "make", command: "make all" })]),
    sessionInfo("play", [tabInfo("app", { program: "htop", command: "htop" })]),
  ]);

  navigate(path);
  render(<App />);
  await daemon.serve();

  return daemon;
};

const layout = (label: string) => {
  fireEvent.click(screen.getByRole("button", { name: label }));
};

const commandLine = () =>
  within(screen.getByRole("group", { name: "Command line" }))
    .getAllByText(/./, { ignore: ".sr-only" })
    .map((part) => part.textContent)
    .join("");

const grid = () => screen.getByRole("group", { name: "Terminals" });

const sections = () => within(grid()).getAllByRole("region");

const cells = () => sections().map((cell) => cell.getAttribute("aria-label") ?? cell.textContent);

const region = (name: string) => screen.getByRole("region", { name });

const toggleIn = (cell: HTMLElement) => within(cell).getAllByRole("button").find((button) => button.hasAttribute("aria-pressed"));

const header = (name: string) => toggleIn(region(name));

const separators = () => within(screen.getByRole("main")).getAllByTestId("divider").length;

const ringed = (name: string, ring: string) => within(region(name)).getByRole("group", { name: new RegExp(`^${name}`) }).matches(`${ring} > *`);

const pressedLayout = () => within(screen.getByRole("group", { name: "Layout" })).getByRole("button", { pressed: true }).textContent;

test("splitting a tab fills the grid from it, and the focused cell owns the toolbar and details", async () => {
  await shownApp();
  layout("Two terminals side by side");
  expect(location.pathname).toBe("/split/work:app,work:api");
  expect([document.title, screen.getByRole("heading", { level: 1 }).textContent]).toEqual(["Split · 2 terminals · agent-terminal", "Split view, 2 terminals"]);
  expect(cells()).toEqual(["work app vim", "work api make"]);
  expect(commandLine()).toBe("vim notes/home/me");
  fireEvent.click(screen.getByRole("region", { name: "work api make" }));
  expect(commandLine()).toBe("make all/home/me");
  expect(screen.getByRole("region", { name: "Details of work api" }).getAttribute("aria-label")).toBe("Details of work api");
  layout("Four terminals");
  expect(location.pathname).toBe("/split/work:app,work:api,_,_");
  expect(cells()).toEqual(["work app vim", "work api make", "Show a terminalworkappvimshown, work appapimakeshown, work apiplayapphtop, play app", "Show a terminalworkappvimshown, work appapimakeshown, work apiplayapphtop, play app"]);
  layout("Single terminal");
  expect(location.pathname).toBe("/s/work/api");
});

test("a cell can be removed, refilled from the picker and maximized", async () => {
  await shownApp("/split/work:app,work:api");
  fireEvent.click(region("work api make"));
  fireEvent.click(screen.getByRole("button", { name: "Remove work app from the grid" }));
  expect(location.pathname).toBe("/split/_,work:api");
  expect(screen.getByRole("main")).toHaveFocus();
  const picker = screen.getByRole("region", { name: "Show a terminal" });

  expect(within(picker).getByRole<HTMLButtonElement>("button", { name: /work api/ }).disabled).toBe(true);
  fireEvent.click(within(picker).getByRole("button", { name: /play app/ }));
  expect(location.pathname).toBe("/split/play:app,work:api");
  expect(header("play app htop")).toHaveFocus();
  expect(header("play app htop")?.getAttribute("aria-pressed")).toBe("true");
  expect(fireEvent.click(screen.getByRole("link", { name: "Maximize work api" }))).toBe(false);
  expect([location.pathname, document.title]).toEqual(["/s/work/api", "work api · make · agent-terminal"]);
  expect(screen.getByRole("main")).toHaveFocus();
});

const cutOff = (part: HTMLElement) => [part.textContent, part.dataset["tooltip"], part.hasAttribute("data-tooltip-overflow"), part.classList.contains("truncate")];

test("a cell header keeps its session, tab and program on one line and the picker each program, each in full on hover when cut off", async () => {
  await shownApp("/split/work:app,_");
  const cell = header("work app vim");
  const picker = region("Show a terminal");
  const names = ["work", "app", "vim"].map((name) => within(region("work app vim")).getByText(name));

  expect([[cell, picker].map((part) => /break-|flex-wrap/.test(part?.outerHTML ?? "")), [...names, within(picker).getByText("htop")].map(cutOff)]).toEqual([
    [false, false],
    [
      ["work", "work", true, true],
      ["app", "app", true, true],
      ["vim", "vim", true, true],
      ["htop", "htop", true, true],
    ],
  ]);
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
  expect(location.pathname).toBe("/s/work/app");
});

test("shrinking the grid past the focused cell moves focus to the first, and the sidebar fills the focused cell", async () => {
  await shownApp("/split/work:app,work:api,play:app,_");
  fireEvent.click(screen.getByRole("region", { name: "play app htop" }));
  expect(commandLine()).toBe("htop/home/me");
  layout("Two terminals side by side");
  expect([location.pathname, commandLine()]).toEqual(["/split/work:app,work:api", "vim notes/home/me"]);
  await act(async () => {
    fireEvent.click(within(screen.getByRole("navigation", { name: "Sessions and tabs" })).getByRole("link", { name: /htop/ }));
    await Promise.resolve();
  });
  expect(location.pathname).toBe("/split/play:app,work:api");
});

test("splitting again from a single terminal focuses its cell, whatever was focused before", async () => {
  await shownApp("/split/work:app,work:api");
  fireEvent.click(screen.getByRole("region", { name: "work api make" }));
  layout("Single terminal");
  expect(location.pathname).toBe("/s/work/api");
  await act(async () => {
    navigate("/s/work/app");
    await Promise.resolve();
  });
  layout("Two terminals side by side");
  expect([location.pathname, commandLine()]).toEqual(["/split/work:app,work:api", "vim notes/home/me"]);
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
  const daemon = await shownApp("/split/work:app,work:api");
  const looks = () => sections().map((cell) => [cell.className, toggleIn(cell)?.getAttribute("aria-pressed")]);

  await drawn(daemon, streamOf("work", "app"));
  expect([grid().className, separators()]).toEqual(["split-grid grid min-h-0 flex-1 grid-cols-2 gap-px bg-border grid-rows-1", 2]);
  expect(looks()).toEqual([[FOCUSED, "true"], [PLAIN, "false"]]);
  expect(await screen.findByRole("group", { name: "work app vim, live frame 3" })).toBeTruthy();
  fireEvent.click(region("work api make"));
  expect(looks()).toEqual([[PLAIN, "false"], [FOCUSED, "true"]]);
  expect([screen.getByRole("link", { name: "Maximize work app" }).dataset["tooltip"], screen.getByRole("button", { name: "Remove work app from the grid" }).dataset["tooltip"]]).toEqual(["Maximize", "Remove"]);
  layout("Four terminals");
  expect(grid().className).toBe("split-grid grid min-h-0 flex-1 grid-cols-2 gap-px bg-border grid-rows-2");
});

const held = async (daemon: FakeDaemon, address: TabAddress) => {
  const path = controlOf(address.session, address.tab);
  const buttons = await screen.findAllByRole("button", { name: "Take control" });
  const [own] = buttons.filter((button) => button.id === controlButtonId(address.session, address.tab));

  expect(own).toBeDefined();
  fireEvent.click(own ?? buttons[0] ?? document.body);
  await daemon.connected(path);
  await daemon.push(path, { type: "held" });
};

test("a held cell takes keys only while it is focused and its program runs, and keeps its ring when it is not", async () => {
  const daemon = await shownApp("/split/work:app,work:api");

  await drawn(daemon, streamOf("work", "app"));
  await drawn(daemon, streamOf("work", "api"));
  await held(daemon, { session: "work", tab: "app" });
  fireEvent.keyDown(screen.getByRole("application", { name: "Terminal work app, you have control" }), { key: "a" });
  expect(daemon.sent(controlOf("work", "app"))).toEqual([{ focused: true }, { typed: "a" }]);
  fireEvent.click(region("work api make"));
  expect([screen.queryByRole("application"), ringed("work api make", ".ring-warning")]).toEqual([null, false]);
  expect(ringed("work app vim", String.raw`.rounded-\[2px\].ring-2.ring-warning.ring-offset-2.ring-offset-background`)).toBe(true);
  fireEvent.click(region("work app vim"));
  await daemon.push(streamOf("work", "app"), frameOf(4, { exited: true }));
  await within(region("work app vim")).findByRole("status");
  expect(within(region("work app vim")).getByTestId("viewport-bottom").className.split(" ").at(-1)).toBe("bottom-2");
});

test("a cell that switches to another tab sends its keys to that tab", async () => {
  const daemon = await shownApp("/split/work:app,work:api");

  await act(async () => {
    fireEvent.click(within(screen.getByRole("navigation", { name: "Sessions and tabs" })).getByRole("link", { name: /htop/ }));
    await Promise.resolve();
  });
  await drawn(daemon, streamOf("play", "app"));
  await held(daemon, { session: "play", tab: "app" });
  fireEvent.keyDown(screen.getByRole("application", { name: "Terminal play app, you have control" }), { key: "b" });
  expect(daemon.sent(controlOf("play", "app"))).toEqual([{ focused: true }, { typed: "b" }]);
});

test("removing a cell keeps the focused cell and focuses the main content without scrolling", async () => {
  await shownApp("/split/work:app,work:api,play:app,_");
  fireEvent.click(region("work api make"));
  const focus = vi.spyOn(HTMLElement.prototype, "focus");

  fireEvent.click(screen.getByRole("button", { name: "Remove play app from the grid" }));
  expect([location.pathname, commandLine(), document.activeElement?.id, focus.mock.calls]).toEqual(["/split/work:app,work:api,_,_", "make all/home/me", "main-content", [[{ preventScroll: true }]]]);
});

test("a grid that shrank past its focused cell still focuses the first cell when it grows back", async () => {
  await shownApp("/split/work:app,work:api,play:app,_");
  fireEvent.click(region("play app htop"));
  layout("Two terminals side by side");
  await act(async () => {
    navigate("/split/work:app,work:api,play:app,_");
    await Promise.resolve();
  });
  expect(commandLine()).toBe("vim notes/home/me");
});
