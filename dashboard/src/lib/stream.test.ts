import { act, renderHook } from "@testing-library/react";
import { Terminal } from "@xterm/xterm";
import { beforeEach, expect, test, vi } from "vitest";

import { startDaemon } from "../fake-daemon/daemon";
import { frameOf, sessionInfo, tabInfo } from "../fake-daemon/messages";
import { openAt } from "../fake-daemon/sockets";
import type { FrameUpdate } from "./protocol";

const STREAM = "/api/session/work/t1/stream";

const painted = async () => {
  const { useTabStream, announceWhenLive } = await import("./stream");
  const { announcements } = await import("./announcements");
  const daemon = startDaemon([sessionInfo("work", [tabInfo("t1")])]);
  const hook = renderHook(() => useTabStream("work", "t1"));
  const terminal = new Terminal({ cols: 80, rows: 24, allowProposedApi: true });
  const stop = hook.result.current.paint(terminal);

  await daemon.connected(STREAM);

  return { daemon, hook, terminal, stop, announcements, announceWhenLive };
};

const row = (terminal: Terminal, y: number) => terminal.buffer.active.getLine(y)?.translateToString(true);

const drawn = async (shown: Awaited<ReturnType<typeof painted>>, update: FrameUpdate) => {
  await shown.daemon.push(STREAM, update);
  await vi.waitFor(() => {
    expect(shown.hook.result.current.shown?.frame).toBe(update.frame);
  });
};

beforeEach(() => {
  vi.resetModules();
});

test("frames draw into the terminal at the daemon's size and report each one back as shown", async () => {
  const shown = await painted();

  await drawn(shown, frameOf(1, { cols: 40, rows: 10, paint: "hello" }));
  expect([shown.terminal.cols, shown.terminal.rows, row(shown.terminal, 0), shown.hook.result.current.state]).toEqual([40, 10, "hello", "open"]);
  await drawn(shown, frameOf(2, { cols: 40, rows: 12, paint: " world" }));
  expect([shown.terminal.cols, shown.terminal.rows, row(shown.terminal, 0)]).toEqual([40, 12, "hello world"]);
  await drawn(shown, frameOf(3, { cols: 50, rows: 12 }));
  expect([shown.terminal.cols, shown.terminal.rows, shown.daemon.sent(STREAM)]).toEqual([50, 12, Array.from({ length: 3 }, () => ({ type: "shown" }))]);
  shown.stop();
});

test("the first frame after a reconnection starts from a blank terminal", async () => {
  const shown = await painted();

  await drawn(shown, frameOf(1, { paint: "old" }));
  await shown.daemon.hangUp(STREAM);
  expect(shown.hook.result.current.state).toBe("reconnecting");
  await shown.daemon.connected(STREAM);
  await drawn(shown, frameOf(2, { paint: "new" }));
  expect(row(shown.terminal, 0)).toBe("new");
  await shown.daemon.push(STREAM, { type: "closed" });
  expect(shown.hook.result.current.state).toBe("closed");
  shown.stop();
});

test("a tab waited for is announced live at its first drawn frame, once", async () => {
  const shown = await painted();

  shown.announceWhenLive("work", "t1");
  await drawn(shown, frameOf(0));
  expect(shown.announcements.get().times).toBe(0);
  await drawn(shown, frameOf(1));
  await drawn(shown, frameOf(2));
  expect(shown.announcements.get()).toEqual({ text: "work t1 is live", times: 1 });
  shown.stop();
});

test("a tab that exits before drawing is announced live too", async () => {
  const exited = await painted();

  exited.announceWhenLive("work", "t1");
  await drawn(exited, frameOf(0, { exited: true }));
  expect(exited.announcements.get()).toEqual({ text: "work t1 is live", times: 1 });
  exited.stop();
});

test("once painting stops the socket closes and a frame still being written is not reported", async () => {
  const shown = await painted();

  await shown.daemon.push(STREAM, frameOf(1, { paint: "late" }));
  act(() => {
    shown.stop();
  });
  await new Promise<void>((resolve) => {
    shown.terminal.write("", resolve);
  });
  expect([shown.hook.result.current.shown, openAt(STREAM)]).toEqual([null, []]);
});
