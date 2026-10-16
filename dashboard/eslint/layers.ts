import { dirname, join, relative, sep } from "node:path";

import type { Rule } from "eslint";
import type { Node } from "estree";

type Layer = "lib" | "components" | "app" | "tests";

const MAY_USE: Record<Layer, Layer[]> = {
  lib: ["lib"],
  components: ["lib", "components"],
  app: ["lib", "components", "app"],
  tests: ["lib", "components", "app", "tests"],
};

const TESTS = [/\.test(\.tsx?)?$/, /^src\/fake-daemon\//, /^src\/test-setup(\.ts)?$/];

const APP = ["src/app", "src/main"];

export const layerOf = (path: string): Layer | undefined => {
  if (TESTS.some((pattern) => pattern.test(path))) {
    return "tests";
  }

  if (path.startsWith("src/lib/")) {
    return "lib";
  }

  if (path.startsWith("src/components/")) {
    return "components";
  }

  return APP.includes(path.replace(/\.tsx?$/, "")) ? "app" : undefined;
};

const posix = (path: string) => path.split(sep).join("/");

const sourceOf = (node: Node) => {
  const source = "source" in node ? node.source : null;

  return source?.type === "Literal" && typeof source.value === "string" ? source.value : undefined;
};

interface Target { to: string; toLayer: Layer }

const targetOf = (file: string, node: Node): Target | undefined => {
  const specifier = sourceOf(node);
  const to = specifier?.startsWith(".") ? posix(join(dirname(file), specifier)) : undefined;
  const toLayer = to === undefined ? undefined : layerOf(to);

  return to !== undefined && toLayer !== undefined ? { to, toLayer } : undefined;
};

export const layers: Rule.RuleModule = {
  meta: {
    type: "problem",
    docs: { description: "Keep src/lib below src/components below the app, and give every file in src a layer" },
    messages: {
      crosses:
        "{{from}} ({{fromLayer}}) imports {{to}} ({{toLayer}}): move what both need into src/lib, or hand it down from the app → GAUNTLET.md#architecture",
      unclassified: "{{file}} has no layer: put it in src/lib or src/components, or make it a test → GAUNTLET.md#architecture",
    },
    schema: [],
  },
  create(context) {
    const file = posix(relative(context.cwd, context.filename));
    const layer = layerOf(file);

    if (!file.startsWith("src/")) {
      return {};
    }

    if (layer === undefined) {
      return {
        Program: (node) => {
          context.report({ node, messageId: "unclassified", data: { file } });
        },
      };
    }

    const check = (node: Node) => {
      const target = targetOf(file, node);

      if (target && !MAY_USE[layer].includes(target.toLayer)) {
        context.report({ node, messageId: "crosses", data: { from: file, fromLayer: layer, ...target } });
      }
    };

    return { ImportDeclaration: check, ExportAllDeclaration: check, ExportNamedDeclaration: check, ImportExpression: check };
  },
};
