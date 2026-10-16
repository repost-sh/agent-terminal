import { spawnSync } from "node:child_process";
import { existsSync, globSync, readFileSync, realpathSync, rmSync } from "node:fs";
import { dirname, join, relative } from "node:path";

import { TEST_CODE } from "./test-code.ts";

const MIN_TOKENS = 100;

const FIX = "move the shared code into the module that owns it and call it from both places → GAUNTLET.md#duplication";

const NOTHING = "run the gate from the repository root, where the crates' src folders and dashboard/ are → GAUNTLET.md#duplication";

const SEALED = "delete it: the duplication gate's options live in dashboard/gauntlet/duplication.ts → GAUNTLET.md#duplication";

const JSCPD = join(import.meta.dirname, "..", "node_modules", ".bin", "jscpd");

const IGNORED = TEST_CODE.map((pattern) => `**/dashboard/${pattern}`);

interface Place { name: string; start: number; end: number }

interface Clone { tokens: number; firstFile: Place; secondFile: Place }

interface Report { duplicates: Clone[]; statistics: { total: { sources: number } } }

interface Merged { min_tokens: number; min_lines: number; ignore: string[]; ignore_patterns: string[] }

export interface Verdict { lines: string[]; passed: boolean }

export const scope = (root: string) => [
  ...globSync("cli/**/Cargo.toml", { cwd: root, exclude: ["**/target/**"] })
    .map((manifest) => join(dirname(manifest), "src"))
    .filter((src) => existsSync(join(root, src))),
  ...(existsSync(join(root, "dashboard")) ? ["dashboard"] : []),
].sort();

const options = (root: string) => [
  "--min-tokens",
  String(MIN_TOKENS),
  "--min-lines",
  "1",
  "--format",
  "rust,typescript,tsx",
  "--ignore",
  IGNORED.join(","),
  "--absolute",
  "--no-tips",
  "--no-colors",
  ...scope(root),
];

const jscpd = (root: string, extra: string[]) => {
  const ran = spawnSync(JSCPD, [...options(root), ...extra], { cwd: root, encoding: "utf8" });

  if (ran.status !== 0) {
    throw new Error(`jscpd failed: ${ran.stderr || String(ran.error)}`);
  }

  return `${ran.stderr}${ran.stdout}`;
};

const ours = (merged: Merged) =>
  merged.min_tokens === MIN_TOKENS && merged.min_lines === 1 && merged.ignore.join(",") === IGNORED.join(",") && merged.ignore_patterns.length === 0;

const unsealed = (root: string) => {
  const debug = jscpd(root, ["--debug"]);
  const source = /^Using config from (.+)$/m.exec(debug)?.[1];

  return source ?? (ours(JSON.parse(debug.slice(debug.indexOf("{"))) as Merged) ? undefined : "the environment");
};

const report = (root: string): Report => {
  const output = join(root, "dashboard", "node_modules", ".cache", "jscpd");
  const written = join(output, "jscpd-report.json");

  rmSync(written, { force: true });
  jscpd(root, ["--reporters", "json", "--output", output, "--silent"]);

  return JSON.parse(readFileSync(written, "utf8")) as Report;
};

const place = (root: string, at: Place) => `${relative(root, at.name)}:${String(at.start)}-${String(at.end)}`;

const offense = (root: string, clone: Clone) =>
  `${place(root, clone.firstFile)} repeats ${place(root, clone.secondFile)} (${String(clone.tokens)} tokens): ${FIX}`;

export const checkDuplication = (root: string): Verdict => {
  const config = unsealed(root);

  if (config) {
    return { lines: [`${config} changes jscpd's options: ${SEALED}`, "1 offense(s)"], passed: false };
  }

  const { duplicates, statistics } = report(root);
  const offenses = statistics.total.sources === 0
    ? [`the duplication gate scanned no files: ${NOTHING}`]
    : duplicates.map((clone) => offense(realpathSync(root), clone));

  return offenses.length
    ? { lines: [...offenses, `${String(offenses.length)} offense(s)`], passed: false }
    : { lines: [`${String(statistics.total.sources)} files: no block of ${String(MIN_TOKENS)} tokens appears twice`], passed: true };
};

if (process.argv[1] && relative(process.argv[1], import.meta.filename) === "") {
  const verdict = checkDuplication(join(import.meta.dirname, "..", ".."));

  process.stdout.write(`${verdict.lines.join("\n")}\n`);
  process.exitCode = verdict.passed ? 0 : 2;
}
