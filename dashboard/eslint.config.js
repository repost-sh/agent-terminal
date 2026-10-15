import { readFileSync } from "node:fs";
import { join } from "node:path";

import js from "@eslint/js";
import stylistic from "@stylistic/eslint-plugin";
import betterMaxParams from "eslint-plugin-better-max-params";
import reactHooks from "eslint-plugin-react-hooks";
import unusedImports from "eslint-plugin-unused-imports";
import { defineConfig } from "eslint/config";
import tseslint from "typescript-eslint";

import { local } from "./eslint/plugin.ts";

const EFFECT_HOOKS =
  "Effect hooks are banned in the dashboard: subscribe with useSyncExternalStore, manage DOM and xterm.js in a ref callback that returns its cleanup, act in event handlers, derive values during render, reset state with key.";

const scratchFolders = readFileSync(join(import.meta.dirname, "..", ".gitignore"), "utf8")
  .split("\n")
  .map((entry) => entry.trim())
  .filter((entry) => entry.startsWith(".") && entry.endsWith("/"))
  .map((entry) => entry.slice(0, -1));

const restrictedSyntax = [
  {
    selector: "RestElement[typeAnnotation.typeAnnotation.type='TSTupleType']",
    message: "Do not use tuple-typed rest parameters. Use a named object parameter instead.",
  },
  {
    selector:
      ":matches(FunctionDeclaration, FunctionExpression, ArrowFunctionExpression, TSDeclareFunction, TSMethodSignature, TSFunctionType) > :matches(Identifier, ObjectPattern, ArrayPattern, RestElement, AssignmentPattern) > TSTypeAnnotation > TSTypeLiteral",
    message: "Inline object types in parameters are not allowed. Extract to a named interface.",
  },
  {
    selector: "TSInterfaceBody[body.length=1] > TSPropertySignature[key.name='children']",
    message: "Interface with only `children` -- use PropsWithChildren from react instead.",
  },
  {
    selector: "TSTypeAliasDeclaration > TSTypeLiteral[members.length=1] > TSPropertySignature[key.name='children']",
    message: "Type with only `children` -- use PropsWithChildren from react instead.",
  },
  {
    selector: "TSTypeAliasDeclaration[typeAnnotation.type='TSTypeReference'][typeAnnotation.typeName.name='PropsWithChildren']",
    message: "Do not alias directly to PropsWithChildren. Use PropsWithChildren at the component parameter instead.",
  },
  {
    selector:
      "TSTypeAliasDeclaration[typeAnnotation.type='TSTypeReference'][typeAnnotation.typeName.type='TSQualifiedName'][typeAnnotation.typeName.right.name='PropsWithChildren']",
    message: "Do not alias directly to PropsWithChildren. Use PropsWithChildren at the component parameter instead.",
  },
  {
    selector: "ObjectPattern[properties.length>2]",
    message: "Destructuring more than two properties is not allowed.",
  },
  {
    selector: "MemberExpression[object.name=/^(test|it|describe|suite)$/][property.name=/^(only|skip|skipIf|runIf|todo|fails)$/]",
    message: "Focused, skipped and pending tests are not allowed: run every test, and fix or delete the one that fails.",
  },
  {
    selector: "TryStatement",
    message:
      "try/catch/finally is not allowed. Use `settle` from src/lib/settle.ts, which turns a throwing call or a rejecting promise into a Result.",
  },
];

const LINTED = ["src/**/*.{ts,tsx}", "vite.config.ts", "vitest.config.ts", "eslint.config.js", "eslint/**/*.ts"];

export default defineConfig(
  {
    files: LINTED,
    extends: [js.configs.recommended, tseslint.configs.strictTypeChecked, tseslint.configs.stylisticTypeChecked],
    linterOptions: {
      noInlineConfig: true,
      reportUnusedDisableDirectives: "error",
      reportUnusedInlineConfigs: "error",
    },
    languageOptions: {
      parserOptions: {
        projectService: { allowDefaultProject: ["vite.config.ts", "vitest.config.ts", "eslint.config.js"], defaultProject: "eslint/tsconfig.json" },
        tsconfigRootDir: import.meta.dirname,
      },
    },
    plugins: {
      "@stylistic": stylistic,
      "better-max-params": betterMaxParams,
      local,
      "react-hooks": { rules: reactHooks.rules },
      "unused-imports": unusedImports,
    },
  },
  {
    files: LINTED,
    rules: {
      curly: ["error", "all"],
      "@stylistic/brace-style": ["error", "1tbs", { allowSingleLine: false }],
      "@stylistic/padding-line-between-statements": [
        "error",
        { blankLine: "always", prev: ["const", "let", "var"], next: "*" },
        { blankLine: "any", prev: ["const", "let", "var"], next: ["const", "let", "var"] },
        { blankLine: "always", prev: "block-like", next: "*" },
        { blankLine: "always", prev: "*", next: "return" },
        { blankLine: "always", prev: "*", next: "throw" },
      ],
      "no-unused-vars": "off",
      "@typescript-eslint/no-unused-vars": "off",
      "unused-imports/no-unused-imports": "error",
      "unused-imports/no-unused-vars": [
        "error",
        {
          args: "all",
          argsIgnorePattern: "^_",
          caughtErrors: "all",
          caughtErrorsIgnorePattern: "^_",
          destructuredArrayIgnorePattern: "^_",
          varsIgnorePattern: "^_",
          ignoreRestSiblings: true,
        },
      ],
      "@typescript-eslint/no-explicit-any": "error",
      "@typescript-eslint/no-deprecated": "error",
      "better-max-params/better-max-params": ["error", { func: 2 }],
    },
  },
  {
    files: LINTED,
    rules: {
      "no-restricted-syntax": ["error", ...restrictedSyntax],
      "react-hooks/rules-of-hooks": "error",
      "react-hooks/exhaustive-deps": "error",
      "no-restricted-imports": [
        "error",
        { paths: [{ name: "react", importNames: ["useEffect", "useLayoutEffect", "useInsertionEffect"], message: EFFECT_HOOKS }] },
      ],
      "no-restricted-properties": [
        "error",
        { property: "useEffect", message: EFFECT_HOOKS },
        { property: "useLayoutEffect", message: EFFECT_HOOKS },
        { property: "useInsertionEffect", message: EFFECT_HOOKS },
      ],
    },
  },
  {
    files: LINTED,
    rules: {
      complexity: ["error", 6],
      "max-depth": ["error", 4],
      "max-lines-per-function": ["error", { max: 40, skipBlankLines: true }],
      "max-lines": ["error", 500],
      "max-nested-callbacks": ["error", 3],
      "local/no-comments": "error",
      "local/no-ignored-artifact-paths": ["error", { paths: scratchFolders }],
      "local/no-redundant-props-alias": "error",
    },
  },
);
