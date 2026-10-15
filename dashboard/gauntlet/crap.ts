import { globSync, readFileSync } from "node:fs";
import { join, relative } from "node:path";

import { functionsIn, type Found } from "./functions.ts";

const CEILING = 6;

const FIX = "cover every branch through the dashboard's tests, or split it into simpler functions that are each covered → GAUNTLET.md#crap";

export const TEST_CODE = ["src/**/*.test.ts", "src/**/*.test.tsx", "src/fake-daemon/**", "src/test-setup.ts"];

interface Start { line: number; column: number }

interface FileCoverage { statementMap: Record<string, { start: Start }>; s: Record<string, number> }

export type Coverage = Record<string, FileCoverage | undefined>;

interface Statement { start: Start; hits: number }

const before = (a: Start, b: Start) => a.line < b.line || (a.line === b.line && a.column <= b.column);

const contains = (found: Found, start: Start) => before(found.start, start) && before(start, found.end);

const innermost = (functions: Found[], start: Start) =>
  functions.filter((found) => contains(found, start)).reduce<Found | null>((inner, found) => (inner && before(found.start, inner.start) ? inner : found), null);

export const scoreOf = (complexity: number, covered: number) => complexity * complexity * (1 - covered) ** 3 + complexity;

const statementsOf = (file: FileCoverage | undefined): Statement[] =>
  Object.entries(file?.statementMap ?? {}).map(([id, statement]) => ({ start: statement.start, hits: file?.s[id] ?? 0 }));

const coveredShare = (owned: Statement[]) => (owned.length ? owned.filter((statement) => statement.hits > 0).length / owned.length : 0);

export interface Scored { found: Found; covered: number; crap: number }

export const scoreFile = (functions: Found[], file: FileCoverage | undefined): Scored[] => {
  const statements = statementsOf(file);

  return functions.map((found) => {
    const covered = coveredShare(statements.filter((statement) => innermost(functions, statement.start) === found));

    return { found, covered, crap: scoreOf(found.complexity, covered) };
  });
};

const offense = (path: string, scored: Scored) =>
  `${path}:${String(scored.found.start.line)}: ${scored.found.name} has CRAP ${scored.crap.toFixed(1)} (complexity ${String(scored.found.complexity)}, ${(scored.covered * 100).toFixed(0)}% of its statements covered), over ${String(CEILING)}: ${FIX}`;

export const productFiles = (root: string) => globSync("src/**/*.{ts,tsx}", { cwd: root, exclude: TEST_CODE }).sort();

export interface Verdict { lines: string[]; passed: boolean }

export const checkCrap = (root: string, coverage: Coverage): Verdict => {
  const offenses: string[] = [];
  let scored = 0;

  for (const path of productFiles(root)) {
    const absolute = join(root, path);
    const results = scoreFile(functionsIn(readFileSync(absolute, "utf8"), path), coverage[absolute]);

    offenses.push(...results.filter((result) => result.crap > CEILING).map((result) => offense(path, result)));
    scored += results.length;
  }

  return offenses.length
    ? { lines: [...offenses, `${String(offenses.length)} offense(s)`], passed: false }
    : { lines: [`${String(scored)} functions at CRAP ${String(CEILING)} or under`], passed: true };
};

const run = () => {
  const root = join(import.meta.dirname, "..");
  const [report = join(root, "coverage", "coverage-final.json")] = process.argv.slice(2);
  const verdict = checkCrap(root, JSON.parse(readFileSync(report, "utf8")) as Coverage);

  process.stdout.write(`${verdict.lines.map((line) => line.replaceAll(`${root}/`, "")).join("\n")}\n`);
  process.exitCode = verdict.passed ? 0 : 2;
};

if (process.argv[1] && relative(process.argv[1], import.meta.filename) === "") {
  run();
}
