import { act, renderHook } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";

const STORED_AS = "agent-terminal-mouse";

const preference = async () => {
  vi.resetModules();
  const lib = await import("./mouse-preference");
  const hook = renderHook(() => lib.useMouseWanted());

  return { ...lib, wanted: () => hook.result.current };
};

beforeEach(() => {
  localStorage.clear();
});

test("the mouse goes to the app until it is switched off, and the switch is remembered across loads", async () => {
  const fresh = await preference();

  expect(fresh.wanted()).toBe(true);
  act(fresh.toggleMouse);
  expect([fresh.wanted(), localStorage.getItem(STORED_AS)]).toEqual([false, "off"]);

  const reloaded = await preference();

  expect(reloaded.wanted()).toBe(false);
  act(reloaded.toggleMouse);
  expect([reloaded.wanted(), localStorage.getItem(STORED_AS)]).toEqual([true, "on"]);
});
