export const NAME = /^[a-z0-9][a-z0-9-]{0,23}$/;

export const NAME_RULE = "Use up to 24 of a-z, 0-9 and -, starting with a letter or digit.";

export interface TabInfo {
  tab: string;
  renamed_from?: string;
  program: string;
  command: string;
  cwd: string;
  cols: number;
  rows: number;
  exited: boolean;
  exit_code: number | null;
  control: "agent" | "human";
}

export interface SessionInfo {
  session: string;
  current: string | null;
  tabs: TabInfo[];
}

export interface Sessions {
  sessions: SessionInfo[];
}

export interface FrameUpdate {
  type: "frame";
  frame: number;
  cols: number;
  rows: number;
  paint: string;
  alt_screen: boolean;
  mouse: string | null;
  bracketed_paste: boolean;
  exited: boolean;
  exit_code: number | null;
}

export type StreamUpdate = FrameUpdate | { type: "closed" };

export interface ErrorBody {
  code: number;
  message: string;
}

export interface ActivityEntry {
  time: string;
  tab?: string;
  command: string;
  duration_ms: number;
  exit: number;
  error?: string;
  frame?: number;
  output: string;
  screenshots?: string[];
  human?: HumanInput;
}

export type HumanInput = { typed: string } | { pressed: string } | { pasted: string };

export interface PendingCall { time: string; tab?: string; command: string }

export type ActivityUpdate = { type: "entries"; entries: ActivityEntry[]; pending: PendingCall[] } | { type: "closed" };

export type ControlUpdate =
  | { type: "held" }
  | { type: "refused"; message: string }
  | { type: "agent-refused"; command: string }
  | { type: "closed" };

export interface CommandRecord {
  id: number;
  command: string;
  cwd: string;
  started_at_ms: number;
  ended_at_ms: number | null;
  duration_ms: number;
  exit_code: number | null;
  first_frame: number;
  last_frame: number | null;
}

export interface CommandsUpdate {
  type: "commands";
  shell: boolean;
  integration: boolean;
  home: string | null;
  commands: CommandRecord[];
}

export type CommandsStreamUpdate = CommandsUpdate | { type: "closed" };

export type CommandOutput = CommandRecord & { session: string; tab: string; output: string };

export interface Screenshot {
  time: string;
  tab: string;
  frame: number;
  path: string;
  by: string | null;
  width: number;
  height: number;
  scale: number;
  cols: number;
  rows: { first: number; last: number };
}

export type ScreenshotsUpdate = { type: "screenshots"; screenshots: Screenshot[] } | { type: "closed" };

export interface CellBox { x: number; y: number; width: number; height: number }

export interface Compared { changed: number | null; boxes: CellBox[] }
