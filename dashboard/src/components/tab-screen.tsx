import { closeTab, reason, restartTab } from "../lib/api";
import type { SessionInfo, TabInfo } from "../lib/protocol";
import { navigate, tabRoute } from "../lib/route";
import { useTabStream } from "../lib/stream";
import { firstTab, tabName } from "../lib/tabs";
import { useXterm } from "../lib/xterm";
import { announce } from "./announcer";
import { StatusBar } from "./status-bar";
import { Viewport } from "./terminal-view";
import { toast } from "./toaster";
import { Toolbar } from "./toolbar";

type TabScreenProps = { session: string; tab: TabInfo; sessions: SessionInfo[] };

export const TabScreen = ({ session, tab, sessions }: TabScreenProps) => {
  const terminal = useXterm();
  const { state, shown } = useTabStream(session, tab.tab, terminal);
  const name = tabName(session, tab.tab);

  const restart = async () => {
    try {
      await restartTab(session, tab.tab);
      announce(`${tab.program} restarted`);
      return true;
    } catch (error) {
      toast("error", `Could not restart ${tab.program}`, reason(error));
      return false;
    }
  };

  const restartFromExitBar = async () => {
    if (await restart()) document.getElementById("main-content")?.focus({ preventScroll: true });
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
      <Toolbar session={session} tab={tab} stream={state} shown={shown} onRestart={restart} />
      <div className="h-px shrink-0 bg-border" role="none" />
      <Viewport
        terminal={terminal}
        state={state}
        shown={shown}
        program={tab.program}
        label={`${name} ${tab.program}, live frame ${shown?.frame ?? 0}`}
        onRestart={restartFromExitBar}
        onClose={close}
      />
      <div className="h-px shrink-0 bg-border" role="none" />
      <StatusBar session={session} tab={tab} state={state} shown={shown} />
    </>
  );
};
