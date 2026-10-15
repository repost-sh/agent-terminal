export type TabInfo = {
  tab: string;
  program: string;
  command: string;
  cwd: string;
  cols: number;
  rows: number;
  exited: boolean;
  exit_code: number | null;
  control: "agent" | "human";
};

export type SessionInfo = {
  session: string;
  current: string | null;
  tabs: TabInfo[];
};

export type Sessions = {
  sessions: SessionInfo[];
};

export type FrameUpdate = {
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
};

export type StreamUpdate = FrameUpdate | { type: "closed" };

export type TimelineFrame = {
  frame: number;
  time_ms: number;
  keyframe: boolean;
  cols: number;
  rows: number;
  paint: string;
  alt_screen: boolean;
  mouse: string | null;
  bracketed_paste: boolean;
};

export type TimelineUpdate =
  | { type: "frames"; reset: boolean; started_ms: number; frames: TimelineFrame[] }
  | { type: "failed"; message: string }
  | { type: "closed" };

export type ErrorBody = {
  code: number;
  message: string;
};

export type ActivityEntry = {
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
};

export type HumanInput = { typed: string } | { pressed: string } | { pasted: string };

export type PendingCall = { time: string; tab?: string; command: string };

export type ActivityUpdate = { type: "entries"; entries: ActivityEntry[]; pending: PendingCall[] } | { type: "closed" };

export type ControlUpdate =
  | { type: "held" }
  | { type: "refused"; message: string }
  | { type: "agent-refused"; command: string }
  | { type: "closed" };

export type CommandRecord = {
  id: number;
  command: string;
  cwd: string;
  started_at_ms: number;
  ended_at_ms: number | null;
  duration_ms: number;
  exit_code: number | null;
  first_frame: number;
  last_frame: number | null;
};

export type CommandsUpdate = {
  type: "commands";
  shell: boolean;
  integration: boolean;
  home: string | null;
  commands: CommandRecord[];
};

export type CommandsStreamUpdate = CommandsUpdate | { type: "closed" };

export type CommandOutput = CommandRecord & { session: string; tab: string; output: string };

export type Screenshot = {
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
};

export type ScreenshotsUpdate = { type: "screenshots"; screenshots: Screenshot[] } | { type: "closed" };

export type CellBox = { x: number; y: number; width: number; height: number };

export type Compared = { changed: number | null; boxes: CellBox[] };
