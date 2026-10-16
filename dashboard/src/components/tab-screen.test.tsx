import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import { App } from "../app";
import { startDaemon, type FakeDaemon } from "../fake-daemon/daemon";
import { frameOf, sessionInfo, tabInfo } from "../fake-daemon/messages";

const STREAM = "/api/session/work/app/stream";

const shownApp = async (daemon: FakeDaemon) => {
  render(<App />);
  await daemon.serve();
  await daemon.connected(STREAM);
};

const exitNote = (text: string) => screen.findByText((_, element) => element?.tagName === "SPAN" && element.textContent === text);

const status = () => screen.getByRole("group", { name: "Status" }).textContent;

const screenRows = () => within(screen.getByRole("list", { name: "Screen" })).getAllByRole("listitem").map((row) => row.textContent);

test("a tab waits for its first frame, then shows it live with its size and modes", async () => {
  const daemon = startDaemon([sessionInfo("work", [tabInfo("app", { program: "htop", command: "htop -d 5", cwd: "/srv" })])]);

  await shownApp(daemon);
  expect(screen.getByText("Waiting for first frame...")).toBeTruthy();
  expect(status()).toContain("Connected");
  expect(within(screen.getByRole("group", { name: "Command line" })).getByText("htop -d 5")).toBeTruthy();
  await daemon.push(STREAM, frameOf(1, { cols: 20, rows: 3, paint: "hello\r\nworld", alt_screen: true, mouse: "drag", bracketed_paste: true }));
  expect(await screen.findByText("Live")).toBeTruthy();
  expect(screen.getByRole("group", { name: "work app htop, live frame 1" })).toBeTruthy();
  expect(screenRows()).toEqual(["hello", "world", ""]);
  expect(status()).toContain("ws://localhost:3000/api/session/work/app/stream");
  expect(["20 × 3", "alt-screen", "mouse drag", "bracketed paste", "frame 1"].map((chip) => screen.getByText(chip).textContent)).toHaveLength(5);
  expect(screen.queryByText("Waiting for first frame...")).toBeNull();
  expect(daemon.sent(STREAM)).toEqual([{ type: "shown" }]);
});

test("a tab renamed by the agent moves the page to its new name and keeps showing it", async () => {
  const daemon = startDaemon([sessionInfo("work", [tabInfo("app", { program: "htop" })])]);

  await shownApp(daemon);
  await daemon.push(STREAM, frameOf(1, { paint: "hello" }));
  expect(await screen.findByRole("heading", { name: "work app · htop" })).toBeTruthy();
  await daemon.publish([sessionInfo("work", [tabInfo("web", { program: "htop", renamed_from: "app" })])]);
  expect(location.pathname).toBe("/s/work/web");
  expect(await screen.findByRole("heading", { name: "work web · htop" })).toBeTruthy();
  await daemon.connected("/api/session/work/web/stream");
  await daemon.push("/api/session/work/web/stream", frameOf(2, { paint: "again" }));
  expect(await screen.findByRole("group", { name: "work web htop, live frame 2" })).toBeTruthy();
});

test("an exited program offers restart and close, and closing moves to the next tab", async () => {
  const daemon = startDaemon([sessionInfo("work", [tabInfo("app", { program: "make" }), tabInfo("api")])]);

  daemon.answer("POST /api/session/work/app/close", () => ({ status: 200, body: { data: null } }));
  await shownApp(daemon);
  await daemon.push(STREAM, frameOf(3, { exited: true, exit_code: 2 }));
  expect(await exitNote("make exited with code 2")).toBeTruthy();
  expect(status()).toContain("Exited");
  fireEvent.click(screen.getByRole("button", { name: "Close tab" }));
  expect(await screen.findByText("Closed work app")).toBeTruthy();
  expect(location.pathname).toBe("/s/work/api");
  expect(daemon.requests.map((request) => `${request.method} ${request.path}`)).toContain("POST /api/session/work/app/close");
});

