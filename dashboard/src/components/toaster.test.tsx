import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import { toast } from "../lib/toasts";
import { Toaster } from "./toaster";

const shown = () => Array.from(screen.getByRole("region", { name: "Notifications" }).querySelectorAll(".toast")).map((entry) => entry.textContent);

test("the newest three toasts show, errors as alerts, and each one closes", () => {
  render(<Toaster />);
  act(() => {
    toast("info", { title: "one" });
    toast("success", { title: "two", description: "detail" });
    toast("warning", { title: "three" });
    toast("error", { title: "four" });
  });
  expect(shown()).toEqual(["four", "three", "twodetail"]);
  expect(screen.getByRole("alert").textContent).toBe("four");
  fireEvent.click(within(screen.getByRole("alert")).getByRole("button", { name: "Close notification" }));
  expect(shown()).toEqual(["three", "twodetail"]);
  for (const button of screen.getAllByRole("button", { name: "Close notification" })) {
    fireEvent.click(button);
  }

  expect(shown()).toEqual([]);
});

test("information leaves after five seconds and problems after nine", () => {
  vi.useFakeTimers();
  render(<Toaster />);
  act(() => {
    toast("success", { title: "saved" });
    toast("warning", { title: "careful" });
  });
  act(() => {
    vi.advanceTimersByTime(4999);
  });
  expect(shown()).toEqual(["careful", "saved"]);
  act(() => {
    vi.advanceTimersByTime(1);
  });
  expect(shown()).toEqual(["careful"]);
  act(() => {
    vi.advanceTimersByTime(4000);
  });
  expect(shown()).toEqual([]);
  vi.useRealTimers();
});
