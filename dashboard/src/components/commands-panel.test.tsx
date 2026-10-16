import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import { App } from "../app";
import { startDaemon, type FakeDaemon } from "../fake-daemon/daemon";
import { commandOf, sessionInfo, tabInfo } from "../fake-daemon/messages";
import type { CommandRecord } from "../lib/protocol";

const COMMANDS = "/api/session/my%20work/t1/commands/stream";

const shownCommands = async (program = "zsh") => {
  const daemon = startDaemon([sessionInfo("my work", [tabInfo("t1", { program })])]);

  render(<App />);
  await daemon.serve();
  fireEvent.click(screen.getByRole("tab", { name: "Commands" }));
  await daemon.connected(COMMANDS);

  return daemon;
};

interface Feed { shell?: boolean; integration?: boolean; commands?: CommandRecord[] }

const pushed = async (daemon: FakeDaemon, feed: Feed) => {
  await daemon.push(COMMANDS, { type: "commands", shell: feed.shell ?? true, integration: feed.integration ?? true, home: "/home/me", commands: feed.commands ?? [] });
};

const panel = () => screen.getByRole("tabpanel");

const counted = () => within(panel()).getByRole("heading", { name: "Commands" }).parentElement?.textContent;

const opened = async (name: RegExp) => {
  await act(async () => {
    fireEvent.click(screen.getByRole("button", { name }));
    await Promise.resolve();
  });
};

test("a program that is not a shell, or a shell without integration, says how to get commands", async () => {
  const daemon = await shownCommands("htop");

  expect(within(panel()).getByRole("heading", { name: "Commands" })).toBeTruthy();
  await pushed(daemon, { shell: false });
  expect([within(panel()).getByText("htop is not a shell"), counted()]).toEqual([within(panel()).getByText("htop is not a shell"), "Commands"]);
  expect(within(panel()).getByText("agent-terminal shell")).toBeTruthy();
  await pushed(daemon, { integration: false });
  expect([within(panel()).getByText("Shell integration is off for this tab"), counted()]).toEqual([within(panel()).getByText("Shell integration is off for this tab"), "Commands"]);
  expect(within(panel()).getByText("agent-terminal --session 'my work' shell")).toBeTruthy();
  expect(within(panel()).getByRole("heading", { name: "Commands" }).parentElement?.outerHTML).toMatchSnapshot();
  await pushed(daemon, {});
  expect([within(panel()).getByText("No commands yet").parentElement?.outerHTML, counted()]).toEqual(['<div class="px-6 py-8 text-center text-xs text-muted-foreground"><p>No commands yet</p></div>', "Commands0"]);
});

test("commands list their result, short path, duration and start time", async () => {
  const daemon = await shownCommands();

  await pushed(daemon, {
    commands: [commandOf(1), commandOf(2, { exit_code: 2 }), commandOf(3, { exit_code: null }), commandOf(4, { ended_at_ms: null, exit_code: null })],
  });
  const rows = within(screen.getByRole("list", { name: "Commands in my work t1" })).getAllByRole("listitem");

  expect(rows.map((row) => row.textContent)).toEqual([
    "✓exit 0step 1~/p/src · 1.0 s · 03:04:05+",
    "✗ 2exit 2step 2~/p/src · 1.0 s · 03:04:05+",
    "?exit unknownstep 3~/p/src · 1.0 s · 03:04:05+",
    "runningstep 4~/p/src · running · 03:04:05+",
  ]);
  expect(within(panel()).getByText("4")).toBeTruthy();
});

test("opening a command loads its output once, styled, and shows an empty or failed load", async () => {
  const daemon = await shownCommands();
  const answered = vi.fn((id: string) => ({ "11": "\x1b[1mbuilt\x1b[0m\nok", "12": "" })[id]);

  for (const id of ["11", "12"]) {
    daemon.answer(`GET /api/session/my%20work/t1/commands/${id}`, () => ({ status: 200, body: { output: answered(id) } }));
  }

  daemon.answer("GET /api/session/my%20work/t1/commands/13", () => ({ status: 500, body: "disk full" }));
  await pushed(daemon, { commands: [commandOf(11), commandOf(12), commandOf(13)] });
  await opened(/step 11/);
  expect((await within(panel()).findByText("built")).style.fontWeight).toBe("700");
  await opened(/step 12/);
  expect(await within(panel()).findByText("No output")).toBeTruthy();
  expect(within(panel()).queryByText("built")).toBeNull();
  await opened(/step 11/);
  expect(await within(panel()).findByText("ok")).toBeTruthy();
  expect(answered).toHaveBeenCalledTimes(2);
  await opened(/step 13/);
  expect(await within(panel()).findByText("disk full")).toBeTruthy();
  await opened(/step 13/);
  expect(screen.getByRole("button", { name: /step 13/ }).getAttribute("aria-expanded")).toBe("false");
});

test("the CLI line copies to the clipboard, and a refused copy says why", async () => {
  const daemon = await shownCommands("htop");
  const writeText = vi.fn(() => Promise.resolve());

  vi.stubGlobal("navigator", { clipboard: { writeText } });
  await pushed(daemon, { shell: false });
  fireEvent.click(within(panel()).getByRole("button", { name: "Copy agent-terminal shell" }));
  expect(await screen.findByText("Copied")).toBeTruthy();
  expect(writeText).toHaveBeenCalledWith("agent-terminal shell");
  writeText.mockImplementation(() => Promise.reject(new Error("not allowed")));
  fireEvent.click(within(panel()).getByRole("button", { name: "Copy agent-terminal shell" }));
  expect(await screen.findByText("Could not copy")).toBeTruthy();
  expect(screen.getByText("not allowed")).toBeTruthy();
});

test("each result has its own mark, a failed command its own background, and an open command its own", async () => {
  const daemon = await shownCommands();

  await pushed(daemon, {
    commands: [commandOf(1), commandOf(2, { exit_code: 2 }), commandOf(3, { exit_code: null }), commandOf(4, { ended_at_ms: null, exit_code: null })],
  });
  const looks = () =>
    within(screen.getByRole("list", { name: "Commands in my work t1" }))
      .getAllByRole("listitem")
      .map((row) => [row.className, row.querySelector("button > span")?.className, row.querySelector("button > span:last-child")?.outerHTML]);
  const SIGN = (sign: string) => `<span class="ml-auto shrink-0 font-mono text-muted-foreground" aria-hidden="true">${sign}</span>`;

  expect(looks()).toEqual([
    ["border-b border-border", "w-12 shrink-0 font-mono text-green-600 dark:text-success", SIGN("+")],
    ["border-b border-border bg-destructive/5", "w-12 shrink-0 font-mono text-destructive", SIGN("+")],
    ["border-b border-border", "w-12 shrink-0 font-mono text-muted-foreground", SIGN("+")],
    ["border-b border-border", "w-12 shrink-0 font-mono text-muted-foreground", SIGN("+")],
  ]);
  daemon.answer("GET /api/session/my%20work/t1/commands/2", () => ({ status: 200, body: { output: "failed" } }));
  await opened(/step 2/);
  expect(looks()[1]).toEqual(["border-b border-border bg-destructive/5 bg-muted/60", "w-12 shrink-0 font-mono text-destructive", SIGN("-")]);
  expect((await within(panel()).findByText("failed")).closest("div.max-h-64")?.className).toBe("mb-1.5 max-h-64 overflow-auto rounded-md bg-muted/50 px-2 py-1.5 font-mono text-[10px] leading-snug");
});
