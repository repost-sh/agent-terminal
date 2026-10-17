import { expect, test, vi } from "vitest";

import { closeAll, commandOutput, compareScreenshots, openTab, reason, socketUrl } from "./api";

interface Sent { path: string; init: RequestInit | undefined }

const answering = (status: number, text: string) => {
  const sent: Sent[] = [];

  vi.stubGlobal("fetch", (path: string, init?: RequestInit) => {
    sent.push({ path, init });

    return Promise.resolve(new Response(text, { status, statusText: status === 503 ? "Service Unavailable" : "" }));
  });

  return sent;
};

test("a failure reads as its message, or as itself when it is not an error", () => {
  expect([reason(new Error("denied")), reason("no clipboard"), reason(404)]).toEqual(["denied", "no clipboard", "404"]);
});

test("sockets use wss on an https page and ws otherwise, on the page's own host", () => {
  vi.stubGlobal("location", { protocol: "https:", host: "example.test:8443" });
  expect(socketUrl("/api/sessions/stream")).toBe("wss://example.test:8443/api/sessions/stream");
  vi.stubGlobal("location", { protocol: "http:", host: "localhost:9000" });
  expect(socketUrl("/x")).toBe("ws://localhost:9000/x");
});

test("a post sends JSON with its content type, or no body at all", async () => {
  const sent = answering(200, JSON.stringify({ data: { session: "work", tab: "app" } }));
  const opening = { session: "work", new_session: true, tab: "top", command: "htop", cwd: "~", cols: 80, rows: 24, shell_integration: false };

  expect(await openTab(opening)).toEqual({ session: "work", tab: "app" });
  await closeAll();
  expect(sent.map(({ path, init }) => [path, init?.method, init?.headers, init?.body])).toEqual([
    ["/api/sessions", "POST", { "content-type": "application/json" }, JSON.stringify(opening)],
    ["/api/sessions/close", "POST", { "content-type": "application/json" }, undefined],
  ]);
});

test("a refused call fails with the daemon's message, its text, or its status", async () => {
  answering(400, JSON.stringify({ error: { message: "no such tab" } }));
  await expect(closeAll()).rejects.toThrow("no such tab");
  answering(500, "plain words");
  await expect(commandOutput({ session: "s", tab: "app" }, 3)).rejects.toThrow("plain words");
  answering(503, "");
  await expect(compareScreenshots("s", { older: "a.png", newer: "b.png" })).rejects.toThrow("503 Service Unavailable");
});

test("comparing screenshots posts both paths as JSON", async () => {
  const sent = answering(200, JSON.stringify({ changed: 1, boxes: [] }));

  expect(await compareScreenshots("my work", { older: "a.png", newer: "b.png" })).toEqual({ changed: 1, boxes: [] });
  expect(sent.map(({ path, init }) => [path, init?.method, init?.headers, init?.body])).toEqual([
    ["/api/session/my%20work/screenshots/compare", "POST", { "content-type": "application/json" }, JSON.stringify({ paths: ["a.png", "b.png"] })],
  ]);
});
