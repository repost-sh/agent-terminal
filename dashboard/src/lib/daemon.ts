import { useEffect, useRef, useState } from "react";

import { sessionsStreamUrl } from "./api";
import type { SessionInfo, Sessions } from "./protocol";

export type Connection = "connecting" | "connected" | "reconnecting" | "down";

const RECONNECT_AFTER_MS = [500, 1000, 2000];

export const useDaemon = () => {
  const [connection, setConnection] = useState<Connection>("connecting");
  const [sessions, setSessions] = useState<SessionInfo[] | null>(null);
  const retry = useRef(() => {});

  useEffect(() => {
    let socket: WebSocket | null = null;
    let timer: number | undefined;
    let failures = 0;
    let disposed = false;

    const connect = () => {
      const opening = new WebSocket(sessionsStreamUrl());
      socket = opening;
      opening.onmessage = (event: MessageEvent<string>) => {
        failures = 0;
        setConnection("connected");
        setSessions((JSON.parse(event.data) as Sessions).sessions);
      };
      opening.onclose = () => {
        if (disposed || socket !== opening) return;
        const wait = RECONNECT_AFTER_MS[failures];
        failures += 1;
        if (wait === undefined) {
          setConnection("down");
          return;
        }
        setConnection("reconnecting");
        timer = window.setTimeout(connect, wait);
      };
    };

    retry.current = () => {
      window.clearTimeout(timer);
      failures = RECONNECT_AFTER_MS.length;
      connect();
    };
    connect();
    return () => {
      disposed = true;
      window.clearTimeout(timer);
      socket?.close();
    };
  }, []);

  return { connection, sessions, retry: () => retry.current() };
};
