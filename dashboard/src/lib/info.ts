import { tabFeedPath } from "./api";
import { useFeed } from "./feed";

export type Modes = {
  alt_screen: boolean;
  mouse: { tracking: string; encoding: string } | null;
  bracketed_paste: boolean;
  app_cursor: boolean;
  focus_events: boolean;
  keyboard: string[];
};

export type TabSetup = {
  program: string;
  args: string;
  pid: number | null;
  cwd: string;
  term: string | null;
  cols: number;
  rows: number;
  shell_integration: boolean | null;
  modes: Modes;
  recording: string | null;
  started: string;
  exited: boolean;
  exit_code: number | null;
  env: { name: string; value: string | null }[];
};

type InfoUpdate = ({ type: "info" } & TabSetup) | { type: "closed" };

const latest = (setup: TabSetup | null, update: InfoUpdate) => (update.type === "info" ? update : setup);

export const useInfo = (session: string, tab: string) => useFeed<TabSetup | null, InfoUpdate>(tabFeedPath(session, tab, "info"), null, latest).value;
