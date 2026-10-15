import type { ESLint } from "eslint";

import { noComments } from "./no-comments.ts";
import { noIgnoredArtifactPaths } from "./no-ignored-artifact-paths.ts";
import { noRedundantPropsAlias } from "./no-redundant-props-alias.ts";

export const local: ESLint.Plugin = {
  meta: { name: "local" },
  rules: {
    "no-comments": noComments,
    "no-ignored-artifact-paths": noIgnoredArtifactPaths,
    "no-redundant-props-alias": noRedundantPropsAlias,
  },
};
