import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import { App } from "../app";
import { startDaemon, type FakeDaemon } from "../fake-daemon/daemon";
import { entryOf, sessionInfo, shotOf, tabInfo } from "../fake-daemon/messages";
import type { ActivityEntry, PendingCall } from "../lib/protocol";

const ACTIVITY = "/api/session/work/t1/activity";

const shownActivity = async (tab = "t1") => {
  const daemon = startDaemon([sessionInfo("work", [tabInfo(tab)])]);

  render(<App />);
  await daemon.serve();
  fireEvent.click(screen.getByRole("tab", { name: "Activity" }));
  await daemon.connected(ACTIVITY.replace("t1", tab));

  return daemon;
};

interface Update { entries: ActivityEntry[]; pending?: PendingCall[] }

const pushed = async (daemon: FakeDaemon, update: Update) => {
  await daemon.push(ACTIVITY, { type: "entries", entries: update.entries, pending: update.pending ?? [] });
};

const calls = () => within(screen.getByRole("list", { name: "Calls to work t1" })).getAllByRole("listitem").map((row) => row.textContent);

test("calls list their time, command, duration and result, with pending calls last", async () => {
  const daemon = await shownActivity();

  expect(screen.getByText("Waiting for calls...")).toBeTruthy();
  await pushed(daemon, {
    entries: [entryOf("snapshot", { duration_ms: 1500 }), entryOf("wait --text ready", { exit: 3, error: "timed out" })],
    pending: [{ time: new Date(2026, 0, 2, 3, 4, 6, 0).toISOString(), command: "wait --idle" }],
  });
  expect(calls()).toEqual(["03:04:05.006snapshot 1.5 s", "03:04:05.006wait --text ready 12 mstimeout", "03:04:06.000wait --idlewaiting"]);
  expect(within(screen.getByRole("region", { name: "Details of work t1" })).getByText("3")).toBeTruthy();
  await pushed(daemon, { entries: [entryOf("type", { human: { typed: "hi" } })] });
  expect(calls()).toEqual(["03:04:05.006snapshot 1.5 s", "03:04:05.006wait --text ready 12 mstimeout", '03:04:05.006human: typed "hi"']);
});

test("a call with output opens and closes it, and the list stops following while it is open", async () => {
  const daemon = await shownActivity();

  await pushed(daemon, { entries: [entryOf("snapshot", { output: "the screen" }), entryOf("press x")] });
  const call = screen.getByRole("button", { name: /snapshot/ });

  expect(call.getAttribute("aria-expanded")).toBe("false");
  fireEvent.click(call);
  expect([call.getAttribute("aria-expanded"), screen.getByText("the screen").tagName]).toEqual(["true", "PRE"]);
  expect(screen.getByRole("button", { name: "Follow" }).getAttribute("aria-pressed")).toBe("false");
  fireEvent.click(screen.getByRole("button", { name: "Jump to latest" }));
  expect(screen.getByRole("button", { name: "Follow" }).getAttribute("aria-pressed")).toBe("true");
  fireEvent.click(call);
  expect(screen.queryByText("the screen")).toBeNull();
  expect(screen.getByRole("button", { name: /press x/ }).getAttribute("aria-expanded")).toBeNull();
});

test("scrolling away from the newest call stops following and scrolling back resumes", async () => {
  const daemon = await shownActivity();

  await pushed(daemon, { entries: [entryOf("snapshot")] });
  const list = screen.getByRole("list", { name: "Calls to work t1" });
  const follow = screen.getByRole("button", { name: "Follow" });

  Object.defineProperties(list, { scrollHeight: { value: 1000 }, clientHeight: { value: 200 } });
  fireEvent.scroll(list, { target: { scrollTop: 100 } });
  expect(follow.getAttribute("aria-pressed")).toBe("false");
  fireEvent.scroll(list, { target: { scrollTop: 790 } });
  expect(follow.getAttribute("aria-pressed")).toBe("true");
  fireEvent.click(follow);
  expect(follow.getAttribute("aria-pressed")).toBe("false");
});

test("clearing hides the calls so far until the list restarts", async () => {
  const daemon = await shownActivity("t9");
  const at = (minute: number) => new Date(2026, 0, 2, 3, minute).toISOString();

  await daemon.push("/api/session/work/t9/activity", { type: "entries", entries: [entryOf("one", { time: at(1) }), entryOf("two", { time: at(2) })], pending: [] });
  fireEvent.click(screen.getByRole("button", { name: "Clear activity" }));
  expect(screen.getByText("Waiting for calls...")).toBeTruthy();
  expect(screen.getByText("Activity cleared")).toBeTruthy();
  expect(screen.getByRole<HTMLButtonElement>("button", { name: "Clear activity" }).disabled).toBe(true);
  await daemon.push("/api/session/work/t9/activity", { type: "entries", entries: [entryOf("three", { time: at(3) })], pending: [] });
  expect(screen.getByRole("button", { name: /three/ })).toBeTruthy();
  expect(screen.queryByRole("button", { name: /two/ })).toBeNull();
});

const SHOTS = "/api/session/work/screenshots/stream";

