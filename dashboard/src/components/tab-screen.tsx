import { useState } from "react";

import { closeTab, reason } from "../lib/api";
import { controlKeys, controlTerminal, useControl } from "../lib/control";
import type { SessionInfo, TabInfo } from "../lib/protocol";
import { navigate, tabRoute } from "../lib/route";
import type { Layout } from "../lib/split";
import { useTabStream } from "../lib/stream";
import { firstTab, tabName } from "../lib/tabs";
import { useTimeline, useViewedFrame } from "../lib/timeline";
import { announce } from "./announcer";
import { restartExited } from "./restart-button";
import { StatusBar } from "./status-bar";
import { Viewport } from "./terminal-view";
import { Timeline } from "./timeline";
import { toast } from "./toaster";
import { Toolbar } from "./toolbar";

type TabScreenProps = { session: string; tab: TabInfo; sessions: SessionInfo[]; onLayout: (count: Layout) => void };

export const TabScreen = ({ session, tab, sessions, onLayout }: TabScreenProps) => {
  const { state, shown, paint } = useTabStream(session, tab.tab);
  const timeline = useTimeline(session, tab.tab);
  const viewed = useViewedFrame(session, tab.tab);
  const past = viewed === null ? null : timeline.frame(viewed);
  const name = tabName(session, tab.tab);
  const [keyboard] = useState(() => controlKeys(session, tab.tab));
  const held = useControl(session, tab.tab) === "held" && viewed === null;
  const typing = held && !shown?.exited;

  const restart = () => void restartExited(session, tab);

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
      <Toolbar layout={1} onLayout={onLayout} current={{ session, tab, stream: state, shown }} />
      <div className="h-px shrink-0 bg-border" role="none" />
      <Viewport
        paint={paint}
        state={state}
        shown={shown}
        program={tab.program}
        label={`${name} ${tab.program}, live frame ${shown?.frame ?? 0}`}
        past={past && { label: `${name} ${tab.program}, viewing frame ${past.frame}`, paint: timeline.paint }}
        held={
          held
            ? { name, focus: controlTerminal(session, tab.tab), keys: typing ? keyboard.keys : undefined, paste: typing ? keyboard.paste : undefined }
            : null
        }
        exit={{ onRestart: restart, onClose: close }}
      />
      <Timeline session={session} tab={tab} />
      <div className="h-px shrink-0 bg-border" role="none" />
      <StatusBar
        session={session}
        tab={tab}
        state={state}
        shown={shown}
        past={past}
        hint={typing ? `Keys go to ${tab.program} · ctrl+p ctrl+q moves focus out` : null}
      />
    </>
  );
};
