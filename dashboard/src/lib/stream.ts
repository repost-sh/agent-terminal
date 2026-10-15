import type { Terminal } from "@xterm/xterm";
import { useState, useSyncExternalStore } from "react";

import { announce } from "../components/announcer";
import { tabStreamUrl } from "./api";
import type { FrameUpdate, StreamUpdate } from "./protocol";
import { followSocket } from "./socket";
import { tabKey, tabName } from "./tabs";

export type StreamState = "connecting" | "open" | "reconnecting" | "closed";

export type Shown = Omit<FrameUpdate, "paint">;

type Stream = { state: StreamState; shown: Shown | null };

const awaitingFirstFrame = new Set<string>();

export const announceWhenLive = (session: string, tab: string) => awaitingFirstFrame.add(tabKey(session, tab));

const tabStream = (session: string, tab: string) => {
  const listeners = new Set<() => void>();
  let stream: Stream = { state: "connecting", shown: null };

  const publish = (next: Partial<Stream>) => {
    stream = { ...stream, ...next };
    const drawn = stream.shown !== null && (stream.shown.frame > 0 || stream.shown.exited);
    if (drawn && awaitingFirstFrame.delete(tabKey(session, tab))) announce(`${tabName(session, tab)} is live`);
    listeners.forEach((listener) => listener());
  };

  const subscribe = (listener: () => void) => {
    listeners.add(listener);
    return () => listeners.delete(listener);
  };

  const paint = (terminal: Terminal) => {
    let painting = true;
    const stop = followSocket<StreamUpdate>(tabStreamUrl(session, tab), {
      opened: () => publish({ state: "open" }),
      message: (update, first, reply) => {
        if (first) terminal.reset();
        if (terminal.cols !== update.cols || terminal.rows !== update.rows) terminal.resize(update.cols, update.rows);
        const { paint: bytes, ...meta } = update;
        terminal.write(bytes, () => {
          reply({ type: "shown", frame: update.frame });
          if (!painting) return;
          publish({ shown: meta });
        });
      },
      reconnecting: () => publish({ state: "reconnecting" }),
      closed: () => publish({ state: "closed" }),
    });
    return () => {
      painting = false;
      stop();
    };
  };

  return { subscribe, read: () => stream, paint };
};

export const useTabStream = (session: string, tab: string) => {
  const [{ subscribe, read, paint }] = useState(() => tabStream(session, tab));
  return { ...useSyncExternalStore(subscribe, read), paint };
};
