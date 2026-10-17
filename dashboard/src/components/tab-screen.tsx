
import { closeTab, reason } from "../lib/api";
import type { SessionInfo, TabInfo } from "../lib/protocol";
import { navigate, tabRoute } from "../lib/route";
import { settle } from "../lib/settle";
import type { Layout } from "../lib/split";
import { firstTab, tabName } from "../lib/tabs";
import { announce } from "../lib/announcements";
import { restartExited } from "./restart-button";
import { StatusBar } from "./status-bar";
import { useHeldViewport, Viewport } from "./terminal-view";
import { toast } from "../lib/toasts";
import { Toolbar } from "./toolbar";

interface TabScreenProps { session: string; tab: TabInfo; sessions: SessionInfo[]; onLayout: (count: Layout) => void }

const closeAndMoveOn = async (props: TabScreenProps) => {
  const name = tabName(props.session, props.tab.tab);
  const closed = await settle(async () => {
    await closeTab(props.session, props.tab.tab);
    const left = props.sessions.map((entry) => ({
      ...entry,
      tabs: entry.tabs.filter((other) => entry.session !== props.session || other.tab !== props.tab.tab),
    }));
    const next = firstTab(left);

    navigate(next ? tabRoute(next.session, next.tab) : "/");
    announce(`Closed ${name}`);
  });

  if (!closed.ok) {
    toast("error", { title: `Could not close ${name}`, description: reason(closed.error) });
  }
};

export const TabScreen = (props: TabScreenProps) => {
  const { session, tab } = props;
  const view = useHeldViewport({ session, tab, focused: true });
  const { stream } = view;
  const exit = {
    onRestart: () => void restartExited(session, tab),
    onClose: () => void closeAndMoveOn(props),
  };

  return (
    <>
      <Toolbar layout={1} onLayout={props.onLayout} current={{ session, tab, stream: stream.state, shown: stream.shown }} />
      <div className="h-px shrink-0 bg-border" role="none" />
      <Viewport {...view.viewport} exit={exit} />
      <div className="h-px shrink-0 bg-border" role="none" />
      <StatusBar
        session={session}
        tab={tab}
        state={stream.state}
        shown={stream.shown}
        hint={view.hint}
      />
    </>
  );
};
