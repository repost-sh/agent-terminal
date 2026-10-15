import { expect, test } from "vitest";

import { settle } from "./settle";

test("settle turns a value, a resolved promise, a throw and a rejection into results", async () => {
  const failure = new Error("no");

  expect(await settle(() => 1)).toEqual({ ok: true, value: 1 });
  expect(await settle(() => Promise.resolve("later"))).toEqual({ ok: true, value: "later" });
  expect(
    await settle(() => {
      throw failure;
    }),
  ).toEqual({ ok: false, error: failure });
  expect(await settle(() => Promise.reject(failure))).toEqual({ ok: false, error: failure });
});
