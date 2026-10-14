import type { Terminal } from "@xterm/xterm";
import { useEffect, useState } from "react";

import { tabStreamUrl } from "./api";
import type { FrameUpdate, StreamUpdate } from "./protocol";

export type StreamState = "connecting" | "open" | "reconnecting" | "closed";

export type Shown = Omit<FrameUpdate, "paint">;

const RECONNECT_AFTER_MS = (failures: number) => Math.min(500 * 2 ** failures, 5000);

export const useTabStream = (session: string, tab: string, terminal: Terminal | null) => {
  const [state, setState] = useState<StreamState>("connecting");
  const [shown, setShown] = useState<Shown | null>(null);

  useEffect(() => {
    if (!terminal) return;
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
        setState("open");
      };
      opening.onmessage = (event: MessageEvent<string>) => {
        const update = JSON.parse(event.data) as StreamUpdate;
        if (update.type === "closed") {
          finished = true;
          setState("closed");
          return;
        }
        if (first) terminal.reset();
        first = false;
        if (terminal.cols !== update.cols || terminal.rows !== update.rows) terminal.resize(update.cols, update.rows);
        const { paint, ...meta } = update;
        terminal.write(paint, () => {
          if (socket === opening) setShown(meta);
        });
      };
      opening.onclose = () => {
        if (finished || socket !== opening) return;
        setState("reconnecting");
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
  }, [session, tab, terminal]);

  return { state, shown };
};
