import { test } from "node:test";

import { RuleTester } from "eslint";

import { noRedundantPropsAlias } from "./no-redundant-props-alias.ts";

void test("no-redundant-props-alias reports a same-name alias of a props member", () => {
  new RuleTester().run("no-redundant-props-alias", noRedundantPropsAlias, {
    valid: [
      "const isOpen = props.open;",
      "const { open } = props;",
      "const open = other.open;",
      "const open = props[key];",
      "const open = props.open.value;",
      "let open;",
    ],
    invalid: [
      { code: "const open = props.open;", errors: [{ messageId: "redundantPropsAlias", data: { name: "open" } }] },
      { code: "let session = props.session;", errors: [{ messageId: "redundantPropsAlias", data: { name: "session" } }] },
    ],
  });
});
