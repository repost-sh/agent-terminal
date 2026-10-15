import { expect, test } from "vitest";

import { sessionInfo, tabInfo as tab } from "../fake-daemon/messages";
import type { SessionInfo } from "./protocol";
import { findTab, firstTab, tabKey, tabName, tabState } from "./tabs";

test("a tab is running, held by a human, or exited with its code", () => {
  expect(
    [tab("t1"), tab("t1", { control: "human" }), tab("t1", { exited: true }), tab("t1", { exited: true, exit_code: 0 }), tab("t1", { exited: true, exit_code: 2 })].map(tabState),
  ).toEqual([
    { kind: "running", label: "running", tone: "success" },
    { kind: "human", label: "human", tone: "warning" },
    { kind: "exited", label: "exited", tone: "muted" },
    { kind: "exited", label: "exited 0", tone: "muted" },
    { kind: "exited", label: "exited 2", tone: "destructive" },
  ]);
});

test("tabs are named and keyed by session and tab", () => {
  expect(tabName("work", "t2")).toBe("work t2");
  expect(tabKey("a b", "t1")).not.toBe(tabKey("a", "b t1"));
});

const sessions: SessionInfo[] = [sessionInfo("empty", []), sessionInfo("work", [tab("t1"), tab("t2")])];

test("tabs are found by address and the first tab is the first of the first session with tabs", () => {
  expect(findTab(sessions, { session: "work", tab: "t2" })?.tab).toBe("t2");
  expect([findTab(sessions, { session: "work", tab: "t9" }), findTab(sessions, { session: "gone", tab: "t1" })]).toEqual([null, null]);
  expect(firstTab(sessions)).toEqual({ session: "work", tab: "t1" });
  expect(firstTab([sessionInfo("empty", [])])).toBeNull();
});
