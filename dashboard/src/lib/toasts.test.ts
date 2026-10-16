import { afterEach, beforeEach, expect, test, vi } from "vitest";

const store = () => import("./toasts");

beforeEach(() => {
  vi.resetModules();
  vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
});

afterEach(() => {
  vi.useRealTimers();
});

const titles = async () => (await store()).toasts.get().map((shown) => shown.title);

test("an error stays nine seconds", async () => {
  const { toast } = await store();

  toast("error", { title: "error" });
  vi.advanceTimersByTime(8999);
  expect(await titles()).toEqual(["error"]);
  vi.advanceTimersByTime(1);
  expect(await titles()).toEqual([]);
});

test("errors and warnings stay nine seconds, other toasts five", async () => {
  const { toast } = await store();

  toast("error", { title: "error" });
  toast("warning", { title: "warning", description: "careful" });
  vi.advanceTimersByTime(4999);
  toast("success", { title: "success" });
  toast("info", { title: "info" });
  expect(await titles()).toEqual(["info", "success", "warning"]);
  vi.advanceTimersByTime(4001);
  expect(await titles()).toEqual(["info", "success"]);
  vi.advanceTimersByTime(998);
  expect(await titles()).toEqual(["info", "success"]);
  vi.advanceTimersByTime(1);
  expect(await titles()).toEqual([]);
});

test("a toast keeps its type and words, and dismissing one leaves the others", async () => {
  const shown = await store();

  shown.toast("warning", { title: "first", description: "why" });
  shown.toast("success", { title: "second" });
  const listed = shown.toasts.get();

  expect(listed.map((each) => [each.type, each.title, each.description])).toEqual([
    ["success", "second", ""],
    ["warning", "first", "why"],
  ]);
  expect(new Set(listed.map((each) => each.id)).size).toBe(2);
  listed.slice(0, 1).forEach((each) => {
    shown.dismiss(each.id);
  });
  expect(await titles()).toEqual(["first"]);
});
