import { expect, test } from "vitest";

import { daemonWidths } from "./unicode";
import { WIDE } from "./widths/wide";
import { ZERO } from "./widths/zero";

const widthOf = (text: string) => daemonWidths.wcwidth(text.codePointAt(0) ?? 0);

const properties = (codepoint: number, preceding: number) => daemonWidths.charProperties(codepoint, preceding);

test("the widths are registered under the daemon's Unicode version", () => {
  expect(daemonWidths.version).toBe("17.0.0");
});

test("characters are as wide as the daemon's table says, in and beyond the basic plane", () => {
  expect(["a", "中", "́", "‍", "😀", "\u{1F3FB}", "\u{E0001}", "𝐀"].map(widthOf)).toEqual([1, 2, 0, 0, 2, 2, 0, 1]);
});

test("a zero-width mark joins the character before it and takes its width", () => {
  const wide = properties(0x4e2d, 0);
  const narrow = properties(0x61, 0);

  expect([wide, narrow]).toEqual([4, 2]);
  expect(properties(0x301, wide)).toBe(5);
  expect(properties(0x301, narrow)).toBe(3);
  expect(properties(0x301, 0)).toBe(0);
  expect(properties(0x301, properties(0x301, 0))).toBe(0);
  expect(properties(0x61, wide)).toBe(2);
});

test("both ends of every range in the daemon's tables have the range's width, and zero width wins", () => {
  const zero = (codepoint: number) => ZERO.some(([start, end]) => start <= codepoint && codepoint <= end);
  const ends = (ranges: typeof WIDE) => ranges.flatMap(([start, end]) => [start, end]);

  expect(ends(ZERO).filter((codepoint) => daemonWidths.wcwidth(codepoint) !== 0)).toEqual([]);
  expect(ends(WIDE).filter((codepoint) => daemonWidths.wcwidth(codepoint) !== (zero(codepoint) ? 0 : 2))).toEqual([]);
});
