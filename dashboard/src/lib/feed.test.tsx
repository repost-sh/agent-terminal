import { render, renderHook } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";

import { startDaemon } from "../fake-daemon/daemon";
import { everySocket, FakeSocket, openAt } from "../fake-daemon/sockets";
import type { FeedSpec } from "./feed";

const PATH = "/api/feed";

interface Counted { type: string; n: number }

const SUM: FeedSpec<number, Counted> = { initial: 0, reduce: (total, received) => total + received.message.n };

const feed = async () => {
  const { useFeed } = await import("./feed");
  const daemon = startDaemon([]);

  return { useFeed, daemon };
};

beforeEach(() => {
  vi.resetModules();
});

test("a feed reduces its messages, starting over from each first one after a reconnection", async () => {
  const { useFeed, daemon } = await feed();
  const hook = renderHook(() => useFeed(PATH, SUM));

  expect(hook.result.current).toEqual({ status: "connecting", value: 0 });
  await daemon.connected(PATH);
  await daemon.push(PATH, { type: "count", n: 2 });
  await daemon.push(PATH, { type: "count", n: 3 });
  expect(hook.result.current).toEqual({ status: "open", value: 5 });
  await daemon.hangUp(PATH);
  expect(hook.result.current).toEqual({ status: "connecting", value: 5 });
  await daemon.connected(PATH);
  await daemon.push(PATH, { type: "count", n: 4 });
  expect(hook.result.current).toEqual({ status: "open", value: 4 });
  await daemon.push(PATH, { type: "closed" });
  expect(hook.result.current).toEqual({ status: "closed", value: 4 });
  hook.unmount();
});

test("a feed nobody follows closes its socket, and the next follower starts afresh", async () => {
  const { useFeed, daemon } = await feed();
  const first = renderHook(() => useFeed(PATH, SUM));

  await daemon.connected(PATH);
  await daemon.push(PATH, { type: "count", n: 2 });
  first.unmount();
  expect(openAt(PATH)).toEqual([]);
  const second = renderHook(() => useFeed(PATH, SUM));

  expect(second.result.current).toEqual({ status: "connecting", value: 0 });
  second.unmount();
});

test("a feed leaving does not take the place of another feed of the same path", async () => {
  const { useFeed, daemon } = await feed();
  const Follower = () => {
    useFeed(PATH, SUM);

    return null;
  };

  const both = render(
    <>
      <Follower key="a" />
      <Follower key="b" />
    </>,
  );

  await daemon.connected(PATH);
  both.rerender(
    <>
      <Follower key="b" />
    </>,
  );
  const third = renderHook(() => useFeed(PATH, SUM));

  expect(everySocket().filter((socket) => socket.path === PATH && socket.readyState !== FakeSocket.CLOSED)).toHaveLength(1);
  third.unmount();
  both.unmount();
});
