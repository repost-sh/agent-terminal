import assert from "node:assert/strict";
import { existsSync, globSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";

import js from "@eslint/js";
import { ESLint, type Linter } from "eslint";
import tseslint from "typescript-eslint";

const ROOT = join(import.meta.dirname, "..");

const FIX = "restore the rule in eslint.config.js and fix the code it flags → GAUNTLET.md#sealed-eslint-config";

const scratchFolders = readFileSync(join(ROOT, "..", ".gitignore"), "utf8")
  .split("\n")
  .map((entry) => entry.trim())
  .filter((entry) => entry.startsWith(".") && entry.endsWith("/"))
  .map((entry) => entry.slice(0, -1));

const RESTRICTED_SELECTORS = [
  "RestElement[typeAnnotation.typeAnnotation.type='TSTupleType']",
  ":matches(FunctionDeclaration, FunctionExpression, ArrowFunctionExpression, TSDeclareFunction, TSMethodSignature, TSFunctionType) > :matches(Identifier, ObjectPattern, ArrayPattern, RestElement, AssignmentPattern) > TSTypeAnnotation > TSTypeLiteral",
  "TSInterfaceBody[body.length=1] > TSPropertySignature[key.name='children']",
  "TSTypeAliasDeclaration > TSTypeLiteral[members.length=1] > TSPropertySignature[key.name='children']",
  "TSTypeAliasDeclaration[typeAnnotation.type='TSTypeReference'][typeAnnotation.typeName.name='PropsWithChildren']",
  "TSTypeAliasDeclaration[typeAnnotation.type='TSTypeReference'][typeAnnotation.typeName.type='TSQualifiedName'][typeAnnotation.typeName.right.name='PropsWithChildren']",
  "ObjectPattern[properties.length>2]",
  "TryStatement",
];

const REQUIRED: Record<string, unknown[]> = {
  curly: ["all"],
  "@stylistic/brace-style": ["1tbs", { allowSingleLine: false }],
  "unused-imports/no-unused-imports": [],
  "@typescript-eslint/no-explicit-any": [],
  "@typescript-eslint/no-deprecated": [],
  "better-max-params/better-max-params": [{ func: 2 }],
  "react-hooks/rules-of-hooks": [],
  "react-hooks/exhaustive-deps": [],
  complexity: [6],
  "max-depth": [4],
  "max-lines-per-function": [{ max: 40, skipBlankLines: true }],
  "max-lines": [500],
  "max-nested-callbacks": [3],
  "local/no-comments": [],
  "local/no-ignored-artifact-paths": [{ paths: scratchFolders }],
  "local/no-redundant-props-alias": [],
};

const REPLACED = ["no-unused-vars", "@typescript-eslint/no-unused-vars"];

const STRICT_FLAGS = [
  "strict",
  "noUnusedLocals",
  "noUnusedParameters",
  "noFallthroughCasesInSwitch",
  "noUncheckedIndexedAccess",
  "exactOptionalPropertyTypes",
  "noPropertyAccessFromIndexSignature",
  "noImplicitOverride",
  "noImplicitReturns",
  "forceConsistentCasingInFileNames",
];

const LINTED = () => [
  ...globSync("src/**/*.{ts,tsx}", { cwd: ROOT }),
  ...globSync("eslint/*.ts", { cwd: ROOT }).filter((file) => !file.endsWith(".d.ts")),
  "vite.config.ts",
  "eslint.config.js",
];

const SEVERITIES: Record<string, number> = { off: 0, warn: 1, error: 2 };

const severityOf = (entry: Linter.RuleEntry | undefined) => {
  const level = typeof entry === "object" ? entry[0] : entry;

  return typeof level === "string" ? SEVERITIES[level] ?? -1 : level ?? 0;
};

const optionsOf = (entry: Linter.RuleEntry | undefined): unknown[] => {
  if (typeof entry !== "object") {
    return [];
  }

  const [, ...options] = entry;

  return options;
};

const isRecord = (value: unknown): value is Record<string, unknown> => value !== null && typeof value === "object";

const containsAll = (actual: unknown, expected: unknown): boolean => {
  if (Array.isArray(expected)) {
    return Array.isArray(actual) && expected.every((item, index) => containsAll(actual[index], item));
  }

  return isRecord(expected) ? isRecord(actual) && Object.entries(expected).every(([key, value]) => containsAll(actual[key], value)) : actual === expected;
};

const errorWith = (entry: Linter.RuleEntry | undefined, options: unknown[]) => severityOf(entry) === 2 && containsAll(optionsOf(entry), options);

const presetsOnly = () =>
  new ESLint({
    cwd: ROOT,
    overrideConfigFile: true,
    overrideConfig: [js.configs.recommended, ...tseslint.configs.strictTypeChecked, ...tseslint.configs.stylisticTypeChecked] as Linter.Config[],
  });

interface Checked { file: string; rules: Partial<Linter.RulesRecord> }

const requiredOffenses = (checked: Checked) =>
  Object.entries(REQUIRED).flatMap(([rule, options]) => {
    const entry = checked.rules[rule];

    return errorWith(entry, options) ? [] : [`${checked.file}: ${rule} is ${JSON.stringify(entry)}, not error ${JSON.stringify(options)}`];
  });

const presetOffenses = (checked: Checked, preset: Partial<Linter.RulesRecord>) =>
  Object.entries(preset)
    .filter(([rule, entry]) => severityOf(entry) > 0 && !REPLACED.includes(rule) && !(rule in REQUIRED))
    .flatMap(([rule, entry]) => {
      const kept = errorWith(checked.rules[rule], optionsOf(entry));

      return kept ? [] : [`${checked.file}: ${rule} from the presets is ${JSON.stringify(checked.rules[rule])}`];
    });

const syntaxOffenses = (checked: Checked) => {
  const listed = optionsOf(checked.rules["no-restricted-syntax"]).map((option) => (option as { selector: string }).selector);
  const missing = RESTRICTED_SELECTORS.filter((selector) => !listed.includes(selector));

  return missing.map((selector) => `${checked.file}: no-restricted-syntax lacks ${selector}`);
};

const replacedOffenses = (checked: Checked) =>
  REPLACED.filter((rule) => severityOf(checked.rules[rule]) !== 0 || severityOf(checked.rules["unused-imports/no-unused-vars"]) !== 2).map(
    (rule) => `${checked.file}: ${rule} is replaced by unused-imports/no-unused-vars, which must be the one on`,
  );

const warnings = (checked: Checked) =>
  Object.entries(checked.rules)
    .filter(([, entry]) => severityOf(entry) === 1)
    .map(([rule]) => `${checked.file}: ${rule} only warns`);

void test("every linted file is linted with the sealed rules at error", async () => {
  const eslint = new ESLint({ cwd: ROOT });
  const presets = presetsOnly();
  const offenses: string[] = [];

  for (const file of LINTED()) {
    assert.equal(await eslint.isPathIgnored(file), false, `${file} is ignored: ${FIX}`);
    const config = (await eslint.calculateConfigForFile(file)) as Linter.Config;
    const preset = ((await presets.calculateConfigForFile(file)) as Linter.Config).rules ?? {};
    const checked = { file, rules: config.rules ?? {} };

    offenses.push(...requiredOffenses(checked), ...presetOffenses(checked, preset), ...syntaxOffenses(checked), ...replacedOffenses(checked), ...warnings(checked));
  }

  assert.deepEqual(offenses, [], `${offenses.join("\n")}\n${FIX}`);
});

void test("inline config is off and unused directives or inline configs are errors for every linted file", async () => {
  const eslint = new ESLint({ cwd: ROOT });

  for (const file of LINTED()) {
    const config = (await eslint.calculateConfigForFile(file)) as Linter.Config;

    assert.deepEqual(
      [config.linterOptions?.noInlineConfig, severityOf(config.linterOptions?.reportUnusedDisableDirectives as Linter.RuleEntry)],
      [true, 2],
      `${file}: ${FIX}`,
    );
    assert.equal(severityOf(config.linterOptions?.reportUnusedInlineConfigs as Linter.RuleEntry), 2, `${file}: ${FIX}`);
  }
});

const compilerOptions = (path: string) => (JSON.parse(readFileSync(join(ROOT, path), "utf8")) as { compilerOptions: Record<string, unknown> }).compilerOptions;

void test("both TypeScript projects keep every strict flag and silence no deprecation", () => {
  for (const path of ["tsconfig.json", "eslint/tsconfig.json"]) {
    const options = compilerOptions(path);
    const lowered = STRICT_FLAGS.filter((flag) => options[flag] !== true);

    assert.deepEqual(lowered, [], `${path} lowers ${lowered.join(", ")}: set them back to true and fix the code → GAUNTLET.md#sealed-eslint-config`);
    assert.equal("ignoreDeprecations" in options, false, `${path} silences deprecations: change the deprecated option instead → GAUNTLET.md#sealed-eslint-config`);
  }
});

void test("the scripts lint with no warnings allowed and type-check without emitting", () => {
  const scripts = (JSON.parse(readFileSync(join(ROOT, "package.json"), "utf8")) as { scripts: Record<string, string> }).scripts;

  assert.deepEqual(
    { lint: scripts["lint"], check: scripts["check"], build: scripts["build"] },
    { lint: "eslint --max-warnings 0 .", check: "tsc --noEmit", build: "tsc --noEmit && vite build" },
    `package.json scripts changed: restore them → GAUNTLET.md#sealed-eslint-config`,
  );
});

void test("eslint.config.js is the only ESLint config in the dashboard", () => {
  const others = [".eslintrc", ".eslintrc.js", ".eslintrc.cjs", ".eslintrc.json", ".eslintrc.yml", "eslint.config.mjs", "eslint.config.cjs", "eslint.config.ts", "eslint.config.mts"]
    .flatMap((name) => [name, ...globSync(`src/**/${name}`, { cwd: ROOT })])
    .filter((path) => existsSync(join(ROOT, path)));

  assert.deepEqual(others, [], `another ESLint config can override the sealed one: delete it → GAUNTLET.md#sealed-eslint-config`);
});
