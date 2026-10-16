import { fireEvent, render, screen, within } from "@testing-library/react";
import { expect, test } from "vitest";

import { App } from "../app";
import { startDaemon } from "../fake-daemon/daemon";
import { sessionInfo, tabInfo } from "../fake-daemon/messages";

const sidebar = () => screen.getByRole("navigation", { name: "Sessions and tabs" });

const shownApp = async () => {
  const daemon = startDaemon([
    sessionInfo("work", [
      tabInfo("t1", { program: "vim" }),
      tabInfo("t2", { program: "make", exited: true, exit_code: 2 }),
      tabInfo("t3", { program: "ls", exited: true, exit_code: 0 }),
      tabInfo("t4", { program: "htop", control: "human" }),
    ]),
    sessionInfo("solo", [tabInfo("t1", { program: "zsh" })]),
  ]);

  render(<App />);
  await daemon.serve();

  return daemon;
};

test("each session lists its tabs with their program and state", async () => {
  await shownApp();
  const links = within(sidebar()).getAllByRole("link").map((link) => [link.textContent, link.getAttribute("aria-current")]);

  expect(links).toEqual([
    ["vim, work t1, running", "page"],
    ["make, work t2, exited 2exited 2", null],
    ["ls, work t3, exited 0exited 0", null],
    ["htop, work t4, humanhuman", null],
    ["zsh, solo t1, running", null],
  ]);
  expect(within(sidebar()).getAllByRole("button", { expanded: true }).map((head) => head.textContent)).toEqual(["work4, 4 tabs", "solo1, 1 tab"]);
});

test("a session folds away its tabs and a tab link opens that tab", async () => {
  await shownApp();
  const work = within(sidebar()).getByRole("button", { name: /^work/ });

  fireEvent.click(work);
  expect(work.getAttribute("aria-expanded")).toBe("false");
  expect(within(sidebar()).getAllByRole("link").map((link) => link.textContent)).toEqual(["zsh, solo t1, running"]);
  fireEvent.click(work);
  fireEvent.click(within(sidebar()).getByRole("link", { name: /make/ }));
  expect(location.pathname).toBe("/s/work/t2");
  expect(fireEvent.click(within(sidebar()).getByRole("link", { name: /zsh/ }), { ctrlKey: true })).toBe(true);
});

test("the theme switches between dark and light and is remembered", async () => {
  await shownApp();
  const toggle = screen.getByRole("button", { name: /Switch to (light|dark) mode/ });
  const toLight = toggle.textContent === "Switch to light mode";

  fireEvent.click(toggle);
  expect([localStorage.getItem("agent-terminal-theme"), document.documentElement.classList.contains("dark")]).toEqual([toLight ? "light" : "dark", !toLight]);
  expect(document.documentElement.style.colorScheme).toBe(toLight ? "light" : "dark");
  fireEvent.click(screen.getByRole("button", { name: toLight ? "Switch to dark mode" : "Switch to light mode" }));
  expect([localStorage.getItem("agent-terminal-theme"), document.documentElement.classList.contains("dark")]).toEqual([toLight ? "dark" : "light", toLight]);
});

test("the sidebar draws each tab's state, marks the current session and tab, and folds a group", async () => {
  await shownApp();
  const solo = () => within(sidebar()).getByRole("button", { name: /^solo/ });

  expect(screen.getByRole("complementary", { name: "Sessions" }).outerHTML).toMatchSnapshot();
  fireEvent.click(solo());
  expect([solo().outerHTML, document.getElementById(solo().getAttribute("aria-controls") ?? "")?.hidden]).toMatchSnapshot();
});

test("closing all sessions needs sessions to close", async () => {
  const daemon = startDaemon([]);

  render(<App />);
  await daemon.serve();
  expect(["Close all sessions", "New session"].map((name) => screen.getAllByRole<HTMLButtonElement>("button", { name })[0]?.disabled)).toEqual([true, false]);
});
