import { fireEvent, render, screen, within } from "@testing-library/react";
import { expect, test } from "vitest";

import { App } from "../app";
import { startDaemon } from "../fake-daemon/daemon";
import { sessionInfo, tabInfo } from "../fake-daemon/messages";

const shownApp = async () => {
  const daemon = startDaemon([sessionInfo("work", [tabInfo("t1"), tabInfo("t2", { exited: true })]), sessionInfo("play", [tabInfo("t1")])]);

  render(<App />);
  await daemon.serve();

  return daemon;
};

const closeAllDialog = () => {
  fireEvent.click(screen.getByRole("button", { name: "Close all sessions" }));

  return screen.getByRole("alertdialog", { name: "Close all sessions" });
};

test("closing all sessions counts what it kills, asks, and closes them on confirm", async () => {
  const daemon = await shownApp();

  daemon.answer("POST /api/sessions/close", () => ({ status: 200, body: { data: null }, sessions: [] }));
  const dialog = closeAllDialog();

  expect(within(dialog).getByText("This will close 2 sessions and kill 2 running processes. This action cannot be undone.")).toBeTruthy();
  expect(document.activeElement?.textContent).toBe("Cancel");
  fireEvent.click(within(dialog).getByRole("button", { name: "Close all" }));
  expect(await screen.findByText("Closed 2 sessions")).toBeTruthy();
  expect(screen.queryByRole("alertdialog")).toBeNull();
  expect([location.pathname, document.activeElement?.id]).toEqual(["/", "main-content"]);
  expect(await screen.findByRole("list", { name: "Commands to get started" })).toBeTruthy();
});

test("a refused close-all keeps the dialog and says why, and cancel puts focus back", async () => {
  const daemon = await shownApp();

  daemon.answer("POST /api/sessions/close", () => ({ status: 500, body: "" }));
  const trigger = screen.getByRole("button", { name: "Close all sessions" });

  trigger.focus();
  const dialog = closeAllDialog();

  fireEvent.click(within(dialog).getByRole("button", { name: "Close all" }));
  expect(await screen.findByText("Could not close all sessions")).toBeTruthy();
  expect(screen.getByText("500")).toBeTruthy();
  fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
  expect(screen.queryByRole("alertdialog")).toBeNull();
  expect(document.activeElement).toBe(trigger);
});

test("one session with one running process reads in the singular", async () => {
  const daemon = startDaemon([sessionInfo("solo", [tabInfo("t1")])]);

  render(<App />);
  await daemon.serve();
  const dialog = closeAllDialog();

  expect(within(dialog).getByText("This will close 1 session and kill 1 running process. This action cannot be undone.")).toBeTruthy();
  fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
});
