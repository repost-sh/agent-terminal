import { expect, test } from "vitest";

import { startDaemon } from "../fake-daemon/daemon";
import { shotOf } from "../fake-daemon/messages";
import { comparisonOf } from "./screenshots";

test("a pair of screenshots is compared once, and a different pair or session again", async () => {
  const daemon = startDaemon([]);
  const pair = { older: shotOf("a.png"), newer: shotOf("b.png") };
  const first = comparisonOf("work", pair);
  const others = [
    comparisonOf("play", pair),
    comparisonOf("work", { ...pair, newer: { ...pair.newer, time: "2026-01-01T00:00:00Z" } }),
    comparisonOf("work", { ...pair, older: shotOf("c.png") }),
  ];

  expect(comparisonOf("work", { ...pair })).toBe(first);
  expect(others.filter((other) => other === first)).toEqual([]);
  await Promise.all([first, ...others]);
  expect(daemon.requests.length).toBe(4);
});
