import { join } from "node:path";
import { test } from "node:test";

import { RuleTester } from "eslint";

import { layers } from "./layers.ts";

const at = (path: string) => join(process.cwd(), path);

const crosses = (from: string, to: string) => {
  const layerOf = (path: string) => (path.startsWith("src/lib/") ? "lib" : path.startsWith("src/components/") ? "components" : path.includes("fake-daemon") || path.includes("test-setup") ? "tests" : "app");

  return { messageId: "crosses", data: { from, fromLayer: layerOf(from), to, toLayer: layerOf(to) } };
};

void test("each layer imports only the layers below it, tests import anything, and code outside src is not judged", () => {
  new RuleTester().run("layers", layers, {
    valid: [
      { filename: at("src/components/a.tsx"), code: 'import { x } from "../lib/b"; import { y } from "./c";' },
      { filename: at("src/app.tsx"), code: 'import { x } from "./components/a"; import { y } from "./lib/b";' },
      { filename: at("src/main.tsx"), code: 'import "./index.css"; import { App } from "./app";' },
      { filename: at("src/lib/a.ts"), code: 'import { useState } from "react"; import { b } from "./b";' },
      { filename: at("src/lib/a.test.ts"), code: 'import { x } from "../components/y"; import { App } from "../app";' },
      { filename: at("src/fake-daemon/server.ts"), code: 'import { x } from "../components/y";' },
      { filename: at("src/test-setup.ts"), code: 'import { x } from "./components/y";' },
      { filename: at("eslint/plugin.ts"), code: 'import { x } from "../src/components/y";' },
    ],
    invalid: [
      { filename: at("src/lib/a.ts"), code: 'import { x } from "../components/y";', errors: [crosses("src/lib/a.ts", "src/components/y")] },
      { filename: at("src/lib/a.ts"), code: 'export { x } from "../components/y";', errors: [crosses("src/lib/a.ts", "src/components/y")] },
      { filename: at("src/lib/a.ts"), code: 'export * from "../app";', errors: [crosses("src/lib/a.ts", "src/app")] },
      { filename: at("src/lib/a.ts"), code: 'const y = import("../components/y");', errors: [crosses("src/lib/a.ts", "src/components/y")] },
      { filename: at("src/components/a.tsx"), code: 'import { App } from "../app";', errors: [crosses("src/components/a.tsx", "src/app")] },
      { filename: at("src/components/a.tsx"), code: 'import { serve } from "../fake-daemon/server";', errors: [crosses("src/components/a.tsx", "src/fake-daemon/server")] },
      { filename: at("src/app.tsx"), code: 'import "./test-setup";', errors: [crosses("src/app.tsx", "src/test-setup")] },
    ],
  });
});

void test("a file in src without a layer is reported", () => {
  new RuleTester().run("layers", layers, {
    valid: [],
    invalid: [
      { filename: at("src/panels.tsx"), code: "export const PANELS = [];", errors: [{ messageId: "unclassified", data: { file: "src/panels.tsx" } }] },
      { filename: at("src/util/format.ts"), code: "export const f = 1;", errors: [{ messageId: "unclassified", data: { file: "src/util/format.ts" } }] },
    ],
  });
});
