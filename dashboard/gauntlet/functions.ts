import { Linter, type Rule } from "eslint";
import type { Node, Position } from "estree";
import tseslint from "typescript-eslint";

export interface Found { name: string; start: Position; end: Position; complexity: number }

const BRANCHES = [
  "CatchClause",
  "ConditionalExpression",
  "LogicalExpression",
  "ForStatement",
  "ForInStatement",
  "ForOfStatement",
  "IfStatement",
  "WhileStatement",
  "DoWhileStatement",
  "AssignmentPattern",
  "SwitchStatement",
];

const LOGICAL_ASSIGNMENTS = new Set(["&&=", "||=", "??="]);

const SCORED = new Set(["function", "class-field-initializer", "class-static-block"]);

const declaredName = (node: Node) => ((node.type === "FunctionDeclaration" || node.type === "FunctionExpression") && node.id ? node.id.name : null);

const assignedName = (parent: Node) => {
  if (parent.type === "VariableDeclarator") {
    return parent.id.type === "Identifier" ? parent.id.name : null;
  }

  return (parent.type === "Property" || parent.type === "MethodDefinition") && parent.key.type === "Identifier" ? parent.key.name : null;
};

const nameOf = (node: Node, parent: Node | null) => declaredName(node) ?? (parent && assignedName(parent)) ?? "an anonymous function";

interface Count { value: number }

const finder = (found: Found[]): Rule.RuleModule => ({
  meta: { type: "problem", schema: [] },
  create() {
    const counts: Count[] = [];
    const add = () => {
      const current = counts.at(-1);

      if (current) {
        current.value += 1;
      }
    };

    return {
      onCodePathStart() {
        counts.push({ value: 1 });
      },
      ...Object.fromEntries(BRANCHES.map((selector) => [selector, add])),
      AssignmentExpression(node) {
        if (LOGICAL_ASSIGNMENTS.has(node.operator)) {
          add();
        }
      },
      "MemberExpression[optional=true], CallExpression[optional=true]": add,
      onCodePathEnd(codePath, node) {
        const count = counts.pop();

        if (count && node.loc && SCORED.has(codePath.origin)) {
          found.push({ name: nameOf(node, node.parent), start: node.loc.start, end: node.loc.end, complexity: count.value });
        }
      },
    };
  },
});

export const functionsIn = (code: string, filename: string): Found[] => {
  const found: Found[] = [];
  const linter = new Linter({ configType: "flat" });
  const messages = linter.verify(
    code,
    [
      {
        files: ["**/*.ts", "**/*.tsx"],
        languageOptions: { parser: tseslint.parser, parserOptions: { ecmaFeatures: { jsx: true } } },
        plugins: { gauntlet: { rules: { functions: finder(found) } } },
        rules: { "gauntlet/functions": "error" },
      },
    ],
    filename,
  );
  const fatal = messages.find((message) => message.fatal);

  if (fatal) {
    throw new Error(`${filename}:${String(fatal.line)}: ${fatal.message}`);
  }

  return found;
};
