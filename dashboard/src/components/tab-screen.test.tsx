import { fireEvent, render, screen, within } from "@testing-library/react";
import { expect, test } from "vitest";

import { App } from "../app";
import { startDaemon, type FakeDaemon } from "../fake-daemon/daemon";
import { frameOf, sessionInfo, tabInfo } from "../fake-daemon/messages";

const STREAM = "/api/session/work/t1/stream";

const shownApp = async (daemon: FakeDaemon) => {
  render(<App />);
  await daemon.serve();
  await daemon.connected(STREAM);
};

const exitNote = (text: string) => screen.findByText((_, element) => element?.tagName === "SPAN" && element.textContent === text);

const status = () => document.getElementById("status-region")?.textContent ?? "";

const screenRows = () => within(screen.getByRole("list", { name: "Screen" })).getAllByRole("listitem").map((row) => row.textContent);

test("a tab waits for its first frame, then shows it live with its size and modes", async () => {
  const daemon = startDaemon([sessionInfo("work", [tabInfo("t1", { program: "htop", command: "htop -d 5", cwd: "/srv" })])]);

  await shownApp(daemon);
  expect(screen.getByText("Waiting for first frame...")).toBeTruthy();
  expect(status()).toContain("Connected");
  expect(within(screen.getByRole("group", { name: "Command line" })).getByText("htop -d 5")).toBeTruthy();
  await daemon.push(STREAM, frameOf(1, { cols: 20, rows: 3, paint: "hello\r\nworld", alt_screen: true, mouse: "drag", bracketed_paste: true }));
  expect(await screen.findByText("Live")).toBeTruthy();
  expect(screenRows()).toEqual(["hello", "world", ""]);
  expect(status()).toContain("ws://localhost:3000/api/session/work/t1/stream");
  expect(["20 × 3", "alt-screen", "mouse drag", "bracketed paste", "frame 1"].map((chip) => screen.getByText(chip).textContent)).toHaveLength(5);
  expect(screen.queryByText("Waiting for first frame...")).toBeNull();
  expect(daemon.sent(STREAM)).toEqual([{ type: "shown" }]);
});

test("an exited program offers restart and close, and closing moves to the next tab", async () => {
  const daemon = startDaemon([sessionInfo("work", [tabInfo("t1", { program: "make" }), tabInfo("t2")])]);

  daemon.answer("POST /api/session/work/t1/close", () => ({ status: 200, body: { data: null } }));
  await shownApp(daemon);
  await daemon.push(STREAM, frameOf(3, { exited: true, exit_code: 2 }));
  expect(await exitNote("make exited with code 2")).toBeTruthy();
  expect(status()).toContain("Exited");
  fireEvent.click(screen.getByRole("button", { name: "Close tab" }));
  expect(await screen.findByText("Closed work t1")).toBeTruthy();
  expect(location.pathname).toBe("/s/work/t2");
  expect(daemon.requests.map((request) => `${request.method} ${request.path}`)).toContain("POST /api/session/work/t1/close");
});

test("a close the daemon refuses shows an error and stays on the tab", async () => {
  const daemon = startDaemon([sessionInfo("work", [tabInfo("t1")])]);

  daemon.answer("POST /api/session/work/t1/close", () => ({ status: 409, body: { error: { code: 1, message: "busy" } } }));
  await shownApp(daemon);
  await daemon.push(STREAM, frameOf(1, { exited: true }));
  fireEvent.click(await screen.findByRole("button", { name: "Close tab" }));
  expect(await screen.findByText("Could not close work t1")).toBeTruthy();
  expect(screen.getByText("busy")).toBeTruthy();
  expect(location.pathname).toBe("/s/work/t1");
});

test("restarting an exited program asks the daemon and announces it", async () => {
  const daemon = startDaemon([sessionInfo("work", [tabInfo("t1", { program: "make" })])]);

  daemon.answer("POST /api/session/work/t1/restart", () => ({ status: 200, body: { data: null } }));
  await shownApp(daemon);
  await daemon.push(STREAM, frameOf(1, { exited: true, exit_code: null }));
  expect(await exitNote("make exited")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Restart" }));
  expect(await screen.findByText("make restarted")).toBeTruthy();
});
