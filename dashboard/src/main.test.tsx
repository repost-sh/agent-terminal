import { screen } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import { startDaemon } from "./fake-daemon/daemon";
import { sessionInfo, tabInfo } from "./fake-daemon/messages";

test("the page needs its root element", async () => {
  vi.resetModules();
  await expect(import("./main")).rejects.toThrow("index.html has no #root element");
});

test("the page renders the dashboard into its root", async () => {
  const daemon = startDaemon([sessionInfo("work", [tabInfo("app")])]);
  const root = document.createElement("div");

  root.id = "root";
  document.body.append(root);
  vi.resetModules();
  await import("./main");
  await daemon.connected("/api/sessions/stream");
  expect(await screen.findByRole("navigation", { name: "Sessions and tabs" })).toBeTruthy();
  root.remove();
});
