import { useState } from "react";

import { closeTab, reason } from "../lib/api";
import { controlKeys, useControl } from "../lib/control";
import type { SessionInfo, TabInfo } from "../lib/protocol";
import { navigate, tabRoute } from "../lib/route";
import { settle } from "../lib/settle";
import type { Layout } from "../lib/split";
import { useTabStream } from "../lib/stream";
import { firstTab, tabName } from "../lib/tabs";
import { announce } from "../lib/announcements";
import { restartExited } from "./restart-button";
import { controlHint, StatusBar } from "./status-bar";
import { heldTerminal, liveLabel, Viewport } from "./terminal-view";
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
  const stream = useTabStream(session, tab.tab);
  const name = tabName(session, tab.tab);
  const [keyboard] = useState(() => controlKeys(session, tab.tab));
  const held = useControl(session, tab.tab) === "held";
  const typing = held && !stream.shown?.exited;
  const exit = {
    onRestart: () => void restartExited(session, tab),
    onClose: () => void closeAndMoveOn(props),
  };

  return (
    <>
      <Toolbar layout={1} onLayout={props.onLayout} current={{ session, tab, stream: stream.state, shown: stream.shown }} />
      <div className="h-px shrink-0 bg-border" role="none" />
      <Viewport
        paint={stream.paint}
        state={stream.state}
        shown={stream.shown}
        program={tab.program}
        label={liveLabel(`${name} ${tab.program}`, stream.shown)}
        held={heldTerminal({ session, tab: tab.tab }, { held, typing, keyboard })}
        exit={exit}
      />
      <div className="h-px shrink-0 bg-border" role="none" />
      <StatusBar
        session={session}
        tab={tab}
        state={stream.state}
        shown={stream.shown}
        hint={controlHint(typing, tab.program)}
      />
    </>
  );
};
