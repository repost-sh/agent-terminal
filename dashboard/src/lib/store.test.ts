import { expect, test, vi } from "vitest";

import { createSignal, createStore } from "./store";

test("a signal calls each subscriber until it unsubscribes", () => {
  const signal = createSignal();
  const [first, second] = [vi.fn(), vi.fn()];
  const leaveFirst = signal.subscribe(first);

  signal.subscribe(second);
  signal.notify();
  leaveFirst();
  signal.notify();
  expect([first.mock.calls.length, second.mock.calls.length]).toEqual([1, 2]);
});

test("a signal is watched from its first subscriber until its last one leaves, and again after", () => {
  const unwatch = vi.fn();
  const watch = vi.fn(() => unwatch);
  const signal = createSignal(watch);
  const leaveFirst = signal.subscribe(vi.fn());
  const leaveSecond = signal.subscribe(vi.fn());

  expect([watch.mock.calls.length, unwatch.mock.calls.length]).toEqual([1, 0]);
  leaveFirst();
  expect(unwatch).not.toHaveBeenCalled();
  leaveSecond();
  expect(unwatch).toHaveBeenCalledOnce();
  signal.subscribe(vi.fn());
  expect(watch).toHaveBeenCalledTimes(2);
});

test("a store keeps its value, tells its subscribers of each new one, and hands itself to its watcher", () => {
  const listener = vi.fn();
  const unwatch = vi.fn();
  const store = createStore(1, (watched) => {
    watched.set(watched.get() + 1);

    return unwatch;
  });

  expect(store.get()).toBe(1);
  const leave = store.subscribe(listener);

  expect([store.get(), listener.mock.calls.length]).toEqual([2, 1]);
  store.set(5);
  expect([store.get(), listener.mock.calls.length]).toEqual([5, 2]);
  leave();
  expect(unwatch).toHaveBeenCalledOnce();
});

test("a store with no watcher still notifies", () => {
  const store = createStore("a");
  const listener = vi.fn();

  store.subscribe(listener)();
  store.subscribe(listener);
  store.set("b");
  expect([store.get(), listener.mock.calls.length]).toEqual(["b", 1]);
});
