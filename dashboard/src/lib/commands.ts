import { useState, useSyncExternalStore } from "react";

import { commandOutput, commandsStreamUrl, reason } from "./api";
import type { CommandRecord, CommandsStreamUpdate, CommandsUpdate } from "./protocol";
import { followSocket } from "./socket";
import { createSignal, createStore, useStore } from "./store";
import { tabKey, type TabAddress } from "./tabs";

const commandsFeed = (session: string, tab: string) =>
  createStore<CommandsUpdate | null>(null, (feed) =>
    followSocket<CommandsStreamUpdate>(commandsStreamUrl(session, tab), {
      message: (update) => {
        feed.set(update);
      },
    }),
  );

export const useCommands = (session: string, tab: string) => {
  const [feed] = useState(() => commandsFeed(session, tab));

  return useStore(feed);
};

export type LoadedOutput = { output: string } | { error: string };

const outputs = new Map<string, Promise<LoadedOutput>>();

export const outputOf = (address: TabAddress, command: CommandRecord) => {
  const key = JSON.stringify([address.session, address.tab, command.id, command.started_at_ms, command.ended_at_ms]);
  const known = outputs.get(key);

  if (known) {
    return known;
  }

  const loading = commandOutput(address, command.id).then(
    (output): LoadedOutput => ({ output }),
    (error: unknown): LoadedOutput => ({ error: reason(error) }),
  );

  outputs.set(key, loading);

  return loading;
};

export const rowOf = (command: CommandRecord) => `${String(command.id)}@${String(command.started_at_ms)}`;

const openRows = new Map<string, string>();
const openRowsChanged = createSignal();

export const useOpenCommand = (session: string, tab: string) => {
  const key = tabKey(session, tab);
  const open = useSyncExternalStore(openRowsChanged.subscribe, () => openRows.get(key) ?? null);
  const toggle = (row: string) => {
    if (openRows.get(key) === row) {
      openRows.delete(key);
    } else {
      openRows.set(key, row);
    }

    openRowsChanged.notify();
  };

  return { open, toggle };
};
