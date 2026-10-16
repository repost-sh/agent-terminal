import { test } from "node:test";

import { RuleTester } from "eslint";

import { noComments } from "./no-comments.ts";

const MESSAGE = "comment";

void test("no-comments reports every comment and nothing else", () => {
  new RuleTester({ languageOptions: { parserOptions: { ecmaFeatures: { jsx: true } } } }).run("no-comments", noComments, {
    valid: ["const a = 1;", 'const url = "http://example.com/*x*/";', "const glob = `src/**/*.ts`;", "const regex = /\\/\\//;"],
    invalid: [
      { code: "// a line comment\nconst a = 1;", errors: [{ messageId: MESSAGE, line: 1 }] },
      { code: "const a = 1; /* a block comment */", errors: [{ messageId: MESSAGE, line: 1 }] },
      { code: "/** a doc comment */\nfunction f() {}", errors: [{ messageId: MESSAGE, line: 1 }] },
      { code: "const a = 1;\n// one\n// two\n", errors: [{ messageId: MESSAGE, line: 2 }, { messageId: MESSAGE, line: 3 }] },
      { code: "const view = <div>{/* in JSX */}</div>;", errors: [{ messageId: MESSAGE, line: 1 }] },
    ],
  });
});

void test("a disable directive is reported as having no effect and as a comment", () => {
  new RuleTester({ linterOptions: { noInlineConfig: true, reportUnusedDisableDirectives: "off" } }).run("no-comments", noComments, {
    valid: [],
    invalid: [
      {
        code: "/* eslint-disable */\nconst a = 1;",
        errors: [
          { message: "'/* eslint-disable */' has no effect because you have 'noInlineConfig' setting in your config.", line: 1 },
          { messageId: MESSAGE, line: 1 },
        ],
      },
    ],
  });
});
