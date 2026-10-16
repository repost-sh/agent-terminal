import { announce } from "./announcements";
import { sessionsStreamUrl } from "./api";
import type { SessionInfo, Sessions } from "./protocol";
import { leadHomeTo, tabRoute } from "./route";
import { createStore, useStore } from "./store";
import { firstTab } from "./tabs";

export type Connection = "connecting" | "connected" | "reconnecting" | "down";

interface Daemon { connection: Connection; sessions: SessionInfo[] | null }

const RECONNECT_AFTER_MS = [500, 1000, 2000];

let socket: WebSocket | null = null;
let timer: number | undefined;
let failures = 0;
let retried = false;

const daemon = createStore<Daemon>({ connection: "connecting", sessions: null }, () => {
  failures = 0;
  connect();

  return disconnect;
});

const publish = (next: Daemon) => {
  const first = next.connection === "connected" && next.sessions ? firstTab(next.sessions) : null;

  daemon.set(next);
  leadHomeTo(first ? tabRoute(first.session, first.tab) : null);
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
      publish({ ...daemon.get(), connection: "down" });

      return;
    }

    publish({ ...daemon.get(), connection: "reconnecting" });
    timer = window.setTimeout(connect, wait);
  };
};

const disconnect = () => {
  window.clearTimeout(timer);
  const open = socket;

  socket = null;
  open?.close();
};

const retry = () => {
  retried = true;
  window.clearTimeout(timer);
  failures = RECONNECT_AFTER_MS.length;
  connect();
};

export const useDaemon = () => ({ ...useStore(daemon), retry });
