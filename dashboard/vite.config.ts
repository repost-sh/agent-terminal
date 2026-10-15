import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

const dashboard = process.env.AGENT_TERMINAL_DASHBOARD ?? "http://localhost:4849";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  build: {
    outDir: "../cli/assets/dashboard",
    emptyOutDir: true,
    sourcemap: false,
  },
  server: {
    proxy: {
      "/api": { target: dashboard, ws: true, changeOrigin: true },
    },
  },
});
