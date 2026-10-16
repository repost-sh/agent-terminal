import assert from "node:assert/strict";
import { writeFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";

import type { MutantResult, MutantStatus } from "@stryker-mutator/api/core";

import { configFilesIn, judge } from "./mutation.ts";
import { scratchFolder } from "./scratch.ts";

const FIX = "write a test whose expected value changes when the mutated code does → GAUNTLET.md#mutation";

const mutant = (status: MutantStatus, line = 1): MutantResult => ({
  id: String(line),
  fileName: "src/lib/format.ts",
  mutatorName: "EqualityOperator",
  replacement: "a !== b",
  location: { start: { line, column: 3 }, end: { line, column: 9 } },
  status,
});

void test("every tested mutant caught passes, and mutants that did not compile are not counted as tested", () => {
  assert.deepEqual(judge([mutant("Killed"), mutant("RuntimeError"), mutant("CompileError"), mutant("Killed")]), {
    lines: ["3 mutants tested: every one caught (1 did not compile)"],
    passed: true,
  });
});

void test("a survivor, a mutant no test ran and an ignored mutant are each an offense naming the mutation", () => {
  assert.deepEqual(judge([mutant("Killed"), mutant("Survived", 4), mutant("NoCoverage", 7), mutant("Ignored", 9)]), {
    lines: [
      `src/lib/format.ts:4:3: EqualityOperator → "a !== b" was not caught: ${FIX}`,
      `src/lib/format.ts:7:3: EqualityOperator → "a !== b" ran under no test: ${FIX}`,
      `src/lib/format.ts:9:3: EqualityOperator → "a !== b" was not caught: ${FIX}`,
      "3 offense(s)",
    ],
    passed: false,
  });
});

void test("a run that tested nothing, or timed out on more than one mutant in twenty, fails", () => {
  assert.deepEqual(judge([mutant("CompileError"), mutant("Pending")]), {
    lines: ["no mutant was tested: point the gate at the dashboard's source → GAUNTLET.md#mutation", "1 offense(s)"],
    passed: false,
  });
  const killed = Array.from({ length: 19 }, () => mutant("Killed"));

  assert.equal(judge([...killed, mutant("Timeout")]).passed, true);
  assert.deepEqual(judge([...killed.slice(1), mutant("Timeout"), mutant("Timeout")]), {
    lines: ["2 of 20 mutants timed out, over 5%: make the slow tests fast → GAUNTLET.md#mutation", "1 offense(s)"],
    passed: false,
  });
});

void test("a change whose only mutants did not compile, or that has none, has nothing to catch", () => {
  assert.deepEqual(judge([mutant("CompileError")]), { lines: ["0 mutants tested: every one caught (1 did not compile)"], passed: true });
  assert.deepEqual(judge([]), { lines: ["0 mutants tested: every one caught (0 did not compile)"], passed: true });
});

void test("every config file name Stryker would read is found", () => {
  const root = scratchFolder("stryker-");

  for (const file of ["stryker.conf.json", ".stryker.config.mjs", "stryker.config.cjs", "stryker.json", "package.json"]) {
    writeFileSync(join(root, file), "{}");
  }

  assert.deepEqual(configFilesIn(root), [".stryker.config.mjs", "stryker.conf.json", "stryker.config.cjs"]);
});
