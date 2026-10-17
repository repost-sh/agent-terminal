import { expect, test } from "vitest";

import { sessionInfo, tabInfo as tab } from "../fake-daemon/messages";
import type { SessionInfo } from "./protocol";
import { findTab, firstTab, tabKey, tabName, tabState } from "./tabs";

test("a tab is running, held by a human, or exited with its code", () => {
  expect(
    [tab("app"), tab("app", { control: "human" }), tab("app", { exited: true }), tab("app", { exited: true, exit_code: 0 }), tab("app", { exited: true, exit_code: 2 })].map(tabState),
  ).toEqual([
    { kind: "running", label: "running", tone: "success" },
    { kind: "human", label: "human", tone: "warning" },
    { kind: "exited", label: "exited", tone: "muted" },
    { kind: "exited", label: "exited 0", tone: "muted" },
    { kind: "exited", label: "exited 2", tone: "destructive" },
  ]);
});

test("tabs are named and keyed by session and tab", () => {
  expect(tabName("work", "api")).toBe("work api");
  expect(tabKey("a b", "app")).not.toBe(tabKey("a", "b app"));
});

const sessions: SessionInfo[] = [sessionInfo("empty", []), sessionInfo("work", [tab("app"), tab("api")])];

test("tabs are found by address and the first tab is the first of the first session with tabs", () => {
  expect(findTab(sessions, { session: "work", tab: "api" })?.tab).toBe("api");
  expect([findTab(sessions, { session: "work", tab: "t9" }), findTab(sessions, { session: "gone", tab: "app" })]).toEqual([null, null]);
  expect(firstTab(sessions)).toEqual({ session: "work", tab: "app" });
  expect(firstTab([sessionInfo("empty", [])])).toBeNull();
});
