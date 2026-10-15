import { useSyncExternalStore } from "react";

import { socketUrl } from "./api";
import { followSocket } from "./socket";

export type FeedStatus = "connecting" | "open" | "closed";

export interface FeedState<T> { status: FeedStatus; value: T }

interface Message { type: string }

export interface Received<M> { message: M; first: boolean }

type Reduce<T, M> = (value: T, received: Received<M>) => T;

export interface FeedSpec<T, M> { initial: T; reduce: Reduce<T, M> }

interface Feed<T> { subscribe: (listener: () => void) => () => void; get: () => FeedState<T> }

const feeds = new Map<string, Feed<unknown>>();

interface Publisher<T> { state: () => FeedState<T>; publish: (next: FeedState<T>) => void }

const follow = <T, M extends Message>(path: string, feed: FeedSpec<T, M> & Publisher<T>) =>
  followSocket<M>(socketUrl(path), {
    message: (message, delivery) => {
      const first = delivery.first;

      feed.publish({ status: "open", value: feed.reduce(first ? feed.initial : feed.state().value, { message, first }) });
    },
    reconnecting: () => {
      if (feed.state().status === "open") {
        feed.publish({ ...feed.state(), status: "connecting" });
      }
    },
    closed: () => {
      feed.publish({ ...feed.state(), status: "closed" });
    },
  });

const createFeed = <T, M extends Message>(path: string, spec: FeedSpec<T, M>): Feed<T> => {
  let state: FeedState<T> = { status: "connecting", value: spec.initial };
  const listeners = new Set<() => void>();
  let stop: (() => void) | null = null;
  const publish = (next: FeedState<T>) => {
    state = next;
    listeners.forEach((listener) => {
      listener();
    });
  };

  const feed: Feed<T> = {
    subscribe: (listener) => {
      listeners.add(listener);
      feeds.set(path, feed);
      if (listeners.size === 1) {
        stop = follow(path, { ...spec, state: () => state, publish });
      }

      return () => {
        listeners.delete(listener);
        if (listeners.size > 0) {
          return;
        }

        stop?.();
        if (feeds.get(path) === feed) {
          feeds.delete(path);
        }
      };
    },
    get: () => state,
  };

  return feed;
};

export const useFeed = <T, M extends Message>(path: string, spec: FeedSpec<T, M>): FeedState<T> => {
  const feed = (feeds.get(path) as Feed<T> | undefined) ?? createFeed(path, spec);

  return useSyncExternalStore(feed.subscribe, feed.get);
};
