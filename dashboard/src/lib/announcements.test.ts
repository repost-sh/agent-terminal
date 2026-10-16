import { beforeEach, expect, test, vi } from "vitest";

beforeEach(() => {
  vi.resetModules();
});

test("nothing is said at first, and each announcement counts once even when the words repeat", async () => {
  const { announce, announcements } = await import("./announcements");

  expect(announcements.get()).toEqual({ text: "", times: 0 });
  announce("Connected");
  announce("Connected");
  expect(announcements.get()).toEqual({ text: "Connected", times: 2 });
});
