import { tabFeedPath } from "./api";
import { useFeed, type Received } from "./feed";
import type { ActivityEntry, ActivityUpdate, HumanInput, PendingCall } from "./protocol";

interface Activity { entries: ActivityEntry[]; pending: PendingCall[] }

const NONE: Activity = { entries: [], pending: [] };

const appended = (activity: Activity, received: Received<ActivityUpdate>): Activity => {
  const update = received.message;

  if (update.type !== "entries") {
    return activity;
  }

  return { entries: received.first ? update.entries : [...activity.entries, ...update.entries], pending: update.pending };
};

export const useActivity = (session: string, tab: string) =>
  useFeed(tabFeedPath({ session, tab }, "activity"), { initial: NONE, reduce: appended }).value;

export interface Result { label: string; tone: "destructive" | "warning" }

const FAILED: Record<number, string> = { 2: "exit 2 expect failed", 3: "timeout", 5: "exit 5 process exited" };

export const resultOf = (entry: ActivityEntry): Result | null => {
  if (entry.exit === 0) {
    return null;
  }

  if (entry.error === undefined) {
    return { label: `exit ${String(entry.exit)}`, tone: "destructive" };
  }

  if (entry.exit === 4) {
    return { label: "refused: human has control", tone: "warning" };
  }

  return { label: FAILED[entry.exit] ?? `exit ${String(entry.exit)}`, tone: "destructive" };
};

export interface ActivityRow { at: number; entry: ActivityEntry; label: string }

export const labelOf = (call: ActivityEntry | PendingCall) => call.action ?? call.command;

interface Typing { row: ActivityRow; text: string }

interface Building { rows: ActivityRow[]; typing: Typing | null }

interface Placed { at: number; entry: ActivityEntry }

interface TypedAt extends Placed { typed: string }

const showTyped = (typing: Typing) => {
  typing.row.label = `typed "${typing.text}"`;

  return typing;
};

const withoutLastCodePoint = (text: string) => Array.from(text).slice(0, -1).join("");

const isBackspace = (human: HumanInput | undefined) => human !== undefined && "pressed" in human && human.pressed === "Backspace";

const typedInto = (building: Building, placed: TypedAt) => {
  if (building.typing) {
    building.typing.text += placed.typed;

    return showTyped(building.typing);
  }

  const typing = { row: { at: placed.at, entry: placed.entry, label: labelOf(placed.entry) }, text: placed.typed };

  building.rows.push(typing.row);

  return typing;
};

const succeededInput = (entry: ActivityEntry) => (entry.exit === 0 ? entry.human : undefined);

const plainRow = (placed: Placed): ActivityRow => ({ at: placed.at, entry: placed.entry, label: labelOf(placed.entry) });

const nextTyping = (building: Building, placed: Placed): Typing | null => {
  const human = succeededInput(placed.entry);

  if (human && "typed" in human) {
    return typedInto(building, { ...placed, typed: human.typed });
  }

  if (building.typing?.text && isBackspace(human)) {
    building.typing.text = withoutLastCodePoint(building.typing.text);

    return showTyped(building.typing);
  }

  building.rows.push(plainRow(placed));

  return null;
};

export const activityRows = (entries: ActivityEntry[], first: number): ActivityRow[] => {
  const building: Building = { rows: [], typing: null };

  for (const [index, entry] of entries.entries()) {
    building.typing = nextTyping(building, { at: first + index, entry });
  }

  return building.rows;
};
