import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";

import { ESLint } from "eslint";
import fc from "fast-check";
import tseslint from "typescript-eslint";

import { checkCrap, productFiles, scoreFile, scoreOf, type Coverage } from "./crap.ts";
import { functionsIn } from "./functions.ts";

const ROOT = join(import.meta.dirname, "..");

const complexitiesByEslint = async (files: string[]) => {
  const eslint = new ESLint({
    cwd: ROOT,
    overrideConfigFile: true,
    overrideConfig: [{ files: ["**/*.ts", "**/*.tsx"], languageOptions: { parser: tseslint.parser }, rules: { complexity: ["error", 0] } }],
  });
  const results = await eslint.lintFiles(files);

  return results.map((result) => result.messages.map((message) => Number(/complexity of (\d+)/.exec(message.message)?.[1])).sort());
};

void test("complexity is counted exactly as ESLint's complexity rule counts it, on every product file", async () => {
  const files = productFiles(ROOT);
  const ours = files.map((file) => functionsIn(readFileSync(join(ROOT, file), "utf8"), file).map((found) => found.complexity).sort());

  assert.ok(files.length > 40, `${String(files.length)} product files`);
  assert.deepEqual(ours, await complexitiesByEslint(files));
});

void test("CRAP equals the complexity when covered and grows as coverage falls", () => {
  fc.assert(
    fc.property(fc.integer({ min: 1, max: 30 }), fc.tuple(fc.double({ min: 0, max: 1, noNaN: true }), fc.double({ min: 0, max: 1, noNaN: true })), (complexity, shares) => {
      const [less, more] = [...shares].sort((a, b) => a - b);

      assert.equal(scoreOf(complexity, 1), complexity);
      assert.equal(scoreOf(complexity, 0), complexity * complexity + complexity);
      assert.ok(scoreOf(complexity, less ?? 0) >= scoreOf(complexity, more ?? 1));
    }),
  );
});

const NESTED = `export const outer = (items: number[]) => {
  if (items.length) {
    return items.map((item) => item * 2);
  }

  return [];
};
`;

void test("a statement counts for the innermost function around it, even on a shared line", () => {
  const functions = functionsIn(NESTED, "nested.ts");
  const file = {
    statementMap: { a: { start: { line: 2, column: 2 } }, b: { start: { line: 3, column: 4 } }, c: { start: { line: 3, column: 32 } }, d: { start: { line: 6, column: 2 } } },
    s: { a: 1, b: 1, c: 0, d: 0 },
  };
  const scored = scoreFile(functions, file);

  assert.deepEqual(
    scored.map((entry) => [entry.found.name, entry.found.complexity, entry.covered]),
    [
      ["an anonymous function", 1, 0],
      ["outer", 2, 2 / 3],
    ],
  );
  assert.deepEqual(scoreFile(functions, undefined).map((entry) => entry.covered), [0, 0]);
});

const SOURCES: Record<string, string> = {
  "src/lib/kept.ts": "export const kept = (a: boolean, b: boolean) => (a && b ? 1 : 2);\n",
  "src/lib/risky.ts": "export const risky = (a?: { b?: number }) => a?.b ?? (a ? 1 : 2);\n",
  "src/lib/risky.test.ts": "export const ignored = (a: boolean) => (a ? (a ? 1 : 2) : 3);\n",
  "src/fake-daemon/fake.ts": "export const ignored = (a: boolean) => (a ? (a ? 1 : 2) : 3);\n",
};

const scratchRoot = () => {
  const root = mkdtempSync(join(tmpdir(), "crap-"));

  for (const [path, code] of Object.entries(SOURCES)) {
    mkdirSync(dirname(join(root, path)), { recursive: true });
    writeFileSync(join(root, path), code);
  }

  return root;
};

void test("functions over the ceiling are offenses and test code is not scored", () => {
  const root = scratchRoot();
  const covered = (hits: number) => ({ statementMap: { a: { start: { line: 1, column: 50 } } }, s: { a: hits } });
  const coverage: Coverage = { [join(root, "src/lib/kept.ts")]: covered(1), [join(root, "src/lib/risky.ts")]: covered(0) };
  const verdict = checkCrap(root, coverage);

  assert.equal(verdict.passed, false);
  assert.deepEqual(verdict.lines, [
    "src/lib/risky.ts:1: risky has CRAP 20.0 (complexity 4, 0% of its statements covered), over 6: cover every branch through the dashboard's tests, or split it into simpler functions that are each covered → GAUNTLET.md#crap",
    "1 offense(s)",
  ]);
  coverage[join(root, "src/lib/risky.ts")] = covered(1);
  assert.deepEqual(checkCrap(root, coverage), { lines: ["2 functions at CRAP 6 or under"], passed: true });
});

void test("a file that does not parse is an error naming it", () => {
  assert.throws(() => functionsIn("export const = ;\n", "broken.ts"), /^Error: broken\.ts:1: /);
});

void test("the command reads a coverage report and exits 2 on an offense", () => {
  const report = join(mkdtempSync(join(tmpdir(), "crap-report-")), "coverage-final.json");

  writeFileSync(report, "{}");
  const run = spawnSync(process.execPath, [join(import.meta.dirname, "crap.ts"), report], { encoding: "utf8" });

  assert.equal(run.status, 2, run.stderr);
  assert.match(run.stdout, /^src\/[^ ]+:\d+: \S+ has CRAP \d+\.\d \(complexity \d+, 0% of its statements covered\), over 6: /m);
  assert.doesNotMatch(run.stdout, new RegExp(ROOT));
});
