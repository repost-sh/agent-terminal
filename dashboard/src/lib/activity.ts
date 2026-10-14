import { tabFeedPath } from "./api";
import { useFeed } from "./feed";
import type { ActivityEntry, ActivityUpdate } from "./protocol";

const NONE: ActivityEntry[] = [];

const appended = (entries: ActivityEntry[], update: ActivityUpdate, first: boolean) =>
  update.type === "entries" ? (first ? update.entries : [...entries, ...update.entries]) : entries;

export const useActivity = (session: string, tab: string) => useFeed(tabFeedPath(session, tab, "activity"), NONE, appended).value;

export type Result = { label: string; tone: "destructive" | "warning" };

const FAILED: Record<number, string> = { 2: "exit 2 expect failed", 3: "timeout", 5: "exit 5 process exited" };

export const resultOf = (entry: ActivityEntry): Result | null => {
  if (entry.exit === 0) return null;
  if (entry.error === undefined) return { label: `exit ${entry.exit}`, tone: "destructive" };
  if (entry.exit === 4) return { label: "refused: human has control", tone: "warning" };
  return { label: FAILED[entry.exit] ?? `exit ${entry.exit}`, tone: "destructive" };
};
