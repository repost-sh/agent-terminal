import { cleanup } from "@testing-library/react";
import { afterEach, expect } from "vitest";

afterEach(() => {
  const open = document.querySelector('[role="dialog"], [role="alertdialog"]');

  cleanup();
  history.replaceState(null, "", "/");
  expect(open?.getAttribute("aria-labelledby") ?? null, "close the dialog the test opened").toBeNull();
});
