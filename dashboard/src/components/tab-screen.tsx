import { useState } from "react";

import { closeTab, restartTab } from "../lib/api";
import { controlKeys, controlTerminal, useControl } from "../lib/control";
import type { SessionInfo, TabInfo } from "../lib/protocol";
import { navigate, tabRoute } from "../lib/route";
import type { Layout } from "../lib/split";
import { useTabStream } from "../lib/stream";
import { firstTab, tabName } from "../lib/tabs";
import { announce } from "./announcer";
import { ControlButton } from "./control-button";
import { StatusBar } from "./status-bar";
import { Viewport } from "./terminal-view";
import { toast } from "./toaster";
import { Toolbar } from "./toolbar";

const reason = (error: unknown) => (error instanceof Error ? error.message : String(error));

type TabScreenProps = { session: string; tab: TabInfo; sessions: SessionInfo[]; onLayout: (count: Layout) => void };

export const TabScreen = ({ session, tab, sessions, onLayout }: TabScreenProps) => {
  const { state, shown, paint } = useTabStream(session, tab.tab);
  const name = tabName(session, tab.tab);
  const [keyboard] = useState(() => controlKeys(session, tab.tab));
  const held = useControl(session, tab.tab) === "held";
  const typing = held && !shown?.exited;

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
      <Toolbar layout={1} onLayout={onLayout}>
        <ControlButton session={session} tab={tab} stream={state} shown={shown} />
      </Toolbar>
      <div className="h-px shrink-0 bg-border" role="none" />
      <Viewport
        paint={paint}
        state={state}
        shown={shown}
        program={tab.program}
        label={`${name} ${tab.program}, live frame ${shown?.frame ?? 0}`}
        held={
          held
            ? { name, focus: controlTerminal(session, tab.tab), keys: typing ? keyboard.keys : undefined, paste: typing ? keyboard.paste : undefined }
            : null
        }
        onRestart={restart}
        onClose={close}
      />
      <div className="h-px shrink-0 bg-border" role="none" />
      <StatusBar
        session={session}
        tab={tab}
        state={state}
        shown={shown}
        hint={typing ? `Keys go to ${tab.program} · ctrl+p ctrl+q moves focus out` : null}
      />
    </>
  );
};
