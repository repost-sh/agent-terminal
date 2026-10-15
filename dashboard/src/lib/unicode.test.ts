import { expect, test } from "vitest";

import { daemonWidths } from "./unicode";

const widthOf = (text: string) => daemonWidths.wcwidth(text.codePointAt(0) ?? 0);

const properties = (codepoint: number, preceding: number) => daemonWidths.charProperties(codepoint, preceding);

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
});
