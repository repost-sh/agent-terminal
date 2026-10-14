import type { Terminal } from "@xterm/xterm";
import { useSyncExternalStore } from "react";

import { toast } from "../components/toaster";
import { timelineUrl } from "./api";
import type { ActivityEntry, TimelineFrame, TimelineUpdate } from "./protocol";
import { tabName } from "./tabs";

const RECONNECT_AFTER_MS = (failures: number) => Math.min(500 * 2 ** failures, 5000);

export type TimelineState = { started: number | null; count: number };

const EMPTY: TimelineState = { started: null, count: 0 };

const keyOf = (session: string, tab: string) => JSON.stringify([session, tab]);

const viewed = new Map<string, number>();
const viewListeners = new Set<() => void>();

const subscribeViews = (listener: () => void) => {
  viewListeners.add(listener);
  return () => {
    viewListeners.delete(listener);
  };
};

const setViewed = (key: string, frame: number | null) => {
  if ((viewed.get(key) ?? null) === frame) return;
  if (frame === null) viewed.delete(key);
  else viewed.set(key, frame);
  viewListeners.forEach((listener) => listener());
};

type Timeline = {
  subscribe: (listener: () => void) => () => void;
  read: () => TimelineState;
  frame: (frame: number) => TimelineFrame | null;
  paint: (terminal: Terminal) => () => void;
};

const timelines = new Map<string, Timeline>();

const createTimeline = (session: string, tab: string): Timeline => {
  const key = keyOf(session, tab);
  const listeners = new Set<() => void>();
  let state = EMPTY;
  let frames: TimelineFrame[] = [];
  let socket: WebSocket | null = null;
  let timer: number | undefined;
  let failures = 0;

  const receive = (update: Exclude<TimelineUpdate, { type: "closed" }>) => {
    if (update.type === "failed") {
      toast("error", `Could not load the timeline of ${tabName(session, tab)}`, update.message);
      return;
    }
    if (update.reset) {
      frames = [];
      if (state.started !== null && state.started !== update.started_ms) setViewed(key, null);
    }
    frames.push(...update.frames);
    state = { started: update.started_ms, count: frames.length };
    listeners.forEach((listener) => listener());
  };

  const connect = () => {
    const opening = new WebSocket(timelineUrl(session, tab));
    socket = opening;
    opening.onopen = () => {
      failures = 0;
    };
    opening.onmessage = (event: MessageEvent<string>) => {
      if (socket !== opening) return;
      const update = JSON.parse(event.data) as TimelineUpdate;
      if (update.type === "closed") socket = null;
      else receive(update);
    };
    opening.onclose = () => {
      if (socket !== opening) return;
      timer = window.setTimeout(connect, RECONNECT_AFTER_MS(failures));
      failures += 1;
    };
  };

  const subscribe = (listener: () => void) => {
    listeners.add(listener);
    if (listeners.size === 1) connect();
    return () => {
      listeners.delete(listener);
      if (listeners.size) return;
      window.clearTimeout(timer);
      const open = socket;
      socket = null;
      open?.close();
      frames = [];
      state = EMPTY;
      setViewed(key, null);
    };
  };

  const paint = (terminal: Terminal) => {
    let painted: TimelineFrame | null = null;
    const repaint = () => {
      const shown = viewed.get(key);
      const target = shown === undefined ? null : (frames[shown - 1] ?? null);
      if (!target || target === painted) return;
      let from = target.frame - 1;
      while (from > 0 && frames[from]?.keyframe !== true) from -= 1;
      terminal.reset();
      if (terminal.cols !== target.cols || terminal.rows !== target.rows) terminal.resize(target.cols, target.rows);
      terminal.write(
        frames
          .slice(from, target.frame)
          .map((frame) => frame.paint)
          .join(""),
      );
      painted = target;
    };
    const stopViews = subscribeViews(repaint);
    const stopFrames = subscribe(repaint);
    repaint();
    return () => {
      stopViews();
      stopFrames();
    };
  };

  return { subscribe, read: () => state, frame: (frame) => frames[frame - 1] ?? null, paint };
};

const timelineOf = (session: string, tab: string) => {
  const key = keyOf(session, tab);
  const known = timelines.get(key);
  if (known) return known;
  const created = createTimeline(session, tab);
  timelines.set(key, created);
  return created;
};

export const useTimeline = (session: string, tab: string) => {
  const timeline = timelineOf(session, tab);
  const state = useSyncExternalStore(timeline.subscribe, timeline.read);
  return { ...state, frame: timeline.frame, paint: timeline.paint };
};

export const useViewedFrame = (session: string, tab: string) => {
  const key = keyOf(session, tab);
  return useSyncExternalStore(subscribeViews, () => viewed.get(key) ?? null);
};

export const useViewingPast = (session: string, tab: string) => useViewedFrame(session, tab) !== null;

export const viewingPast = (session: string, tab: string) => viewed.has(keyOf(session, tab));

export const viewFrame = (session: string, tab: string, frame: number) => {
  const { count } = timelines.get(keyOf(session, tab))?.read() ?? EMPTY;
  if (count === 0) return;
  setViewed(keyOf(session, tab), frame >= count ? null : Math.max(1, frame));
};

export const showMoment = (session: string, tab: string, frame: number, at: number) => {
  const { started } = timelines.get(keyOf(session, tab))?.read() ?? EMPTY;
  if (started === null || at < started) return;
  viewFrame(session, tab, frame);
};

export const goLive = (session: string, tab: string) => setViewed(keyOf(session, tab), null);

export const endOf = (entry: ActivityEntry) => Date.parse(entry.time) + entry.duration_ms;
