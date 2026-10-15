import type { Rule } from "eslint";

interface Options { paths: string[] }

const ignoredPathIn = (value: unknown, paths: readonly string[]) =>
  typeof value === "string" ? paths.find((path) => value.includes(path)) : undefined;

export const noIgnoredArtifactPaths: Rule.RuleModule = {
  meta: {
    type: "problem",
    docs: {
      description: "Disallow referencing gitignored artefact directories from code or tests. Fixtures must be tracked next to the test.",
    },
    messages: {
      ignoredArtifactPath:
        'Do not reference "{{path}}": it is a gitignored artefact directory. Commit the fixture next to the code that needs it.',
    },
    schema: [
      {
        type: "object",
        properties: { paths: { type: "array", items: { type: "string" }, minItems: 1 } },
        required: ["paths"],
        additionalProperties: false,
      },
    ],
  },
  create(context) {
    const [options] = context.options as [Options];
    const report = (node: Rule.Node, value: unknown) => {
      const path = ignoredPathIn(value, options.paths);

      if (path !== undefined) {
        context.report({ node, messageId: "ignoredArtifactPath", data: { path } });
      }
    };

    return {
      Literal(node) {
        report(node, node.value);
      },
      TemplateElement(node) {
        report(node, node.value.cooked ?? node.value.raw);
      },
    };
  },
};
