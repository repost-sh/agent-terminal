import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import { App } from "../app";
import { startDaemon, type FakeDaemon } from "../fake-daemon/daemon";
import { entryOf, frameOf, sessionInfo, shotOf, tabInfo } from "../fake-daemon/messages";
import type { Screenshot } from "../lib/protocol";
import { comparisonOf } from "../lib/screenshots";

const SHOTS = "/api/session/work/screenshots/stream";

const shownApp = async () => {
  const daemon = startDaemon([sessionInfo("work", [tabInfo("t1")])]);

  render(<App />);
  await daemon.serve();

  return daemon;
};

const a = shotOf("a.png", { by: "snapshot --png" });
const b = shotOf("b.png", { frame: 4 });
const c = shotOf("c.png", { frame: 5, width: 480 });
const shots = [a, b, c];

const listed = async (daemon: FakeDaemon, list: Screenshot[]) => {
  fireEvent.click(screen.getByRole("tab", { name: "Screenshots" }));
  await daemon.connected(SHOTS);
  await daemon.push(SHOTS, { type: "screenshots", screenshots: list });
};

const lightbox = () => screen.getByRole("dialog");

const position = () => within(lightbox()).getByText(/ of /).textContent;

test("the panel lists each screenshot with its tab, frame, time and maker", async () => {
  const daemon = await shownApp();

  await listed(daemon, []);
  expect(screen.getByText("No screenshots yet")).toBeTruthy();
  await daemon.push(SHOTS, { type: "screenshots", screenshots: shots });
  const items = within(screen.getByRole("list", { name: "Screenshots of work" })).getAllByRole("listitem");

  expect(items.map((item) => item.textContent)).toEqual([
    "a.pngt1 · frame 3 · 03:04:05snapshot --png",
    "b.pngt1 · frame 4 · 03:04:05dashboard",
    "c.pngt1 · frame 5 · 03:04:05dashboard",
  ]);
});

test("the lightbox steps through the screenshots and copies a path", async () => {
  const daemon = await shownApp();
  const writeText = vi.fn(() => Promise.resolve());

  vi.stubGlobal("navigator", { clipboard: { writeText } });
  await listed(daemon, shots);
  fireEvent.click(screen.getByRole("button", { name: /a\.png/ }));
  expect([within(lightbox()).getByRole("img").getAttribute("alt"), position()]).toEqual(["a.png, frame 3", "1 of 3"]);
  expect(within(lightbox()).getByRole<HTMLButtonElement>("button", { name: "Previous screenshot" }).disabled).toBe(true);
  fireEvent.click(within(lightbox()).getByRole("button", { name: "Next screenshot" }));
  fireEvent.click(within(lightbox()).getByRole("button", { name: "Next screenshot" }));
  expect([position(), within(lightbox()).getByRole<HTMLButtonElement>("button", { name: "Next screenshot" }).disabled]).toEqual(["3 of 3", true]);
  fireEvent.click(within(lightbox()).getByRole("button", { name: "Previous screenshot" }));
  expect(position()).toBe("2 of 3");
  fireEvent.click(within(lightbox()).getByRole("button", { name: "Copy path" }));
  expect(await screen.findByText("Copied")).toBeTruthy();
  expect(writeText).toHaveBeenCalledWith("/home/me/.agent-terminal/work/b.png");
  fireEvent.keyDown(lightbox(), { key: "Escape" });
});

interface Pair { shown: Screenshot; other: Screenshot }

const compareWith = async (pair: Pair) => {
  fireEvent.click(within(lightbox()).getByRole("button", { name: "Compare with…" }));
  const select = within(lightbox()).getByLabelText("Compare with");
  const [older, newer] = pair.shown.frame < pair.other.frame ? [pair.shown, pair.other] : [pair.other, pair.shown];

  expect(document.activeElement).toBe(select);
  fireEvent.change(select, { target: { value: "" } });
  await act(async () => {
    fireEvent.change(select, { target: { value: pair.other.path } });
    await Promise.resolve();
  });
  await act(async () => {
    await comparisonOf("work", { older, newer });
  });
};

