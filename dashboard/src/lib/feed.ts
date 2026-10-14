import { useSyncExternalStore } from "react";

import { socketUrl } from "./api";

export type FeedStatus = "connecting" | "open" | "closed";

export type FeedState<T> = { status: FeedStatus; value: T };

type Message = { type: string };

type Reduce<T, M> = (value: T, message: M, first: boolean) => T;

type Feed<T> = { subscribe: (listener: () => void) => () => void; get: () => FeedState<T> };

const RECONNECT_AFTER_MS = (failures: number) => Math.min(500 * 2 ** failures, 5000);

const feeds = new Map<string, Feed<unknown>>();

const createFeed = <T, M extends Message>(path: string, initial: T, reduce: Reduce<T, M>): Feed<T> => {
  let state: FeedState<T> = { status: "connecting", value: initial };
  const listeners = new Set<() => void>();
  let socket: WebSocket | null = null;
  let timer: number | undefined;
  let failures = 0;

  const publish = (next: FeedState<T>) => {
    state = next;
    listeners.forEach((listener) => listener());
  };

  const connect = () => {
    const opening = new WebSocket(socketUrl(path));
    socket = opening;
    let first = true;
    opening.onmessage = (event: MessageEvent<string>) => {
      if (socket !== opening) return;
      const message = JSON.parse(event.data) as M;
      failures = 0;
      if (message.type === "closed") {
        publish({ ...state, status: "closed" });
        return;
      }
      publish({ status: "open", value: reduce(first ? initial : state.value, message, first) });
      first = false;
    };
    opening.onclose = () => {
      if (socket !== opening || state.status === "closed") return;
      if (state.status === "open") publish({ ...state, status: "connecting" });
      timer = window.setTimeout(connect, RECONNECT_AFTER_MS(failures));
      failures += 1;
    };
  };

  const feed: Feed<T> = {
    subscribe: (listener) => {
      listeners.add(listener);
      feeds.set(path, feed as Feed<unknown>);
      if (listeners.size === 1) connect();
      return () => {
        listeners.delete(listener);
        if (listeners.size > 0) return;
        window.clearTimeout(timer);
        const open = socket;
        socket = null;
        open?.close();
        if (feeds.get(path) === feed) feeds.delete(path);
      };
    },
    get: () => state,
  };
  return feed;
};

export const useFeed = <T, M extends Message>(path: string, initial: T, reduce: Reduce<T, M>): FeedState<T> => {
  const feed = (feeds.get(path) as Feed<T> | undefined) ?? createFeed(path, initial, reduce);
  return useSyncExternalStore(feed.subscribe, feed.get);
};
