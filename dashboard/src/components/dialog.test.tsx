import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import { App } from "../app";
import { startDaemon } from "../fake-daemon/daemon";
import { frameOf, sessionInfo, tabInfo } from "../fake-daemon/messages";

const STREAM = "/api/session/work/app/stream";

const shownApp = async () => {
  const daemon = startDaemon([sessionInfo("work", [tabInfo("app", { program: "vim" })])]);

  render(<App />);
  await daemon.serve();

  return daemon;
};

const newSession = () => {
  fireEvent.click(screen.getByRole("button", { name: "New session" }));

  return screen.getByRole("dialog");
};

const focused = (element: HTMLElement | undefined) => element?.matches(":focus");

test("Escape, the close button and a click outside close a dialog, and back closes it too", async () => {
  await shownApp();
  fireEvent.keyDown(newSession(), { key: "Escape" });
  expect(screen.queryByRole("dialog")).toBeNull();
  fireEvent.click(within(newSession()).getByRole("button", { name: "Close" }));
  expect(screen.queryByRole("dialog")).toBeNull();
  newSession();
  fireEvent.click(screen.getByTestId("backdrop"));
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
  const [first] = within(dialog).getAllByRole("radio");
  const last = within(dialog).getByRole("button", { name: "Close" });

  last.focus();
  expect(fireEvent.keyDown(dialog, { key: "Tab" })).toBe(false);
  expect(focused(first)).toBe(true);
  expect(fireEvent.keyDown(dialog, { key: "Tab", shiftKey: true })).toBe(false);
  expect(focused(last)).toBe(true);
  first?.focus();
  expect(fireEvent.keyDown(dialog, { key: "Tab" })).toBe(true);
  expect(fireEvent.keyDown(dialog, { key: "a" })).toBe(true);
  last.focus();
  expect([fireEvent.keyDown(dialog, { key: "a" }), focused(last)]).toEqual([true, true]);
  screen.getByRole("main").focus();
  expect(fireEvent.keyDown(dialog, { key: "Tab" })).toBe(false);
  expect(focused(first)).toBe(true);
  fireEvent.keyDown(dialog, { key: "Escape" });
});

test("restarting a running program asks first in an alert that only its buttons close", async () => {
  const daemon = await shownApp();

  daemon.answer("POST /api/session/work/app/restart", () => ({ status: 503, body: { error: { code: 1, message: "daemon busy" } } }));
  await daemon.connected(STREAM);
  await daemon.push(STREAM, frameOf(2));
  fireEvent.click(await screen.findByRole("button", { name: "Restart vim" }));
  const alert = screen.getByRole("alertdialog", { name: "Restart vim?", description: "work app is killed and started again with the same command, size and working directory." });

  expect(alert.outerHTML).toMatchSnapshot();
  expect(within(alert).queryByRole("button", { name: "Close" })).toBeNull();
  fireEvent.click(screen.getByTestId("backdrop"));
  fireEvent.click(within(alert).getByRole("button", { name: "Cancel" }));
  expect(screen.queryByRole("alertdialog")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Restart vim" }));
  fireEvent.click(within(screen.getByRole("alertdialog")).getByRole("button", { name: "Restart" }));
  expect(await screen.findByText("Could not restart vim")).toBeTruthy();
  expect(screen.getByText("daemon busy")).toBeTruthy();
});

test("Tab skips controls that are not shown, unless focus is already on one", async () => {
  await shownApp();
  const dialog = newSession();
  const [first] = within(dialog).getAllByRole("radio");
  const unshown = within(dialog).getByRole("button", { name: "Close" });

  Object.defineProperty(unshown, "offsetParent", { value: null });
  within(dialog).getByRole("button", { name: "Create" }).focus();
  expect([fireEvent.keyDown(dialog, { key: "Tab" }), focused(first)]).toEqual([false, true]);
  unshown.focus();
  expect([fireEvent.keyDown(dialog, { key: "Tab" }), focused(first)]).toEqual([false, true]);
  expect(fireEvent.keyDown(dialog, { key: "Escape" })).toBe(false);
});

test("opening and closing move focus without scrolling, and a submitted form stays on the page", async () => {
  await shownApp();
  const opener = screen.getByRole("button", { name: "New session" });

  opener.focus();
  const focus = vi.spyOn(HTMLElement.prototype, "focus");
  const dialog = newSession();

  expect(fireEvent.submit(within(dialog).getByRole("button", { name: "Create" }))).toBe(false);
  fireEvent.click(within(dialog).getByRole("button", { name: "Close" }));
  expect([focus.mock.calls.at(0), focus.mock.calls.at(-1), focused(opener)]).toEqual([[{ preventScroll: true }], [{ preventScroll: true }], true]);
});

test("a dialog whose opener has gone returns focus to the main content", async () => {
  const daemon = await shownApp();

  const opener = screen.getByRole("button", { name: "New tab in work" });

  opener.focus();
  fireEvent.click(opener);
  await act(async () => {
    daemon.announce([]);
    await Promise.resolve();
  });
  fireEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Close" }));
  expect(focused(screen.getByRole("main"))).toBe(true);
});
