import { act, renderHook } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";

beforeEach(() => {
  vi.resetModules();
});

test("the side panel opens on Activity at 30% wide, and remembers the panel and width it is given", async () => {
  const panel = await import("./side-panel");
  const hook = renderHook(() => panel.useSidePanel());

  expect(hook.result.current).toEqual({ panel: "activity", width: 30 });
  act(() => {
    panel.showPanel("output");
  });
  act(() => {
    panel.resizePanel(45);
  });
  expect(hook.result.current).toEqual({ panel: "output", width: 45 });
});
