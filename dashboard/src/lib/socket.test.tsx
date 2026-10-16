import { act, fireEvent, render, screen } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import { App } from "../app";
import { startDaemon, type FakeDaemon } from "../fake-daemon/daemon";
import { frameOf, sessionInfo, shotOf, tabInfo } from "../fake-daemon/messages";
import { waiting } from "../fake-daemon/sockets";

const SHOTS = "/api/session/work/screenshots/stream";
const STREAM = "/api/session/work/t1/stream";

const shownApp = async () => {
  const daemon = startDaemon([sessionInfo("work", [tabInfo("t1")])]);

  render(<App />);
  await daemon.serve();

  return daemon;
};

const after = async (ms: number) => {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(ms);
  });
};

const status = () => screen.getByRole("group", { name: "Status" }).textContent;

const reopened = async (daemon: FakeDaemon, ms: number) => {
  await after(ms - 1);
  expect(daemon.sent(SHOTS)).toEqual([]);
  await after(1);
};

test("a feed that drops reconnects after a growing wait and picks up where the daemon is", async () => {
  vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
  const daemon = await shownApp();

  fireEvent.click(screen.getByRole("tab", { name: "Screenshots" }));
  await daemon.serve();
  await daemon.push(SHOTS, { type: "screenshots", screenshots: [] });
  expect(screen.getByText("No screenshots yet")).toBeTruthy();
  await daemon.hangUp(SHOTS);
  expect(screen.queryByText("No screenshots yet")).toBeNull();
  await reopened(daemon, 500);
  await daemon.refuse();
  await reopened(daemon, 1000);
  await daemon.serve();
  await daemon.push(SHOTS, { type: "screenshots", screenshots: [shotOf("a.png")] });
  expect(screen.getByRole("button", { name: /a\.png/ })).toBeTruthy();
  await daemon.hangUp(SHOTS);
  await reopened(daemon, 500);
  await daemon.serve();
  await daemon.push(SHOTS, { type: "closed" });
  await after(10_000);
  expect([screen.getByRole("button", { name: /a\.png/ }).textContent, waiting()]).toEqual(["a.pngt1 · frame 3 · 03:04:05dashboard", []]);
  vi.useRealTimers();
});

test("a dropped terminal stream shows it is reconnecting and redraws from the next first frame", async () => {
  vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
  const daemon = await shownApp();

  await daemon.serve();
  await daemon.push(STREAM, frameOf(4, { paint: "old" }));
  await after(1);
  await daemon.hangUp(STREAM);
  expect(status()).toContain("Reconnecting…");
  await after(500);
  await daemon.serve();
  expect(status()).toContain("Live");
  expect(status()).toContain("frame 4");
  await daemon.push(STREAM, frameOf(5, { paint: "new" }));
  await after(1);
  expect(status()).toContain("frame 5");
  await daemon.push(STREAM, { type: "closed" });
  expect(status()).not.toContain("frame 5");
  vi.useRealTimers();
});
