import { expect, test } from "vitest";

import { clockMs, clockTime, dateTime, formatDuration, shellWord, shortPath } from "./format";

test("durations read in milliseconds, then seconds, then minutes", () => {
  expect([0.4, 999, 1000, 1049, 59_949, 60_000, 125_000, 3_725_000].map(formatDuration)).toEqual([
    "0 ms",
    "999 ms",
    "1.0 s",
    "1.0 s",
    "59.9 s",
    "1 min 00 s",
    "2 min 05 s",
    "62 min 05 s",
  ]);
});

test("clock times are local, padded and keep milliseconds when asked", () => {
  const at = new Date(2026, 0, 2, 3, 4, 5, 7);

  expect(clockTime(at.getTime())).toBe("03:04:05");
  expect(clockMs(at.toISOString())).toBe("03:04:05.007");
  expect(dateTime(at.toISOString())).toBe("Jan 2, 2026 03:04:05");
  expect(dateTime(new Date(2025, 11, 31, 23, 59, 58).toISOString())).toBe("Dec 31, 2025 23:59:58");
});

test("paths shorten every folder but the first and last to its initial", () => {
  expect(shortPath("/home/me/projects/app", "/home/me")).toBe("~/p/app");
  expect(shortPath("/home/me", "/home/me")).toBe("~");
  expect(shortPath("/home/meadow/app", "/home/me")).toBe("/h/m/app");
  expect(shortPath("/srv/.config/data/app", null)).toBe("/s/.c/d/app");
  expect(shortPath("/srv/ünïcode/émoji/app", null)).toBe("/s/ü/é/app");
  expect(shortPath("relative//app", null)).toBe("relative//app");
});

test("shell words are quoted only when the shell would split or expand them", () => {
  expect(["plain-word_1.2", "user@host:path/a,b=c+d%", "two words", "it's", "$HOME", ""].map(shellWord)).toEqual([
    "plain-word_1.2",
    "user@host:path/a,b=c+d%",
    "'two words'",
    "'it'\\''s'",
    "'$HOME'",
    "''",
  ]);
});
