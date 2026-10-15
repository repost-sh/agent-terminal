import { useSyncExternalStore } from "react";

import { socketUrl } from "./api";
import { followSocket } from "./socket";

export type FeedStatus = "connecting" | "open" | "closed";

export type FeedState<T> = { status: FeedStatus; value: T };

type Message = { type: string };

type Reduce<T, M> = (value: T, message: M, first: boolean) => T;

type Feed<T> = { subscribe: (listener: () => void) => () => void; get: () => FeedState<T> };

const feeds = new Map<string, Feed<unknown>>();

const createFeed = <T, M extends Message>(path: string, initial: T, reduce: Reduce<T, M>): Feed<T> => {
  let state: FeedState<T> = { status: "connecting", value: initial };
  const listeners = new Set<() => void>();
  let stop = () => {};

  const publish = (next: FeedState<T>) => {
    state = next;
    listeners.forEach((listener) => listener());
  };

  const feed: Feed<T> = {
    subscribe: (listener) => {
      listeners.add(listener);
      feeds.set(path, feed as Feed<unknown>);
      if (listeners.size === 1) {
        stop = followSocket<M>(socketUrl(path), {
          message: (message, first) => publish({ status: "open", value: reduce(first ? initial : state.value, message as M, first) }),
          reconnecting: () => {
            if (state.status === "open") publish({ ...state, status: "connecting" });
          },
          closed: () => publish({ ...state, status: "closed" }),
        });
      }
      return () => {
        listeners.delete(listener);
        if (listeners.size > 0) return;
        stop();
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
