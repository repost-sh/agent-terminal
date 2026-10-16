import { afterEach, beforeEach, expect, test, vi } from "vitest";

import { accept, deliver, drop, everySocket, FakeSocket, forgetSockets } from "../fake-daemon/sockets";
import { followSocket } from "./socket";

interface Update { type: string }

const followed = () => {
  const events: string[] = [];
  const stop = followSocket<Update>("ws://daemon.test/feed", {
    message: (message, delivery) => {
      events.push(`${message.type}${delivery.first ? " first" : ""}`);
      delivery.reply({ type: "shown" });
    },
    opened: () => events.push("opened"),
    reconnecting: () => events.push("reconnecting"),
    closed: () => events.push("closed"),
  });

  return { events, stop };
};

const latest = () => everySocket().at(-1) ?? new FakeSocket("ws://daemon.test/none");

beforeEach(() => {
  forgetSockets();
  vi.stubGlobal("WebSocket", FakeSocket);
  vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
});

afterEach(() => {
  vi.useRealTimers();
});

const reconnectedAfter = () => {
  const count = everySocket().length;
  let waited = 0;

  while (everySocket().length === count && waited < 10_000) {
    vi.advanceTimersByTime(1);
    waited += 1;
  }

  return waited;
};

test("a dropped socket comes back after 500 ms, doubling up to 5 s, and an open one starts again at 500 ms", () => {
  followed();
  const waits = Array.from({ length: 6 }, () => {
    drop(latest());

    return reconnectedAfter();
  });

  accept(latest());
  drop(latest());
  expect([waits, reconnectedAfter()]).toEqual([[500, 1000, 2000, 4000, 5000, 5000], 500]);
});

test("messages carry whether they are first and reply only on the open, current socket", () => {
  const { events } = followed();
  const first = latest();

  accept(first);
  deliver(first, { type: "frame" });
  deliver(first, { type: "frame" });
  first.readyState = FakeSocket.CLOSING;
  deliver(first, { type: "frame" });
  drop(first);
  vi.advanceTimersByTime(500);
  deliver(first, { type: "frame" });
  expect([events, first.sent]).toEqual([["opened", "frame first", "frame", "frame", "reconnecting"], [{ type: "shown" }, { type: "shown" }]]);
});

test("a socket that opens after it was replaced is ignored", () => {
  const { events, stop } = followed();
  const stale = latest();

  stop();
  accept(stale);
  expect(events).toEqual([]);
});

test("stopping closes the socket and cancels a pending reconnection", () => {
  const { events, stop } = followed();
  const open = latest();

  accept(open);
  stop();
  expect([open.readyState, events]).toEqual([FakeSocket.CLOSED, ["opened"]]);
  const again = followed();

  drop(latest());
  again.stop();
  vi.advanceTimersByTime(10_000);
  expect(everySocket().length).toBe(2);
});

test("a feed the daemon closed stays closed", () => {
  const { events } = followed();

  accept(latest());
  deliver(latest(), { type: "closed" });
  drop(latest());
  vi.advanceTimersByTime(10_000);
  expect([events, everySocket().length]).toEqual([["opened", "closed"], 1]);
});