test("comparing two screenshots shows both and the cells that changed", async () => {
  const daemon = await shownApp();

  daemon.answer("POST /api/session/work/screenshots/compare", (request) => {
    const [older] = (request.body as { paths: string[] }).paths;

    return older?.endsWith("a.png") ? { status: 200, body: { changed: 2, boxes: [{ x: 0, y: 0, width: 96, height: 64 }] } } : { status: 200, body: { changed: null, boxes: [] } };
  });
  await listed(daemon, shots);
  fireEvent.click(screen.getByRole("button", { name: /b\.png/ }));
  await compareWith({ shown: b, other: a });
  expect(await within(lightbox()).findByText("2 cells changed")).toBeTruthy();
  expect(within(lightbox()).getByRole("heading").textContent).toBe("b.png compared");
  expect(within(lightbox()).getAllByRole("img").map((image) => image.getAttribute("alt"))).toEqual(["a.png, frame 3", "b.png, frame 4"]);
  expect(Array.from(lightbox().querySelectorAll<HTMLElement>("[data-changed]")).map((box) => box.style.width)).toEqual(["10%", "10%"]);
  fireEvent.click(within(lightbox()).getByRole("button", { name: "Back to screenshot" }));
  await compareWith({ shown: b, other: c });
  expect(await within(lightbox()).findByText("Different sizes: no cell comparison")).toBeTruthy();
  fireEvent.keyDown(lightbox(), { key: "Escape" });
});

test("a failed comparison says why, and one screenshot cannot be compared", async () => {
  const daemon = await shownApp();

  daemon.answer("POST /api/session/work/screenshots/compare", () => ({ status: 422, body: { error: { code: 1, message: "not a screenshot" } } }));
  const d = shotOf("d.png");
  const e = shotOf("e.png", { frame: 9 });

  await listed(daemon, [d, e]);
  fireEvent.click(screen.getByRole("button", { name: /d\.png/ }));
  await compareWith({ shown: d, other: e });
  expect(await within(lightbox()).findByText("not a screenshot")).toBeTruthy();
  fireEvent.keyDown(lightbox(), { key: "Escape" });
  await daemon.push(SHOTS, { type: "screenshots", screenshots: [d] });
  fireEvent.click(screen.getByRole("button", { name: /d\.png/ }));
  expect(within(lightbox()).getByRole<HTMLButtonElement>("button", { name: "Compare with…" }).disabled).toBe(true);
  fireEvent.keyDown(lightbox(), { key: "Escape" });
});

test("the toolbar takes a screenshot once the tab is drawn, and an activity call shows its pictures", async () => {
  const daemon = await shownApp();
  const failed = vi.spyOn(console, "error").mockImplementation(() => undefined);

  daemon.answer("POST /api/session/work/t1/screenshot", () => ({ status: 200, body: { data: { path: "/tmp/shots/new.png" } } }));
  expect(screen.getByRole<HTMLButtonElement>("button", { name: "Take a screenshot" }).disabled).toBe(true);
  await daemon.connected("/api/session/work/t1/stream");
  await daemon.push("/api/session/work/t1/stream", frameOf(1));
  expect(await screen.findByText("Live")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Take a screenshot" }));
  expect(await screen.findByText("Saved new.png")).toBeTruthy();
  daemon.answer("POST /api/session/work/t1/screenshot", () => ({ status: 500, body: "disk full" }));
  fireEvent.click(screen.getByRole("button", { name: "Take a screenshot" }));
  await vi.waitFor(() => {
    expect(failed).toHaveBeenCalledWith("Could not take a screenshot of work t1: disk full");
  });
  fireEvent.click(screen.getByRole("tab", { name: "Activity" }));
  await daemon.connected("/api/session/work/t1/activity");
  await daemon.push("/api/session/work/t1/activity", { type: "entries", entries: [entryOf("screenshot", { screenshots: [a.path, "/x/other.png", "/x/notes.txt"] })], pending: [] });
  await daemon.connected(SHOTS);
  await daemon.push(SHOTS, { type: "screenshots", screenshots: [a] });
  expect(screen.getByRole("link", { name: "Open screenshot other.png" }).getAttribute("href")).toBe("/api/session/work/screenshots/%2Fx%2Fother.png");
  fireEvent.click(screen.getByRole("button", { name: "Open screenshot a.png" }));
  expect(within(lightbox()).getByRole("img").getAttribute("alt")).toBe("a.png, frame 3");
  fireEvent.keyDown(lightbox(), { key: "Escape" });
});
