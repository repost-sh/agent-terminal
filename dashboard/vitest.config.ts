import { defineConfig, mergeConfig } from "vitest/config";

import vite from "./vite.config.ts";

export default mergeConfig(
  vite,
  defineConfig({
    test: {
      environment: "happy-dom",
      include: ["src/**/*.test.{ts,tsx}"],
      setupFiles: ["src/test-setup.ts"],
      restoreMocks: true,
      unstubGlobals: true,
      coverage: {
        provider: "v8",
        include: ["src/**/*.{ts,tsx}"],
        reporter: ["text-summary", "json"],
        thresholds: { lines: 90 },
      },
    },
  }),
);
