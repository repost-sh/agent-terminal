import { useSyncExternalStore } from "react";

import { announce } from "../components/announcer";
import { sessionsStreamUrl } from "./api";
import type { SessionInfo, Sessions } from "./protocol";
import { leadHomeTo, tabRoute } from "./route";
import { firstTab } from "./tabs";

export type Connection = "connecting" | "connected" | "reconnecting" | "down";

interface Daemon { connection: Connection; sessions: SessionInfo[] | null }

const RECONNECT_AFTER_MS = [500, 1000, 2000];

const listeners = new Set<() => void>();
let daemon: Daemon = { connection: "connecting", sessions: null };
let socket: WebSocket | null = null;
let timer: number | undefined;
let failures = 0;
let retried = false;

const publish = (next: Daemon) => {
  daemon = next;
  const first = next.connection === "connected" && next.sessions ? firstTab(next.sessions) : null;

  leadHomeTo(first ? tabRoute(first.session, first.tab) : null);
  listeners.forEach((listener) => {
    listener();
  });
};

const connect = () => {
  const opening = new WebSocket(sessionsStreamUrl());

  socket = opening;
  opening.onmessage = (event: MessageEvent<string>) => {
    failures = 0;
    publish({ connection: "connected", sessions: (JSON.parse(event.data) as Sessions).sessions });
    if (retried) {
      announce("Connected");
    }

    retried = false;
  };

  opening.onclose = () => {
    if (socket !== opening) {
      return;
    }

    const wait = RECONNECT_AFTER_MS[failures];

    failures += 1;
    if (wait === undefined) {
      publish({ ...daemon, connection: "down" });

      return;
    }

    publish({ ...daemon, connection: "reconnecting" });
    timer = window.setTimeout(connect, wait);
  };
};

const disconnect = () => {
  window.clearTimeout(timer);
  const open = socket;

  socket = null;
  open?.close();
};

const subscribe = (listener: () => void) => {
  listeners.add(listener);
  if (listeners.size === 1) {
    failures = 0;
    connect();
  }

  return () => {
    listeners.delete(listener);
    if (listeners.size === 0) {
      disconnect();
    }
  };
};

const retry = () => {
  retried = true;
  window.clearTimeout(timer);
  failures = RECONNECT_AFTER_MS.length;
  connect();
};

export const useDaemon = () => ({ ...useSyncExternalStore(subscribe, () => daemon), retry });
