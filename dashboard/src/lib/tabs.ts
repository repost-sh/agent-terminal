import type { SessionInfo, TabInfo } from "./protocol";

export type TabState =
  | { kind: "running"; label: "running"; tone: "success" }
  | { kind: "human"; label: "human"; tone: "warning" }
  | { kind: "exited"; label: string; tone: "destructive" | "muted" };

export interface TabAddress { session: string; tab: string }

export const tabName = (session: string, tab: string) => `${session} ${tab}`;

export const tabKey = (session: string, tab: string) => JSON.stringify([session, tab]);

export const tabState = (tab: TabInfo): TabState => {
  if (tab.exited) {
    return { kind: "exited", label: tab.exit_code === null ? "exited" : `exited ${String(tab.exit_code)}`, tone: tab.exit_code ? "destructive" : "muted" };
  }

  if (tab.control === "human") {
    return { kind: "human", label: "human", tone: "warning" };
  }

  return { kind: "running", label: "running", tone: "success" };
};

export const findTab = (sessions: SessionInfo[], address: TabAddress) =>
  sessions.find((entry) => entry.session === address.session)?.tabs.find((entry) => entry.tab === address.tab) ?? null;

export const firstTab = (sessions: SessionInfo[]) => {
  const session = sessions.find((entry) => entry.tabs.length > 0);
  const tab = session?.tabs[0];

  return session && tab ? { session: session.session, tab: tab.tab } : null;
};
