import { test } from "node:test";

import { RuleTester } from "eslint";

import { noIgnoredArtifactPaths } from "./no-ignored-artifact-paths.ts";

const options = [{ paths: [".scratch", "docs/scratch"] }];

void test("no-ignored-artifact-paths reports strings and templates that point into an ignored folder", () => {
  new RuleTester().run("no-ignored-artifact-paths", noIgnoredArtifactPaths, {
    valid: [
      { code: 'const path = "src/services/payment/fixture.json";', options },
      { code: "const count = 42;", options },
      { code: "const nothing = null;", options },
      { code: "const path = `src/${name}`;", options },
    ],
    invalid: [
      {
        code: 'const evidence = ".scratch/qa/2026-09-08/finality.json";',
        options,
        errors: [{ messageId: "ignoredArtifactPath", data: { path: ".scratch" } }],
      },
      {
        code: "const spec = `docs/scratch/specs/${name}`;",
        options,
        errors: [{ messageId: "ignoredArtifactPath", data: { path: "docs/scratch" } }],
      },
    ],
  });
});

void test("no-ignored-artifact-paths only knows the paths it is given", () => {
  new RuleTester().run("no-ignored-artifact-paths", noIgnoredArtifactPaths, {
    valid: [{ code: 'const path = ".scratch/anything.json";', options: [{ paths: ["output/"] }] }],
    invalid: [
      {
        code: 'const path = "output/run.json";',
        options: [{ paths: ["output/"] }],
        errors: [{ messageId: "ignoredArtifactPath", data: { path: "output/" } }],
      },
    ],
  });
});
