import { tabFeedPath } from "./api";
import { useFeed } from "./feed";
import type { ActivityEntry, ActivityUpdate, HumanInput, PendingCall } from "./protocol";

type Activity = { entries: ActivityEntry[]; pending: PendingCall[] };

const NONE: Activity = { entries: [], pending: [] };

const appended = (activity: Activity, update: ActivityUpdate, first: boolean): Activity =>
  update.type === "entries" ? { entries: first ? update.entries : [...activity.entries, ...update.entries], pending: update.pending } : activity;

export const useActivity = (session: string, tab: string) => useFeed(tabFeedPath(session, tab, "activity"), NONE, appended).value;

export type Result = { label: string; tone: "destructive" | "warning" };

const FAILED: Record<number, string> = { 2: "exit 2 expect failed", 3: "timeout", 5: "exit 5 process exited" };

export const resultOf = (entry: ActivityEntry): Result | null => {
  if (entry.exit === 0) return null;
  if (entry.error === undefined) return { label: `exit ${entry.exit}`, tone: "destructive" };
  if (entry.exit === 4) return { label: "refused: human has control", tone: "warning" };
  return { label: FAILED[entry.exit] ?? `exit ${entry.exit}`, tone: "destructive" };
};

export type ActivityRow = { at: number; entry: ActivityEntry; human: string | null };

const quoted = (text: string) => `"${text}"`;

const humanAction = (human: HumanInput) => {
  if ("typed" in human) return `typed ${quoted(human.typed)}`;
  if ("pasted" in human) return `pasted ${quoted(human.pasted)}`;
  return `pressed ${human.pressed}`;
};

export const activityRows = (entries: ActivityEntry[], first: number): ActivityRow[] => {
  const rows: ActivityRow[] = [];
  let typing: { row: ActivityRow; text: string } | null = null;
  for (const [index, entry] of entries.entries()) {
    const human = entry.exit === 0 ? entry.human : undefined;
    if (human && "typed" in human) {
      if (typing) typing.text += human.typed;
      else {
        typing = { row: { at: first + index, entry, human: "" }, text: human.typed };
        rows.push(typing.row);
      }
      typing.row.human = `typed ${quoted(typing.text)}`;
      continue;
    }
    if (typing && human && "pressed" in human && human.pressed === "Backspace" && typing.text) {
      typing.text = [...typing.text].slice(0, -1).join("");
      typing.row.human = `typed ${quoted(typing.text)}`;
      continue;
    }
    typing = null;
    rows.push({ at: first + index, entry, human: entry.human ? humanAction(entry.human) : null });
  }
  return rows;
};
