import type { Rule } from "eslint";
import type { VariableDeclarator } from "estree";

type Member = Extract<VariableDeclarator["init"], { type: "MemberExpression" }>;

const isPropsObject = (member: Member) => member.object.type === "Identifier" && member.object.name === "props";

const propsMember = (init: VariableDeclarator["init"]) => {
  if (init?.type !== "MemberExpression" || init.computed || !isPropsObject(init)) {
    return undefined;
  }

  return init.property.type === "Identifier" ? init.property.name : undefined;
};

const redundantAlias = (declarator: VariableDeclarator) => {
  const { id, init } = declarator;

  return id.type === "Identifier" && propsMember(init) === id.name ? id.name : undefined;
};

export const noRedundantPropsAlias: Rule.RuleModule = {
  meta: {
    type: "suggestion",
    docs: { description: "Disallow redundant same-name aliases like `const open = props.open`." },
    messages: {
      redundantPropsAlias: "Do not alias `props.{{name}}` to `{{name}}`. Use `props.{{name}}` directly or destructure intentionally.",
    },
    schema: [],
  },
  create(context) {
    return {
      VariableDeclarator(node) {
        const name = redundantAlias(node);

        if (name !== undefined) {
          context.report({ node, messageId: "redundantPropsAlias", data: { name } });
        }
      },
    };
  },
};
