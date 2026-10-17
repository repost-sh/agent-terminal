import type { ActivityEntry, CommandRecord, FrameUpdate, Screenshot, SessionInfo, TabInfo } from "../lib/protocol";

export const tabInfo = (tab: string, change: Partial<TabInfo> = {}): TabInfo => ({
  tab,
  program: "bash",
  command: "bash",
  cwd: "/home/me",
  cols: 80,
  rows: 24,
  exited: false,
  exit_code: null,
  control: "agent",
  ...change,
});

export const sessionInfo = (session: string, tabs: TabInfo[]): SessionInfo => ({ session, current: tabs[0]?.tab ?? null, tabs });

export const frameOf = (frame: number, change: Partial<FrameUpdate> = {}): FrameUpdate => ({
  type: "frame",
  frame,
  cols: 80,
  rows: 24,
  paint: "",
  alt_screen: false,
  mouse: null,
  bracketed_paste: false,
  exited: false,
  exit_code: null,
  ...change,
});

export const entryOf = (command: string, change: Partial<ActivityEntry> = {}): ActivityEntry => ({
  time: new Date(2026, 0, 2, 3, 4, 5, 6).toISOString(),
  command,
  duration_ms: 12,
  exit: 0,
  output: "",
  ...change,
});

export const commandOf = (id: number, change: Partial<CommandRecord> = {}): CommandRecord => ({
  id,
  command: `step ${String(id)}`,
  cwd: "/home/me/project/src",
  started_at_ms: new Date(2026, 0, 2, 3, 4, 5).getTime() + id,
  ended_at_ms: new Date(2026, 0, 2, 3, 4, 6).getTime(),
  duration_ms: 1000,
  exit_code: 0,
  first_frame: 1,
  last_frame: 2,
  ...change,
});

export const shotOf = (name: string, change: Partial<Screenshot> = {}): Screenshot => ({
  time: new Date(2026, 0, 2, 3, 4, 5).toISOString(),
  tab: "app",
  frame: 3,
  path: `/home/me/.agent-terminal/work/${name}`,
  by: null,
  width: 960,
  height: 640,
  scale: 1,
  cols: 120,
  rows: { first: 0, last: 39 },
  ...change,
});
