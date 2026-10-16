import { globSync, readFileSync } from "node:fs";
import { join, relative } from "node:path";

import { Linter, type Rule } from "eslint";
import type { Node } from "estree";
import tseslint from "typescript-eslint";

const FIX = "state an invariant of the dashboard's behaviour as fc.assert(fc.property(…)) with an assertion in it → GAUNTLET.md#property-tests";

const PROPERTY =
  "CallExpression[callee.object.name='fc'][callee.property.name='assert'] > CallExpression.arguments:first-child[callee.object.name='fc'][callee.property.name=/^(property|asyncProperty)$/]";

interface Count { value: number }

const counter = (count: Count): Rule.RuleModule => ({
  meta: { type: "problem", schema: [] },
  create(context) {
    return {
      [PROPERTY](node: Node) {
        const predicate = node.type === "CallExpression" ? node.arguments.at(-1) : undefined;
        const text = predicate ? context.sourceCode.getText(predicate) : "";

        count.value += /\bexpect\(|\bassert\./.test(text) ? 1 : 0;
      },
    };
  },
});

export const propertiesIn = (code: string, filename: string) => {
  const count = { value: 0 };
  const messages = new Linter({ configType: "flat" }).verify(
    code,
    [
      {
        files: ["**/*.ts", "**/*.tsx"],
        languageOptions: { parser: tseslint.parser, parserOptions: { ecmaFeatures: { jsx: true } } },
        plugins: { gauntlet: { rules: { properties: counter(count) } } },
        rules: { "gauntlet/properties": "error" },
      },
    ],
    filename,
  );
  const fatal = messages.find((message) => message.fatal);

  if (fatal) {
    throw new Error(`${filename}:${String(fatal.line)}: ${fatal.message}`);
  }

  return count.value;
};

export interface Verdict { lines: string[]; passed: boolean }

export const checkProperties = (root: string): Verdict => {
  const tests = globSync("src/**/*.test.{ts,tsx}", { cwd: root }).sort();
  const found = tests.reduce((total, path) => total + propertiesIn(readFileSync(join(root, path), "utf8"), path), 0);

  return found > 0
    ? { lines: [`dashboard: ${String(found)} property tests`], passed: true }
    : { lines: [`dashboard: no property test: ${FIX}`, "1 offense(s)"], passed: false };
};

if (process.argv[1] && relative(process.argv[1], import.meta.filename) === "") {
  const verdict = checkProperties(join(import.meta.dirname, ".."));

  process.stdout.write(`${verdict.lines.join("\n")}\n`);
  process.exitCode = verdict.passed ? 0 : 2;
}
