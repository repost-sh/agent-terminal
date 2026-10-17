import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach, expect, vi } from "vitest";

afterEach(() => {
  const open = document.querySelector('[role="dialog"], [role="alertdialog"]');

  cleanup();
  vi.useRealTimers();
  history.replaceState(null, "", "/");
  expect(open?.getAttribute("aria-labelledby") ?? null, "close the dialog the test opened").toBeNull();
});
