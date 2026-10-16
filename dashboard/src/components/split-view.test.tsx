import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { expect, test } from "vitest";

import { App } from "../app";
import { startDaemon } from "../fake-daemon/daemon";
import { sessionInfo, tabInfo } from "../fake-daemon/messages";
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

const cells = () => Array.from(document.querySelectorAll(".split-grid > section")).map((cell) => cell.getAttribute("aria-label") ?? cell.textContent);

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
  fireEvent.click(screen.getByRole("button", { name: "Remove work t1 from the grid" }));
  expect([location.pathname, document.activeElement?.id]).toEqual(["/split/_,work:t2", "main-content"]);
  const picker = screen.getByRole("region", { name: "Show a terminal" });

  expect(within(picker).getByRole<HTMLButtonElement>("button", { name: /work t2/ }).disabled).toBe(true);
  fireEvent.click(within(picker).getByRole("button", { name: /play t1/ }));
  expect(location.pathname).toBe("/split/play:t1,work:t2");
  expect(document.activeElement?.textContent).toContain("play");
  fireEvent.click(screen.getByRole("link", { name: "Maximize work t2" }));
  expect(location.pathname).toBe("/s/work/t2");
});

test("a grid with nothing focused offers only the layout switch, and closing it goes home", async () => {
  await shownApp("/split/_,_");
  expect(screen.queryByRole("group", { name: "Command line" })).toBeNull();
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
