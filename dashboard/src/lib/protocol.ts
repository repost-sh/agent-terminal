export type TabInfo = {
  tab: string;
  program: string;
  cols: number;
  rows: number;
  exited: boolean;
  exit_code: number | null;
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

export type ErrorBody = {
  code: number;
  message: string;
};

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
