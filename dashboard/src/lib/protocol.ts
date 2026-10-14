export type TabInfo = {
  tab: string;
  program: string;
  command: string;
  cwd: string;
  cols: number;
  rows: number;
  exited: boolean;
  exit_code: number | null;
  recording: string | null;
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
