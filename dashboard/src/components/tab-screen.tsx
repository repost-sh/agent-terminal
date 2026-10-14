import { closeTab, restartTab } from "../lib/api";
import type { SessionInfo, TabInfo } from "../lib/protocol";
import { navigate, tabRoute } from "../lib/route";
import type { Layout } from "../lib/split";
import { useTabStream } from "../lib/stream";
import { firstTab, tabName } from "../lib/tabs";
import { useTimeline, useViewedFrame } from "../lib/timeline";
import { announce } from "./announcer";
import { StatusBar } from "./status-bar";
import { Viewport } from "./terminal-view";
import { Timeline } from "./timeline";
import { toast } from "./toaster";
import { Toolbar } from "./toolbar";

const reason = (error: unknown) => (error instanceof Error ? error.message : String(error));

type TabScreenProps = { session: string; tab: TabInfo; sessions: SessionInfo[]; onLayout: (count: Layout) => void };

export const TabScreen = ({ session, tab, sessions, onLayout }: TabScreenProps) => {
  const { state, shown, paint } = useTabStream(session, tab.tab);
  const timeline = useTimeline(session, tab.tab);
  const viewed = useViewedFrame(session, tab.tab);
  const past = viewed === null ? null : timeline.frame(viewed);
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
      <Toolbar layout={1} onLayout={onLayout} />
      <div className="h-px shrink-0 bg-border" role="none" />
      <Viewport
        paint={paint}
        state={state}
        shown={shown}
        program={tab.program}
        label={`${name} ${tab.program}, live frame ${shown?.frame ?? 0}`}
        past={past && { label: `${name} ${tab.program}, viewing frame ${past.frame}`, paint: timeline.paint }}
        onRestart={restart}
        onClose={close}
      />
      <Timeline session={session} tab={tab} />
      <div className="h-px shrink-0 bg-border" role="none" />
      <StatusBar session={session} tab={tab} state={state} shown={shown} past={past} />
    </>
  );
};