test("a close the daemon refuses shows an error and stays on the tab", async () => {
  const daemon = startDaemon([sessionInfo("work", [tabInfo("app")])]);

  daemon.answer("POST /api/session/work/app/close", () => ({ status: 409, body: { error: { code: 1, message: "busy" } } }));
  await shownApp(daemon);
  await daemon.push(STREAM, frameOf(1, { exited: true }));
  fireEvent.click(await screen.findByRole("button", { name: "Close tab" }));
  expect(await screen.findByText("Could not close work app")).toBeTruthy();
  expect(screen.getByText("busy")).toBeTruthy();
  expect(location.pathname).toBe("/s/work/app");
});

test("restarting an exited program asks the daemon and announces it", async () => {
  const daemon = startDaemon([sessionInfo("work", [tabInfo("app", { program: "make" })])]);

  daemon.answer("POST /api/session/work/app/restart", () => ({ status: 200, body: { data: null } }));
  await shownApp(daemon);
  await daemon.push(STREAM, frameOf(1, { exited: true, exit_code: null }));
  expect(await exitNote("make exited")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Restart" }));
  expect(await screen.findByText("make restarted")).toBeTruthy();
});

const statusBar = () => {
  const bar = within(screen.getByRole("group", { name: "Status" }));

  return [
    bar.getByText(/^(Connected|Exited|Live)$/).textContent,
    within(bar.getByRole("list", { name: "Details" })).queryAllByRole("listitem").map((chip) => chip.textContent),
    bar.queryByText(/^Keys go to /),
  ];
};

test("the status bar says Connected with only the size until a frame is drawn, and Exited once the program ends", async () => {
  const daemon = startDaemon([sessionInfo("work", [tabInfo("app", { cols: 100, rows: 30 })])]);

  await shownApp(daemon);
  expect(statusBar()).toEqual(["Connected", ["100 × 30"], null]);
  await daemon.push(STREAM, frameOf(0, { mouse: "drag" }));
  await vi.waitFor(() => {
    expect(daemon.sent(STREAM)).toHaveLength(1);
  });
  expect(statusBar()).toEqual(["Connected", ["80 × 24", "mouse drag"], null]);
  await daemon.push(STREAM, frameOf(0, { exited: true }));
  await vi.waitFor(() => {
    expect(daemon.sent(STREAM)).toHaveLength(2);
  });
  expect(statusBar()).toEqual(["Exited", ["80 × 24", "frame 0"], null]);
});

test("choosing the layout already shown keeps the tab, and a new layout takes focus to its button", async () => {
  const daemon = startDaemon([sessionInfo("work", [tabInfo("app"), tabInfo("api")])]);

  await shownApp(daemon);
  fireEvent.click(screen.getByRole("button", { name: "Single terminal" }));
  expect(location.pathname).toBe("/s/work/app");
  fireEvent.click(screen.getByRole("button", { name: "Two terminals side by side" }));
  expect([location.pathname, screen.getByRole("button", { name: "Two terminals side by side" }).matches(":focus")]).toEqual(["/split/work:app,work:api", true]);
});

test("closing a tab moves to the next open tab, even one of the same name in another session, and closing the last goes home", async () => {
  const daemon = startDaemon([sessionInfo("work", [tabInfo("app")]), sessionInfo("play", [tabInfo("app")])]);

  daemon.answer("POST /api/session/work/app/close", () => ({ status: 200, body: { data: null } }));
  daemon.answer("POST /api/session/play/app/close", () => ({ status: 200, body: { data: null }, sessions: [] }));
  await shownApp(daemon);
  await daemon.push(STREAM, frameOf(1, { exited: true }));
  fireEvent.click(await screen.findByRole("button", { name: "Close tab" }));
  await vi.waitFor(() => {
    expect(location.pathname).toBe("/s/play/app");
  });
  await act(async () => {
    daemon.announce([sessionInfo("play", [tabInfo("app")])]);
    await Promise.resolve();
  });
  await daemon.connected("/api/session/play/app/stream");
  await daemon.push("/api/session/play/app/stream", frameOf(1, { exited: true }));
  fireEvent.click(await screen.findByRole("button", { name: "Close tab" }));
  await vi.waitFor(() => {
    expect(location.pathname).toBe("/");
  });
});
