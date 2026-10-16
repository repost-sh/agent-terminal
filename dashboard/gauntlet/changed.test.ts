import assert from "node:assert/strict";
import { test } from "node:test";

import { changedRanges } from "./changed.ts";

const diff = [
  "diff --git a/src/lib/format.ts b/src/lib/format.ts",
  "index 1111111..2222222 100644",
  "--- a/src/lib/format.ts",
  "+++ b/src/lib/format.ts",
  "@@ -3,4 +3,6 @@ export const a = 1;",
  " const b = 2;",
  "-const c = 3;",
  "+const c = 4;",
  "+const d = 5;",
  " const e = 6;",
  "+const f = 7;",
  " const g = 8;",
  "@@ -20 +22 @@ export const z = 1;",
  "-old",
  "\\ No newline at end of file",
  "+new",
  "\\ No newline at end of file",
  "diff --git a/src/lib/format.test.ts b/src/lib/format.test.ts",
  "--- a/src/lib/format.test.ts",
  "+++ b/src/lib/format.test.ts",
  "@@ -1 +1 @@",
  "-a",
  "+b",
  "diff --git a/src/gone.ts b/src/gone.ts",
  "--- a/src/gone.ts",
  "+++ /dev/null",
  "@@ -1 +0,0 @@",
  "-gone",
  "diff --git a/README.md b/README.md",
  "--- a/README.md",
  "+++ b/README.md",
  "@@ -1 +1 @@",
  "+text",
].join("\n");

void test("each run of added product lines becomes one range Stryker can mutate", () => {
  assert.deepEqual(changedRanges(diff), ["src/lib/format.ts:4-5", "src/lib/format.ts:7-7", "src/lib/format.ts:22-22"]);
});

void test("a diff that adds no product line names nothing to mutate", () => {
  assert.deepEqual(changedRanges(""), []);
  assert.deepEqual(changedRanges(diff.split("diff --git a/src/lib/format.test.ts")[1] ?? ""), []);
});
