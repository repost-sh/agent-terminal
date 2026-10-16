import { useSyncExternalStore } from "react";

type Unwatch = () => void;

export interface Signal { notify: () => void; subscribe: (listener: () => void) => () => void }

export interface Store<T> { get: () => T; set: (next: T) => void; subscribe: Signal["subscribe"] }

export const createSignal = (watch?: () => Unwatch): Signal => {
  const listeners = new Set<() => void>();
  let unwatch: Unwatch | undefined;

  return {
    notify: () => {
      listeners.forEach((listener) => {
        listener();
      });
    },
    subscribe: (listener) => {
      listeners.add(listener);
      if (listeners.size === 1) {
        unwatch = watch?.();
      }

      return () => {
        listeners.delete(listener);
        if (listeners.size === 0) {
          unwatch?.();
        }
      };
    },
  };
};

export const createStore = <T>(initial: T, watch?: (store: Store<T>) => Unwatch): Store<T> => {
  let value = initial;
  const signal = createSignal(watch && (() => watch(store)));
  const store: Store<T> = {
    get: () => value,
    set: (next) => {
      value = next;
      signal.notify();
    },
    subscribe: signal.subscribe,
  };

  return store;
};

export const useStore = <T>(store: Store<T>) => useSyncExternalStore(store.subscribe, store.get);