const at = (minute: number) => new Date(2026, 0, 2, 3, minute).toISOString();

const list = () => screen.getByRole("list", { name: "Calls to work t1" });

test("each kind of call has its own look, and a screenshot it took opens in the lightbox when it is listed", async () => {
  const daemon = await shownActivity();

  await pushed(daemon, {
    entries: [
      entryOf("snapshot", { time: at(1) }),
      entryOf("open htop", { exit: 1, time: at(2) }),
      entryOf("press x", { exit: 4, error: "a human has control", time: at(3) }),
      entryOf("type", { human: { typed: "hi" }, time: at(4) }),
      entryOf("get text", { output: "the screen", time: at(5) }),
      entryOf("screenshot", { screenshots: ["a.png", "notes.txt", "B.PNG"], time: at(6) }),
    ],
    pending: [
      { time: at(7), command: "wait --idle" },
      { time: at(8), command: "wait --gone" },
    ],
  });
  await daemon.connected(SHOTS);
  await daemon.push(SHOTS, { type: "screenshots", screenshots: [shotOf("a.png", { path: "a.png" })] });
  expect(list().outerHTML).toMatchSnapshot();
  fireEvent.click(screen.getByRole("button", { name: /get text/ }));
  expect(list().outerHTML).toMatchSnapshot();
});

test("a call without output shows nothing more when clicked", async () => {
  const daemon = await shownActivity();

  await pushed(daemon, { entries: [entryOf("press x")], pending: [{ time: at(9), command: "wait --idle" }] });
  fireEvent.click(screen.getByRole("button", { name: /press x/ }));
  expect([list().querySelector("pre"), calls().length]).toEqual([null, 2]);
});

const scrolledTo = (element: HTMLElement) => {
  const writes: number[] = [];

  Object.defineProperties(element, {
    scrollHeight: { value: 1000, configurable: true },
    clientHeight: { value: 200, configurable: true },
    scrollTop: {
      configurable: true,
      get: () => writes.at(-1) ?? 0,
      set: (value: number) => {
        writes.push(value);
      },
    },
  });

  return writes;
};

test("a following list scrolls to each new call, to a call that grows, and back when following resumes", async () => {
  const daemon = await shownActivity();

  await pushed(daemon, { entries: [entryOf("screenshot", { screenshots: ["a.png"] })] });
  const writes = scrolledTo(list());

  await pushed(daemon, { entries: [entryOf("press x")] });
  expect(writes).toEqual([1000]);
  await daemon.connected(SHOTS);
  await daemon.push(SHOTS, { type: "screenshots", screenshots: [shotOf("a.png", { path: "a.png" })] });
  expect(writes).toEqual([1000, 1000]);
  fireEvent.scroll(list(), { target: { scrollTop: 760 } });
  expect(screen.getByRole("button", { name: "Follow" }).getAttribute("aria-pressed")).toBe("false");
  await pushed(daemon, { entries: [entryOf("press y")] });
  expect(writes).toEqual([1000, 1000, 760]);
  await act(async () => {
    fireEvent.click(screen.getByRole("button", { name: "Jump to latest" }));
    await Promise.resolve();
  });
  expect([writes.at(-1), document.activeElement]).toEqual([1000, screen.getByRole("button", { name: "Follow" })]);
});

test("a closed list stops watching its calls", async () => {
  const disconnected: string[] = [];

  vi.stubGlobal(
    "MutationObserver",
    class extends MutationObserver {
      override disconnect() {
        disconnected.push("disconnected");
        super.disconnect();
      }
    },
  );
  const daemon = await shownActivity();

  await pushed(daemon, { entries: [entryOf("snapshot")] });
  fireEvent.click(screen.getByRole("tab", { name: "Output" }));
  expect(disconnected.length).toBeGreaterThan(0);
});

test("clearing outlives switching panels, and a restarted list shows its calls again, collapsed", async () => {
  const daemon = await shownActivity();

  await pushed(daemon, { entries: [entryOf("get text", { output: "the screen", time: at(1) }), entryOf("press x", { time: at(2) })] });
  fireEvent.click(screen.getByRole("button", { name: /get text/ }));
  fireEvent.click(screen.getByRole("button", { name: "Clear activity" }));
  fireEvent.click(screen.getByRole("tab", { name: "Output" }));
  fireEvent.click(screen.getByRole("tab", { name: "Activity" }));
  await daemon.connected(ACTIVITY);
  await daemon.push(ACTIVITY, { type: "entries", entries: [entryOf("get text", { output: "the screen", time: at(1) }), entryOf("press x", { time: at(2) })], pending: [] });
  expect(screen.getByText("Waiting for calls...")).toBeTruthy();
  await daemon.hangUp(ACTIVITY);
  await daemon.connected(ACTIVITY);
  await daemon.push(ACTIVITY, { type: "entries", entries: [entryOf("get text", { output: "the screen", time: at(5) })], pending: [] });
  expect([screen.getByRole("button", { name: /get text/ }).getAttribute("aria-expanded"), screen.queryByText("the screen")]).toEqual(["false", null]);
});
