import type { Rule } from "eslint";

export const noComments: Rule.RuleModule = {
  meta: {
    type: "problem",
    docs: { description: "Disallow every comment: names, types and tests say what code does." },
    messages: { comment: "Comments are banned in this codebase: say it with a name, a type or a test." },
    schema: [],
  },
  create(context) {
    return {
      Program(program) {
        for (const comment of context.sourceCode.getAllComments()) {
          context.report({ loc: comment.loc ?? program.loc, messageId: "comment" });
        }
      },
    };
  },
};
