import type { Terminal } from "@xterm/xterm";
import { useSyncExternalStore } from "react";

import { timelineUrl } from "./api";
import type { ActivityEntry, TimelineFrame, TimelineUpdate } from "./protocol";
import { followSocket } from "./socket";
import { tabKey } from "./tabs";

export type TimelineState = { started: number | null; count: number; failed: boolean };

const EMPTY: TimelineState = { started: null, count: 0, failed: false };

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
  paint: (terminal: Terminal, drawn: () => void) => () => void;
};

const timelines = new Map<string, Timeline>();

const createTimeline = (session: string, tab: string): Timeline => {
  const key = tabKey(session, tab);
  const listeners = new Set<() => void>();
  let state = EMPTY;
  let frames: TimelineFrame[] = [];
  let stop = () => {};

  const publish = (next: TimelineState) => {
    state = next;
    listeners.forEach((listener) => listener());
  };

  const receive = (update: Exclude<TimelineUpdate, { type: "closed" }>) => {
    if (update.type === "failed") {
      frames = [];
      setViewed(key, null);
      publish({ started: null, count: 0, failed: true });
      return;
    }
    if (update.reset) {
      frames = [];
      if (state.started !== null && state.started !== update.started_ms) setViewed(key, null);
    }
    frames.push(...update.frames);
    publish({ started: update.started_ms, count: frames.length, failed: false });
  };

  const subscribe = (listener: () => void) => {
    listeners.add(listener);
    if (listeners.size === 1) stop = followSocket<TimelineUpdate>(timelineUrl(session, tab), { message: receive });
    return () => {
      listeners.delete(listener);
      if (listeners.size) return;
      stop();
      frames = [];
      state = EMPTY;
      setViewed(key, null);
    };
  };

  const paint = (terminal: Terminal, drawn: () => void) => {
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
        drawn,
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
  const key = tabKey(session, tab);
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
  const key = tabKey(session, tab);
  return useSyncExternalStore(subscribeViews, () => viewed.get(key) ?? null);
};


export const viewFrame = (session: string, tab: string, frame: number) => {
  const { count } = timelines.get(tabKey(session, tab))?.read() ?? EMPTY;
  if (count === 0) return;
  setViewed(tabKey(session, tab), frame >= count ? null : Math.max(1, frame));
};

export const showMoment = (session: string, tab: string, frame: number, at: number) => {
  const { started } = timelines.get(tabKey(session, tab))?.read() ?? EMPTY;
  if (started === null || at < started) return;
  viewFrame(session, tab, frame);
};

export const goLive = (session: string, tab: string) => setViewed(tabKey(session, tab), null);

export const endOf = (entry: ActivityEntry) => Date.parse(entry.time) + entry.duration_ms;
