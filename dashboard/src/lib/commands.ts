import { useState, useSyncExternalStore } from "react";

import { commandOutput, commandsStreamUrl } from "./api";
import type { CommandRecord, CommandsStreamUpdate, CommandsUpdate } from "./protocol";
import { followSocket } from "./socket";
import { tabKey } from "./tabs";

const commandsFeed = (session: string, tab: string) => {
  const listeners = new Set<() => void>();
  let latest: CommandsUpdate | null = null;
  let stop = () => {};

  const subscribe = (listener: () => void) => {
    listeners.add(listener);
    if (listeners.size === 1) {
      stop = followSocket<CommandsStreamUpdate>(commandsStreamUrl(session, tab), {
        message: (update) => {
          latest = update;
          listeners.forEach((each) => each());
        },
      });
    }
    return () => {
      listeners.delete(listener);
      if (listeners.size === 0) stop();
    };
  };

  return { subscribe, read: () => latest };
};

export const useCommands = (session: string, tab: string) => {
  const [{ subscribe, read }] = useState(() => commandsFeed(session, tab));
  return useSyncExternalStore(subscribe, read);
};

export type LoadedOutput = { output: string } | { error: string };

const outputs = new Map<string, Promise<LoadedOutput>>();

export const outputOf = (session: string, tab: string, command: CommandRecord) => {
  const key = JSON.stringify([session, tab, command.id, command.started_at_ms, command.ended_at_ms]);
  const known = outputs.get(key);
  if (known) return known;
  const loading = commandOutput(session, tab, command.id).then(
    (output): LoadedOutput => ({ output }),
    (error: unknown): LoadedOutput => ({ error: error instanceof Error ? error.message : String(error) }),
  );
  outputs.set(key, loading);
  return loading;
};

export const rowOf = (command: CommandRecord) => `${command.id}@${command.started_at_ms}`;

const openRows = new Map<string, string>();
const openListeners = new Set<() => void>();

const subscribeOpenRows = (listener: () => void) => {
  openListeners.add(listener);
  return () => openListeners.delete(listener);
};

export const useOpenCommand = (session: string, tab: string) => {
  const key = tabKey(session, tab);
  const open = useSyncExternalStore(subscribeOpenRows, () => openRows.get(key) ?? null);
  const toggle = (row: string) => {
    if (openRows.get(key) === row) openRows.delete(key);
    else openRows.set(key, row);
    openListeners.forEach((listener) => listener());
  };
  return { open, toggle };
};
