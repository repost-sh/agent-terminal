import { fireEvent, render, screen, within } from "@testing-library/react";
import { expect, test } from "vitest";

import { App } from "../app";
import { startDaemon, type FakeDaemon } from "../fake-daemon/daemon";
import { sessionInfo, tabInfo } from "../fake-daemon/messages";
import type { TabSetup } from "../lib/info";

const INFO = "/api/session/work/app/info";

const setup = (change: Partial<TabSetup>): TabSetup => ({
  program: "bash",
  args: "",
  pid: null,
  cwd: "/srv",
  term: null,
  cols: 80,
  rows: 24,
  shell_integration: null,
  modes: { alt_screen: false, mouse: null, bracketed_paste: false, app_cursor: false, focus_events: false, keyboard: [] },
  started: new Date(2026, 4, 6, 7, 8, 9).toISOString(),
  exited: false,
  exit_code: null,
  env: [],
  ...change,
});

const shownInfo = async () => {
  const daemon = startDaemon([sessionInfo("work", [tabInfo("app")])]);

  render(<App />);
  await daemon.serve();
  fireEvent.click(screen.getByRole("tab", { name: "Info" }));
  await daemon.connected(INFO);

  return daemon;
};

const rows = () => {
  const panel = within(screen.getByRole("tabpanel"));
  const definitions = panel.getAllByRole("definition");

  return panel.getAllByRole("term").map((term, index) => `${term.textContent}: ${String(definitions[index]?.textContent)}`);
};

const pushed = async (daemon: FakeDaemon, change: Partial<TabSetup>) => {
  await daemon.push(INFO, { type: "info", ...setup(change) });
};

test("the info panel lists the tab's setup, modes and environment", async () => {
  const daemon = await shownInfo();

  await pushed(daemon, {});
  expect(rows()).toEqual([
    "program: bash",
    "args: none",
    "pid: unknown",
    "cwd: /srv",
    "TERM: none",
    "size: 80 × 24",
    "shell integration: not a shell",
    "started: May 6, 2026 07:08:09",
    "alt-screen: off",
    "mouse tracking: off",
    "bracketed paste: off",
    "application cursor keys: off",
    "focus events: off",
    "kitty keyboard flags: off",
  ]);
  expect(screen.getByText("(0)")).toBeTruthy();
});

test("modes that are on, an exit and masked variables read as such", async () => {
  const daemon = await shownInfo();
  const modes = { alt_screen: true, mouse: { tracking: "drag", encoding: "sgr" }, bracketed_paste: true, app_cursor: true, focus_events: true, keyboard: ["disambiguate", "all"] };

  await pushed(daemon, { args: "-l", pid: 42, term: "xterm-256color", shell_integration: true, modes, exited: true, exit_code: 3, env: [{ name: "HOME", value: "/home/me" }, { name: "TOKEN", value: null }] });
  expect(rows()).toEqual([
    "program: bash",
    "args: -l",
    "pid: 42 (exited 3)",
    "cwd: /srv",
    "TERM: xterm-256color",
    "size: 80 × 24",
    "shell integration: on",
    "started: May 6, 2026 07:08:09",
    "alt-screen: on",
    "mouse tracking: on (sgr)",
    "bracketed paste: on",
    "application cursor keys: on",
    "focus events: on",
    "kitty keyboard flags: disambiguate, all",
    "HOME: /home/me",
    "TOKEN: ••••masked ",
  ]);
  await pushed(daemon, { shell_integration: false, exited: true });
  expect(within(screen.getByRole("tabpanel")).getAllByText(/off|exited/).map((element) => element.textContent)).toContain(" (exited)");
});
