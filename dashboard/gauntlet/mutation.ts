import { globSync, readFileSync } from "node:fs";
import { join, relative } from "node:path";

import type { MutantResult, PartialStrykerOptions } from "@stryker-mutator/api/core";
import { Stryker } from "@stryker-mutator/core";

import { changedRanges } from "./changed.ts";
import { TEST_CODE } from "./test-code.ts";

const FIX = "write a test whose expected value changes when the mutated code does → GAUNTLET.md#mutation";

const SEALED = "delete it: the mutation gate's options live in dashboard/gauntlet/mutation.ts → GAUNTLET.md#mutation";

const TIMEOUTS_AT_MOST = 0.05;

const UNDETECTED = ["Survived", "NoCoverage", "Ignored"];

const UNTESTED = ["CompileError", "Pending"];

export const CONFIG_FILES = "{,.}stryker{.conf,.config}.{json,js,mjs,cjs}";

const PRODUCT_SOURCE = ["src/**/*.{ts,tsx}", ...TEST_CODE.map((pattern) => `!${pattern}`)];

export const OPTIONS: PartialStrykerOptions = {
  testRunner: "vitest",
  plugins: ["@stryker-mutator/vitest-runner", "@stryker-mutator/typescript-checker"],
  checkers: ["typescript"],
  tsconfigFile: "tsconfig.json",
  vitest: { configFile: "vitest.config.ts" },
  mutate: PRODUCT_SOURCE,
  coverageAnalysis: "perTest",
  symlinkNodeModules: false,
  ignoreStatic: false,
  reporters: ["progress"],
  thresholds: { high: 100, low: 100, break: null },
};

export interface Verdict { lines: string[]; passed: boolean }

const where = (mutant: MutantResult) => `${mutant.fileName}:${String(mutant.location.start.line)}:${String(mutant.location.start.column)}`;

const survivor = (mutant: MutantResult) => `${where(mutant)}: ${mutant.mutatorName} → ${JSON.stringify(mutant.replacement)} ${mutant.status === "NoCoverage" ? "ran under no test" : "was not caught"}: ${FIX}`;

const guards = (tested: MutantResult[], compiled: number) => {
  const timeouts = tested.filter((mutant) => mutant.status === "Timeout").length;

  return [
    ...(tested.length === 0 && compiled > 0 ? [`no mutant was tested: point the gate at the dashboard's source → GAUNTLET.md#mutation`] : []),
    ...(timeouts > TIMEOUTS_AT_MOST * tested.length ? [`${String(timeouts)} of ${String(tested.length)} mutants timed out, over 5%: make the slow tests fast → GAUNTLET.md#mutation`] : []),
  ];
};

export const judge = (results: MutantResult[]): Verdict => {
  const tested = results.filter((mutant) => !UNTESTED.includes(mutant.status));
  const compiled = results.filter((mutant) => mutant.status !== "CompileError").length;
  const offenses = [...results.filter((mutant) => UNDETECTED.includes(mutant.status)).map(survivor), ...guards(tested, compiled)];

  return offenses.length
    ? { lines: [...offenses, `${String(offenses.length)} offense(s)`], passed: false }
    : { lines: [`${String(tested.length)} mutants tested: every one caught (${String(results.length - tested.length)} did not compile)`], passed: true };
};

export const configFilesIn = (root: string) => globSync(CONFIG_FILES, { cwd: root }).sort();

const NOTHING_CHANGED: Verdict = { lines: ["no line of the dashboard's source changed, so no mutant needs testing"], passed: true };

const mutated = async (root: string, mutate: string[]) =>
  mutate.length
    ? judge((await new Stryker({ ...OPTIONS, mutate }).runMutationTest()).map((mutant) => ({ ...mutant, fileName: relative(root, mutant.fileName) })))
    : NOTHING_CHANGED;

const run = async () => {
  const root = join(import.meta.dirname, "..");
  const [diff] = process.argv.slice(2);
  const configs = configFilesIn(root);
  const verdict = configs.length
    ? { lines: [...configs.map((file) => `${file} changes Stryker's options: ${SEALED}`), `${String(configs.length)} offense(s)`], passed: false }
    : await mutated(root, diff === undefined ? PRODUCT_SOURCE : changedRanges(readFileSync(diff, "utf8")));

  process.stdout.write(`${verdict.lines.join("\n")}\n`);
  process.exitCode = verdict.passed ? 0 : 2;
};

if (process.argv[1] && relative(process.argv[1], import.meta.filename) === "") {
  await run();
}
