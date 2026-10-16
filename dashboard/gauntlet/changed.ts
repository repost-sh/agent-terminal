import { matchesGlob } from "node:path";

import { TEST_CODE } from "./test-code.ts";

const PRODUCT = "src/**/*.{ts,tsx}";

const FILE = /^\+\+\+ (?:b\/(.+)|\/dev\/null)$/;

const HUNK = /^@@ -\d+(?:,\d+)? \+(\d+)(?:,\d+)? @@/;

const isProduct = (path: string) => matchesGlob(path, PRODUCT) && !TEST_CODE.some((pattern) => matchesGlob(path, pattern));

interface Added { path: string; line: number }

interface Reading { path: string | null; line: number; added: Added[] }

const header = (reading: Reading, text: string): Reading | null => {
  const file = FILE.exec(text);

  if (file) {
    return { ...reading, path: file[1] ?? null };
  }

  const hunk = HUNK.exec(text);

  return hunk ? { ...reading, line: Number(hunk[1]) } : null;
};

const body = (reading: Reading, text: string): Reading => {
  if (text.startsWith("+") && reading.path !== null) {
    reading.added.push({ path: reading.path, line: reading.line });

    return { ...reading, line: reading.line + 1 };
  }

  return text.startsWith(" ") ? { ...reading, line: reading.line + 1 } : reading;
};

const addedLines = (diff: string) =>
  diff.split("\n").reduce<Reading>((reading, text) => header(reading, text) ?? body(reading, text), { path: null, line: 0, added: [] }).added;

interface Range { path: string; first: number; last: number }

export const changedRanges = (diff: string): string[] => {
  const ranges: Range[] = [];

  for (const added of addedLines(diff).filter((line) => isProduct(line.path))) {
    const last = ranges.at(-1);

    if (last?.path === added.path && last.last + 1 === added.line) {
      last.last = added.line;
    } else {
      ranges.push({ path: added.path, first: added.line, last: added.line });
    }
  }

  return ranges.map((range) => `${range.path}:${String(range.first)}-${String(range.last)}`);
};
