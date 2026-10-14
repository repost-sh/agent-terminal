import { closeTab, restartTab } from "../lib/api";
import type { SessionInfo, TabInfo } from "../lib/protocol";
import { navigate, tabRoute } from "../lib/route";
import { useTabStream } from "../lib/stream";
import { firstTab, tabName } from "../lib/tabs";
import { useXterm } from "../lib/xterm";
import { announce } from "./announcer";
import { StatusBar } from "./status-bar";
import { Viewport } from "./terminal-view";
import { toast } from "./toaster";

const reason = (error: unknown) => (error instanceof Error ? error.message : String(error));

type TabScreenProps = { session: string; tab: TabInfo; sessions: SessionInfo[] };

export const TabScreen = ({ session, tab, sessions }: TabScreenProps) => {
  const terminal = useXterm();
  const { state, shown } = useTabStream(session, tab.tab, terminal);
  const name = tabName(session, tab.tab);

  const restart = async () => {
    try {
      await restartTab(session, tab.tab);
      announce(`${tab.program} restarted`);
      document.getElementById("main-content")?.focus({ preventScroll: true });
    } catch (error) {
      toast("error", `Could not restart ${tab.program}`, reason(error));
    }
  };

  const close = async () => {
    try {
      await closeTab(session, tab.tab);
      const left = sessions.map((entry) => ({ ...entry, tabs: entry.tabs.filter((other) => entry.session !== session || other.tab !== tab.tab) }));
      const next = firstTab(left);
      navigate(next ? tabRoute(next.session, next.tab) : "/");
      announce(`Closed ${name}`);
    } catch (error) {
      toast("error", `Could not close ${name}`, reason(error));
    }
  };

  return (
    <>
      <Viewport
        terminal={terminal}
        state={state}
        shown={shown}
        program={tab.program}
        label={`${name} ${tab.program}, live frame ${shown?.frame ?? 0}`}
        onRestart={restart}
        onClose={close}
      />
      <div className="h-px shrink-0 bg-border" role="none" />
      <StatusBar session={session} tab={tab} state={state} shown={shown} />
    </>
  );
};
