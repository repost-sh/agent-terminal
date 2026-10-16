import type { Terminal } from "@xterm/xterm";
import { useState } from "react";

import { announce } from "./announcements";
import { tabStreamUrl } from "./api";
import type { FrameUpdate, StreamUpdate } from "./protocol";
import { followSocket, type Delivery } from "./socket";
import { createStore, useStore } from "./store";
import { tabKey, tabName, type TabAddress } from "./tabs";

export type StreamState = "connecting" | "open" | "reconnecting" | "closed";

export type Shown = Omit<FrameUpdate, "paint">;

interface Stream { state: StreamState; shown: Shown | null }

const awaitingFirstFrame = new Set<string>();

export const announceWhenLive = (session: string, tab: string) => awaitingFirstFrame.add(tabKey(session, tab));

interface Drawing { terminal: Terminal; shown: (meta: Shown) => void }

interface Frame { update: FrameUpdate; delivery: Delivery }

const draw = (drawing: Drawing, frame: Frame) => {
  const terminal = drawing.terminal;

  if (frame.delivery.first) {
    terminal.reset();
  }

  terminal.resize(frame.update.cols, frame.update.rows);

  const { paint: bytes, ...meta } = frame.update;

  terminal.write(bytes, () => {
    frame.delivery.reply({ type: "shown" });
    drawing.shown(meta);
  });
};

interface Publishing { address: TabAddress; publish: (next: Partial<Stream>) => void }

const painter = (publishing: Publishing) => (terminal: Terminal) => {
  const address = publishing.address;
  const publish = publishing.publish;
  let painting = true;
  const shown = (meta: Shown) => {
    if (painting) {
      publish({ shown: meta });
    }
  };

  const stop = followSocket<StreamUpdate>(tabStreamUrl(address.session, address.tab), {
    opened: () => {
      publish({ state: "open" });
    },
    message: (update, delivery) => {
      draw({ terminal, shown }, { update, delivery });
    },
    reconnecting: () => {
      publish({ state: "reconnecting" });
    },
    closed: () => {
      publish({ state: "closed" });
    },
  });

  return () => {
    painting = false;
    stop();
  };
};

const tabStream = (address: TabAddress) => {
  const stream = createStore<Stream>({ state: "connecting", shown: null });
  const publish = (next: Partial<Stream>) => {
    const now = { ...stream.get(), ...next };
    const drawn = now.shown !== null && (now.shown.frame > 0 || now.shown.exited);

    if (drawn && awaitingFirstFrame.delete(tabKey(address.session, address.tab))) {
      announce(`${tabName(address.session, address.tab)} is live`);
    }

    stream.set(now);
  };

  return { stream, paint: painter({ address, publish }) };
};

export const useTabStream = (session: string, tab: string) => {
  const [{ stream, paint }] = useState(() => tabStream({ session, tab }));

  return { ...useStore(stream), paint };
};
