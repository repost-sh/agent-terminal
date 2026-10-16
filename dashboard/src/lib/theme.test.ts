import { act, renderHook } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";

const STORED_AS = "agent-terminal-theme";

const themed = async (stored: string | null) => {
  if (stored !== null) {
    localStorage.setItem(STORED_AS, stored);
  }

  const { useTheme } = await import("./theme");

  return renderHook(() => useTheme());
};

const page = () => [localStorage.getItem(STORED_AS), document.documentElement.classList.contains("dark"), document.documentElement.style.colorScheme];

beforeEach(() => {
  vi.resetModules();
  localStorage.clear();
});

test("the theme starts dark unless light was chosen before", async () => {
  expect([(await themed(null)).result.current.theme]).toEqual(["dark"]);
  vi.resetModules();
  expect([(await themed("sepia")).result.current.theme]).toEqual(["dark"]);
  vi.resetModules();
  expect([(await themed("light")).result.current.theme]).toEqual(["light"]);
});

test("toggling flips the theme on the page and remembers it", async () => {
  const hook = await themed("light");

  act(() => {
    hook.result.current.toggle();
  });
  expect([hook.result.current.theme, ...page()]).toEqual(["dark", "dark", true, "dark"]);
  act(() => {
    hook.result.current.toggle();
  });
  expect([hook.result.current.theme, ...page()]).toEqual(["light", "light", false, "light"]);
});
