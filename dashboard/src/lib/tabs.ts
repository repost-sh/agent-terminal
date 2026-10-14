import type { SessionInfo, TabInfo } from "./protocol";

export type TabState =
  | { kind: "running"; label: "running"; tone: "success" }
  | { kind: "exited"; label: string; tone: "destructive" | "muted" };

export const tabName = (session: string, tab: string) => `${session} ${tab}`;

export const tabState = (tab: TabInfo): TabState =>
  tab.exited
    ? { kind: "exited", label: tab.exit_code === null ? "exited" : `exited ${tab.exit_code}`, tone: tab.exit_code ? "destructive" : "muted" }
    : { kind: "running", label: "running", tone: "success" };

export const findTab = (sessions: SessionInfo[], session: string, tab: string) =>
  sessions.find((entry) => entry.session === session)?.tabs.find((entry) => entry.tab === tab) ?? null;

export const firstTab = (sessions: SessionInfo[]) => {
  const session = sessions.find((entry) => entry.tabs.length > 0);
  const tab = session?.tabs[0];
  return session && tab ? { session: session.session, tab: tab.tab } : null;
};
