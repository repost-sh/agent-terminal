import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";

import { checkDuplication, scope } from "./duplication.ts";

const FIX = "move the shared code into the module that owns it and call it from both places → GAUNTLET.md#duplication";

const SEALED = "delete it: the duplication gate's options live in dashboard/gauntlet/duplication.ts → GAUNTLET.md#duplication";

const RUST = Array.from({ length: 6 }, (_, at) => `pub fn step${String(at)}(a: u32) -> u32 {\n    let b = a * ${String(at)} + 7;\n    if b > 10 { b - 3 } else { b + 3 }\n}\n`).join("\n");

const TYPESCRIPT = Array.from({ length: 6 }, (_, at) => `export const step${String(at)} = (a: number) => {\n  const b = a * ${String(at)} + 7;\n  return b > 10 ? b - 3 : b + 3;\n};\n`).join("\n");

const PRODUCT = [
  "export const describe = (count: number, noun: string) => {",
  "  const plural = count === 1 ? noun : `${noun}s`;",
  "  const shown = count > 999 ? `${String(Math.round(count / 1000))}k` : String(count);",
  "  return [shown, plural].join(\" \");",
  "};",
  "export const clamp = (value: number, low: number, high: number) => Math.min(high, Math.max(low, value));",
  "export const initials = (name: string) => name.split(/\\s+/).filter(Boolean).map((part) => part[0]?.toUpperCase() ?? \"\").join(\"\");",
  "export const percent = (part: number, whole: number) => (whole === 0 ? \"0%\" : `${(100 * part / whole).toFixed(1)}%`);",
].join("\n");

const scratchRoot = (files: Record<string, string>) => {
  const root = mkdtempSync(join(tmpdir(), "duplication-"));

  for (const [path, code] of Object.entries(files)) {
    mkdirSync(dirname(join(root, path)), { recursive: true });
    writeFileSync(join(root, path), code);
  }

  return root;
};

const crates = {
  "cli/Cargo.toml": "[package]\nname = \"a\"\n",
  "cli/perf/Cargo.toml": "[package]\nname = \"b\"\n",
  "cli/target/debug/build/Cargo.toml": "[package]\nname = \"built\"\n",
  "cli/target/debug/build/src/copy.rs": RUST,
};

void test("the scope is every crate's src folder outside target/, and the dashboard", () => {
  assert.deepEqual(scope(scratchRoot({ ...crates, "cli/src/a.rs": "", "cli/perf/src/b.rs": "", "dashboard/src/a.ts": "" })), [
    "cli/perf/src",
    "cli/src",
    "dashboard",
  ]);
  assert.deepEqual(scope(scratchRoot({ "cli/Cargo.toml": "" })), []);
});

void test("a block repeated across two crates is an offense naming both places", () => {
  const root = scratchRoot({ ...crates, "cli/src/a.rs": RUST, "cli/perf/src/b.rs": RUST, "dashboard/src/a.ts": "export const a = 1;\n" });

  assert.deepEqual(checkDuplication(root), {
    lines: [`cli/perf/src/b.rs:1-29 repeats cli/src/a.rs:1-29 (222 tokens): ${FIX}`, "1 offense(s)"],
    passed: false,
  });
});

void test("a block repeated in the dashboard's code is an offense, and its tests may repeat themselves", () => {
  const tests = {
    "dashboard/src/a.test.ts": TYPESCRIPT,
    "dashboard/src/b.test.tsx": TYPESCRIPT,
    "dashboard/src/fake-daemon/c.ts": TYPESCRIPT,
    "dashboard/src/test-setup.ts": TYPESCRIPT,
    "dashboard/gauntlet/d.test.ts": TYPESCRIPT,
    "dashboard/eslint/e.test.ts": TYPESCRIPT,
  };

  assert.deepEqual(checkDuplication(scratchRoot({ ...tests, "dashboard/src/f.ts": PRODUCT })), {
    lines: ["1 files: no block of 100 tokens appears twice"],
    passed: true,
  });
  assert.equal(checkDuplication(scratchRoot({ ...tests, "dashboard/src/f.ts": TYPESCRIPT, "dashboard/src/g.ts": TYPESCRIPT })).passed, false);
});

void test("a jscpd configuration file is refused before anything is scanned", () => {
  const product = { "dashboard/src/f.ts": PRODUCT };

  assert.deepEqual(checkDuplication(scratchRoot({ ...product, ".jscpd.json": "{\"ignore\": [\"**/f.ts\"]}" })), {
    lines: [`.jscpd.json changes jscpd's options: ${SEALED}`, "1 offense(s)"],
    passed: false,
  });
  assert.deepEqual(checkDuplication(scratchRoot({ ...product, "package.json": "{\"jscpd\": {\"minTokens\": 999}}" })), {
    lines: [`package.json changes jscpd's options: ${SEALED}`, "1 offense(s)"],
    passed: false,
  });
});

void test("a scope with nothing in it fails rather than passing on no files", () => {
  assert.deepEqual(checkDuplication(scratchRoot({ "dashboard/README.md": "nothing to scan\n" })), {
    lines: ["the duplication gate scanned no files: run the gate from the repository root, where the crates' src folders and dashboard/ are → GAUNTLET.md#duplication", "1 offense(s)"],
    passed: false,
  });
});
