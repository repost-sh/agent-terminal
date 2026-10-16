import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";

import { SESSIONS_STREAM, startDaemon } from "../fake-daemon/daemon";
import { sessionInfo, tabInfo } from "../fake-daemon/messages";
import { everySocket, FakeSocket } from "../fake-daemon/sockets";

const SESSIONS = [sessionInfo("work", [tabInfo("app")])];

const watched = async () => {
  const { useDaemon } = await import("./daemon");
  const { announcements } = await import("./announcements");
  const { navigate } = await import("./route");
  const daemon = startDaemon(SESSIONS);
  const hook = renderHook(() => useDaemon());

  return { daemon, hook, announcements, navigate };
};

const sockets = () => everySocket().filter((socket) => socket.path === SESSIONS_STREAM);

const reconnectedAfter = async () => {
  const count = sockets().length;
  let waited = 0;

  while (sockets().length === count && waited < 10_000) {
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1);
    });
    waited += 1;
  }

  return waited;
};

beforeEach(() => {
  vi.resetModules();
  vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
});

afterEach(() => {
  vi.useRealTimers();
});

test("the sessions arrive and lead home to the first tab, until the daemon is lost", async () => {
  const shown = await watched();
  const { daemon, hook } = shown;

  expect(hook.result.current.connection).toBe("connecting");
  await daemon.serve();
  expect([hook.result.current.connection, hook.result.current.sessions, location.pathname]).toEqual(["connected", SESSIONS, "/s/work/app"]);
  await daemon.hangUp(SESSIONS_STREAM);
  act(() => {
    shown.navigate("/");
  });
  expect([hook.result.current.connection, location.pathname]).toEqual(["reconnecting", "/"]);
  hook.unmount();
});

test("a lost daemon is tried after 500, 1000 and 2000 ms and then given up on", async () => {
  const { daemon, hook } = await watched();

  await daemon.serve();
  await daemon.hangUp(SESSIONS_STREAM);
  const waits = [await reconnectedAfter()];

  await daemon.refuse();
  waits.push(await reconnectedAfter());
  await daemon.refuse();
  waits.push(await reconnectedAfter());
  await daemon.refuse();
  await act(async () => {
    await vi.advanceTimersByTimeAsync(60_000);
  });
  expect([waits, hook.result.current.connection, sockets().length]).toEqual([[500, 1000, 2000], "down", 4]);
  hook.unmount();
});

test("retrying connects at once, says Connected once, and drops the pending attempt", async () => {
  const shown = await watched();
  const { daemon, hook } = shown;

  await daemon.serve();
  await daemon.hangUp(SESSIONS_STREAM);
  act(() => {
    hook.result.current.retry();
  });
  expect(sockets().length).toBe(2);
  await daemon.serve();
  await daemon.publish(SESSIONS);
  await act(async () => {
    await vi.advanceTimersByTimeAsync(10_000);
  });
  expect([sockets().length, hook.result.current.connection, shown.announcements.get()]).toEqual([2, "connected", { text: "Connected", times: 1 }]);
  hook.unmount();
});

test("when nothing watches the daemon any more its socket closes and stays closed", async () => {
  const { daemon, hook } = await watched();

  await daemon.serve();
  hook.unmount();
  await act(async () => {
    await vi.advanceTimersByTimeAsync(10_000);
  });
  expect(sockets().map((socket) => socket.readyState)).toEqual([FakeSocket.CLOSED]);
  const lost = await watched();

  await lost.daemon.serve();
  await lost.daemon.hangUp(SESSIONS_STREAM);
  lost.hook.unmount();
  await act(async () => {
    await vi.advanceTimersByTimeAsync(10_000);
  });
  expect(sockets().length).toBe(1);
});
