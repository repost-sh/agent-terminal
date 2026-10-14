import type { Terminal } from "@xterm/xterm";
import { useState, useSyncExternalStore } from "react";

import { tabStreamUrl } from "./api";
import type { FrameUpdate, StreamUpdate } from "./protocol";

export type StreamState = "connecting" | "open" | "reconnecting" | "closed";

export type Shown = Omit<FrameUpdate, "paint">;

type Stream = { state: StreamState; shown: Shown | null };

const RECONNECT_AFTER_MS = (failures: number) => Math.min(500 * 2 ** failures, 5000);

const tabStream = (session: string, tab: string) => {
  const listeners = new Set<() => void>();
  let stream: Stream = { state: "connecting", shown: null };

  const publish = (next: Partial<Stream>) => {
    stream = { ...stream, ...next };
    listeners.forEach((listener) => listener());
  };

  const subscribe = (listener: () => void) => {
    listeners.add(listener);
    return () => listeners.delete(listener);
  };

  const paint = (terminal: Terminal) => {
    let socket: WebSocket | null = null;
    let timer: number | undefined;
    let failures = 0;
    let finished = false;

    const connect = () => {
      const opening = new WebSocket(tabStreamUrl(session, tab));
      socket = opening;
      let first = true;
      opening.onopen = () => {
        failures = 0;
        publish({ state: "open" });
      };
      opening.onmessage = (event: MessageEvent<string>) => {
        const update = JSON.parse(event.data) as StreamUpdate;
        if (update.type === "closed") {
          finished = true;
          publish({ state: "closed" });
          return;
        }
        if (first) terminal.reset();
        first = false;
        if (terminal.cols !== update.cols || terminal.rows !== update.rows) terminal.resize(update.cols, update.rows);
        const { paint, ...meta } = update;
        terminal.write(paint, () => {
          if (socket === opening) publish({ shown: meta });
        });
      };
      opening.onclose = () => {
        if (finished || socket !== opening) return;
        publish({ state: "reconnecting" });
        timer = window.setTimeout(connect, RECONNECT_AFTER_MS(failures));
        failures += 1;
      };
    };

    connect();
    return () => {
      finished = true;
      window.clearTimeout(timer);
      const open = socket;
      socket = null;
      open?.close();
    };
  };

  return { subscribe, read: () => stream, paint };
};

export const useTabStream = (session: string, tab: string) => {
  const [{ subscribe, read, paint }] = useState(() => tabStream(session, tab));
  return { ...useSyncExternalStore(subscribe, read), paint };
};
