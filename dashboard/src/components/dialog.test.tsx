import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { expect, test } from "vitest";

import { App } from "../app";
import { startDaemon } from "../fake-daemon/daemon";
import { frameOf, sessionInfo, tabInfo } from "../fake-daemon/messages";

const STREAM = "/api/session/work/t1/stream";

const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]):not([type="hidden"]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

const shownApp = async () => {
  const daemon = startDaemon([sessionInfo("work", [tabInfo("t1", { program: "vim" })])]);

  render(<App />);
  await daemon.serve();

  return daemon;
};

const newSession = () => {
  fireEvent.click(screen.getByRole("button", { name: "New session" }));

  return screen.getByRole("dialog");
};

test("Escape, the close button and a click outside close a dialog, and back closes it too", async () => {
  await shownApp();
  fireEvent.keyDown(newSession(), { key: "Escape" });
  expect(screen.queryByRole("dialog")).toBeNull();
  fireEvent.click(within(newSession()).getByRole("button", { name: "Close" }));
  expect(screen.queryByRole("dialog")).toBeNull();
  newSession();
  const overlay = document.querySelector(".fixed.inset-0.isolate");

  expect(overlay).not.toBeNull();
  fireEvent.click(overlay ?? document.body);
  expect(screen.queryByRole("dialog")).toBeNull();
  newSession();
  await act(async () => {
    window.dispatchEvent(new PopStateEvent("popstate"));
    await Promise.resolve();
  });
  expect(screen.queryByRole("dialog")).toBeNull();
});

test("Tab and Shift+Tab wrap around inside the dialog and other keys pass", async () => {
  await shownApp();
  const dialog = newSession();
  const focusables = Array.from(dialog.querySelectorAll<HTMLElement>(FOCUSABLE));
  const first = focusables[0];
  const last = focusables.at(-1);

  last?.focus();
  expect(fireEvent.keyDown(dialog, { key: "Tab" })).toBe(false);
  expect(document.activeElement).toBe(first);
  expect(fireEvent.keyDown(dialog, { key: "Tab", shiftKey: true })).toBe(false);
  expect(document.activeElement).toBe(last);
  first?.focus();
  expect(fireEvent.keyDown(dialog, { key: "Tab" })).toBe(true);
  expect(fireEvent.keyDown(dialog, { key: "a" })).toBe(true);
  document.body.focus();
  expect(fireEvent.keyDown(dialog, { key: "Tab" })).toBe(false);
  expect(document.activeElement).toBe(first);
  fireEvent.keyDown(dialog, { key: "Escape" });
});

test("restarting a running program asks first in an alert that only its buttons close", async () => {
  const daemon = await shownApp();

  daemon.answer("POST /api/session/work/t1/restart", () => ({ status: 503, body: { error: { code: 1, message: "daemon busy" } } }));
  await daemon.connected(STREAM);
  await daemon.push(STREAM, frameOf(2));
  fireEvent.click(await screen.findByRole("button", { name: "Restart vim" }));
  const alert = screen.getByRole("alertdialog", { name: "Restart vim?" });

  expect(within(alert).getByText("work t1 is killed and started again with the same command, size and working directory.")).toBeTruthy();
  expect(within(alert).queryByRole("button", { name: "Close" })).toBeNull();
  fireEvent.click(document.querySelector(".fixed.inset-0.isolate") ?? document.body);
  fireEvent.click(within(alert).getByRole("button", { name: "Cancel" }));
  expect(screen.queryByRole("alertdialog")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Restart vim" }));
  fireEvent.click(within(screen.getByRole("alertdialog")).getByRole("button", { name: "Restart" }));
  expect(await screen.findByText("Could not restart vim")).toBeTruthy();
  expect(screen.getByText("daemon busy")).toBeTruthy();
});
