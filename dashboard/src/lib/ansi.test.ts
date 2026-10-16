import fc from "fast-check";
import { expect, test } from "vitest";

import { moreStyled, runStyle, styledLines, UNSTYLED, type Run } from "./ansi";

const sgr = (params: string) => `\x1b[${params}m`;

const styleAfter = (params: string) => styledLines(`${sgr(params)}x`)[0]?.[0]?.style;

test("a CSI that is not a colour sequence stays in the text, even when an m follows later", () => {
  expect(styledLines("a\x1b[2Kbm")).toEqual([[{ text: "a\x1b[2Kbm", style: {} }]]);
});

test("plain text splits into lines of unstyled runs", () => {
  expect(styledLines("one\ntwo\n")).toEqual([[{ text: "one", style: {} }], [{ text: "two", style: {} }]]);
  expect(styledLines("")).toEqual([[]]);
  expect(styledLines("\n")).toEqual([[]]);
});

test("switches turn attributes on and off and reset clears them", () => {
  expect(["1", "2", "3", "4", "4:0", "4:3", "7", "8", "9", "21"].map(styleAfter)).toEqual([
    { bold: true },
    { dim: true },
    { italic: true },
    { underline: true },
    { underline: false },
    { underline: true },
    { inverse: true },
    { hidden: true },
    { strike: true },
    { underline: true },
  ]);
  expect(["1;22", "1;2;22", "3;23", "4;24", "7;27", "8;28", "9;29", "1;0", "1;", "31;39", "41;49", "1;5"].map(styleAfter)).toEqual([
    { bold: false, dim: false },
    { bold: false, dim: false },
    { italic: false },
    { underline: false },
    { inverse: false },
    { hidden: false },
    { strike: false },
    {},
    {},
    { fg: undefined },
    { bg: undefined },
    { bold: true },
  ]);
});

test("the sixteen colours map to the palette in both intensities", () => {
  expect(["30", "37", "40", "47", "90", "97", "100", "107"].map(styleAfter)).toEqual([
    { fg: "var(--ansi-0)" },
    { fg: "var(--ansi-7)" },
    { bg: "var(--ansi-0)" },
    { bg: "var(--ansi-7)" },
    { fg: "var(--ansi-8)" },
    { fg: "var(--ansi-15)" },
    { bg: "var(--ansi-8)" },
    { bg: "var(--ansi-15)" },
  ]);
});

test("extended colours read the 256 cube, the gray ramp and true colour in either syntax", () => {
  expect(["38;5;1", "38;5;16", "38;5;21", "38;5;231", "38;5;232", "48;5;255", "38;5;256", "38:5:196"].map(styleAfter)).toEqual([
    { fg: "var(--ansi-1)" },
    { fg: "rgb(0 0 0)" },
    { fg: "rgb(0 0 255)" },
    { fg: "rgb(255 255 255)" },
    { fg: "rgb(8 8 8)" },
    { bg: "rgb(238 238 238)" },
    { fg: undefined },
    { fg: "rgb(255 0 0)" },
  ]);
  expect(["38;2;1;2;3", "48:2::4:5:6", "38:2:7:8", "38:2:7", "38:5", "38:3:1:2:3", "38;9", "38", "38;2;1;2;3;1"].map(styleAfter)).toEqual([
    { fg: "rgb(1 2 3)" },
    { bg: "rgb(4 5 6)" },
    { fg: undefined },
    { fg: undefined },
    { fg: undefined },
    { fg: undefined },
    { fg: undefined, strike: true },
    { fg: undefined },
    { fg: "rgb(1 2 3)", bold: true },
  ]);
});

test("styles carry across chunks and lines and only SGR sequences are taken", () => {
  const first = moreStyled(UNSTYLED, `a${sgr("1")}b\nc`);
  const second = moreStyled(first, `d${sgr("0")}e\x1b[2Kf\x1b[`);

  expect(second.lines).toEqual([
    [
      { text: "a", style: {} },
      { text: "b", style: { bold: true } },
    ],
    [
      { text: "c", style: { bold: true } },
      { text: "d", style: { bold: true } },
      { text: "e\x1b[2Kf\x1b[", style: {} },
    ],
  ]);
  expect(second.style).toEqual({});
});

const textOf = (lines: Run[][]) => lines.map((line) => line.map((run) => run.text).join("")).join("\n");

test("text without escapes keeps every character", () => {
  fc.assert(
    fc.property(fc.string({ unit: "grapheme" }).filter((text) => !text.includes("\x1b")), (text) => {
      expect(textOf(styledLines(text))).toBe(text.endsWith("\n") ? text.slice(0, -1) : text);
    }),
  );
});

test("a run's CSS follows its style, swapping colours when inverse", () => {
  expect(runStyle({})).toEqual({});
  expect(runStyle({ fg: "red", bg: "blue", bold: true, dim: true, italic: true, underline: true, strike: true, hidden: true })).toEqual({
    color: "red",
    backgroundColor: "blue",
    fontWeight: 700,
    opacity: 0.6,
    fontStyle: "italic",
    textDecorationLine: "underline line-through",
    visibility: "hidden",
  });
  expect(runStyle({ inverse: true })).toEqual({ color: "var(--background)", backgroundColor: "var(--foreground)" });
  expect(runStyle({ inverse: true, fg: "red", bg: "blue" })).toEqual({ color: "blue", backgroundColor: "red" });
});
