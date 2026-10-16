import { tabFeedPath } from "./api";
import { useFeed, type Received } from "./feed";

export interface Modes {
  alt_screen: boolean;
  mouse: { tracking: string; encoding: string } | null;
  bracketed_paste: boolean;
  app_cursor: boolean;
  focus_events: boolean;
  keyboard: string[];
}

export interface TabSetup {
  program: string;
  args: string;
  pid: number | null;
  cwd: string;
  term: string | null;
  cols: number;
  rows: number;
  shell_integration: boolean | null;
  modes: Modes;
  started: string;
  exited: boolean;
  exit_code: number | null;
  env: { name: string; value: string | null }[];
}

type InfoUpdate = ({ type: "info" } & TabSetup) | { type: "closed" };

const latest = (setup: TabSetup | null, received: Received<InfoUpdate>) => (received.message.type === "info" ? received.message : setup);

export const useInfo = (session: string, tab: string) =>
  useFeed<TabSetup | null, InfoUpdate>(tabFeedPath({ session, tab }, "info"), { initial: null, reduce: latest }).value;
