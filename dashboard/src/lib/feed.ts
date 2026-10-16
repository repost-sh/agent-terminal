import { socketUrl } from "./api";
import { followSocket } from "./socket";
import { createStore, useStore, type Store } from "./store";

export type FeedStatus = "connecting" | "open" | "closed";

export interface FeedState<T> { status: FeedStatus; value: T }

interface Message { type: string }

export interface Received<M> { message: M; first: boolean }

type Reduce<T, M> = (value: T, received: Received<M>) => T;

export interface FeedSpec<T, M> { initial: T; reduce: Reduce<T, M> }

const feeds = new Map<string, Store<FeedState<unknown>>>();

interface Publisher<T> { state: () => FeedState<T>; publish: (next: FeedState<T>) => void }

const follow = <T, M extends Message>(path: string, feed: FeedSpec<T, M> & Publisher<T>) =>
  followSocket<M>(socketUrl(path), {
    message: (message, delivery) => {
      const first = delivery.first;

      feed.publish({ status: "open", value: feed.reduce(first ? feed.initial : feed.state().value, { message, first }) });
    },
    reconnecting: () => {
      feed.publish({ ...feed.state(), status: "connecting" });
    },
    closed: () => {
      feed.publish({ ...feed.state(), status: "closed" });
    },
  });

const createFeed = <T, M extends Message>(path: string, spec: FeedSpec<T, M>): Store<FeedState<T>> => {
  const feed = createStore<FeedState<T>>({ status: "connecting", value: spec.initial }, (watched) => {
    feeds.set(path, feed as Store<FeedState<unknown>>);
    const stop = follow(path, { ...spec, state: watched.get, publish: watched.set });

    return () => {
      stop();
      if (feeds.get(path) === feed) {
        feeds.delete(path);
      }
    };
  });

  return feed;
};

export const useFeed = <T, M extends Message>(path: string, spec: FeedSpec<T, M>): FeedState<T> =>
  useStore((feeds.get(path) as Store<FeedState<T>> | undefined) ?? createFeed(path, spec));
