import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";

import { checkProperties, propertiesIn } from "./props.ts";

const PROPERTY = `fc.assert(fc.property(fc.nat(), (n) => {
  expect(n * 2 % 2).toBe(0);
}));`;

void test("a property with an assertion counts, and decoys that only name fast-check do not", () => {
  assert.equal(propertiesIn(PROPERTY, "a.test.ts"), 1);
  assert.equal(propertiesIn("fc.assert(fc.asyncProperty(fc.nat(), async (n) => { assert.ok(n >= 0); }));", "b.test.ts"), 1);
  assert.equal(propertiesIn("fc.assert(fc.property(fc.nat(), (n) => n >= 0));", "c.test.ts"), 0);
  assert.equal(propertiesIn("fc.property(fc.nat(), (n) => { expect(n).toBe(n); });", "d.test.ts"), 0);
  assert.equal(propertiesIn('const text = "fc.assert(fc.property(fc.nat(), (n) => { expect(n) }))";', "e.test.ts"), 0);
  assert.equal(propertiesIn("other.assert(fc.property(fc.nat(), (n) => { expect(n).toBe(n); }));", "f.test.ts"), 0);
});

void test("a file that does not parse is an error naming it", () => {
  assert.throws(() => propertiesIn("fc.assert(", "broken.test.ts"), /^Error: broken\.test\.ts:1: /);
});

const scratchRoot = (files: Record<string, string>) => {
  const root = mkdtempSync(join(tmpdir(), "props-"));

  for (const [path, code] of Object.entries(files)) {
    mkdirSync(dirname(join(root, path)), { recursive: true });
    writeFileSync(join(root, path), code);
  }

  return root;
};

void test("the dashboard needs at least one property test among its tests", () => {
  assert.deepEqual(checkProperties(scratchRoot({ "src/lib/a.test.ts": PROPERTY, "src/lib/a.ts": PROPERTY })), {
    lines: ["dashboard: 1 property tests"],
    passed: true,
  });
  assert.deepEqual(checkProperties(scratchRoot({ "src/lib/a.ts": PROPERTY })), {
    lines: [
      "dashboard: no property test: state an invariant of the dashboard's behaviour as fc.assert(fc.property(…)) with an assertion in it → GAUNTLET.md#property-tests",
      "1 offense(s)",
    ],
    passed: false,
  });
});
