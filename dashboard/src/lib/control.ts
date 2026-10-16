import type { ClipboardEvent, KeyboardEvent } from "react";
import { useSyncExternalStore } from "react";

import { announce } from "./announcements";
import { toast } from "./toasts";
import { controlUrl } from "./api";
import { focusNext } from "./focus";
import type { ControlUpdate, HumanInput } from "./protocol";
import { tabKey, tabName, type TabAddress } from "./tabs";

export type ControlState = "off" | "taking" | "held";

interface Held { socket: WebSocket; state: ControlState }

const holds = new Map<string, Held>();
const listeners = new Set<() => void>();

const publish = () => {
  listeners.forEach((listener) => {
    listener();
  });
};

const subscribe = (listener: () => void) => {
  listeners.add(listener);

  return () => listeners.delete(listener);
};

export const controlButtonId = (session: string, tab: string) => `take-control-${encodeURIComponent(session)}-${tab}`;

export const controlTerminal = (session: string, tab: string) => `control-terminal:${tabKey(session, tab)}`;

export const useControl = (session: string, tab: string): ControlState =>
  useSyncExternalStore(subscribe, () => holds.get(tabKey(session, tab))?.state ?? "off");

const handedBack = (address: TabAddress, socket: WebSocket) => {
  const key = tabKey(address.session, address.tab);

  if (holds.get(key)?.socket !== socket) {
    return false;
  }

  holds.delete(key);
  publish();

  return true;
};

interface Taking { address: TabAddress; socket: WebSocket; program: string }

const gotControl = (taking: Taking) => {
  const address = taking.address;

  holds.set(tabKey(address.session, address.tab), { socket: taking.socket, state: "held" });
  focusNext(controlTerminal(address.session, address.tab));
  publish();
  announce(`You have control of ${tabName(address.session, address.tab)}`);
};

const refusedHuman = (taking: Taking, message: string) => {
  const address = taking.address;
  const name = tabName(address.session, address.tab);
  const stillTaking = holds.get(tabKey(address.session, address.tab))?.state === "taking";

  if (stillTaking) {
    handedBack(address, taking.socket);
  }

  toast("error", { title: stillTaking ? `Could not take control of ${name}` : `Could not send the key to ${taking.program}`, description: message });
};

const controlUpdate = (taking: Taking, update: ControlUpdate) => {
  if (update.type === "held") {
    gotControl(taking);

    return;
  }

  if (update.type === "agent-refused") {
    toast("warning", { title: "The agent is waiting for control", description: `${update.command} refused · exit 4 · now: wait --control` });

    return;
  }

  if (update.type === "refused") {
    refusedHuman(taking, update.message);

    return;
  }

  if (handedBack(taking.address, taking.socket)) {
    announce("Control returned to the agent");
  }
};

export const takeControl = (address: TabAddress, program: string) => {
  const key = tabKey(address.session, address.tab);

  if (holds.has(key)) {
    return;
  }

  const socket = new WebSocket(controlUrl(address.session, address.tab));
  const taking = { address, socket, program };

  holds.set(key, { socket, state: "taking" });
  publish();
  socket.onmessage = (event: MessageEvent<string>) => {
    controlUpdate(taking, JSON.parse(event.data) as ControlUpdate);
  };

  socket.onclose = () => {
    if (handedBack(address, socket)) {
      announce("Control returned to the agent");
    }
  };
};

export const releaseControl = (session: string, tab: string) => {
  const held = holds.get(tabKey(session, tab));

  if (!held) {
    return;
  }

  handedBack({ session, tab }, held.socket);
  held.socket.close();
  announce("Control returned to the agent");
};

const send = (address: TabAddress, input: HumanInput) => {
  const held = holds.get(tabKey(address.session, address.tab));

  if (held?.state === "held" && held.socket.readyState === WebSocket.OPEN) {
    held.socket.send(JSON.stringify(input));
  }
};

const NAMED: Record<string, string> = {
  Enter: "Enter",
  Tab: "Tab",
  Escape: "Esc",
  Backspace: "Backspace",
  Delete: "Delete",
  Insert: "Insert",
  ArrowUp: "Up",
  ArrowDown: "Down",
  ArrowLeft: "Left",
  ArrowRight: "Right",
  Home: "Home",
  End: "End",
  PageUp: "PageUp",
  PageDown: "PageDown",
};

const MODIFIER_KEYS = new Set(["Shift", "Control", "Alt", "AltGraph", "Meta", "CapsLock", "NumLock", "ScrollLock", "Fn", "OS"]);

const FUNCTION_KEY = /^F([1-9]|1[0-2])$/;

const CODE_KEYS = [/^Key([A-Z])$/, /^Digit(\d)$/];

const isOneCodePoint = (text: string) => Array.from(text).length === 1;

const fromCode = (code: string) => {
  for (const pattern of CODE_KEYS) {
    const found = pattern.exec(code)?.[1];

    if (found) {
      return found.toLowerCase();
    }
  }

  return undefined;
};

const baseKey = (event: KeyboardEvent) => {
  const coded = fromCode(event.code);

  if (coded) {
    return coded;
  }

  if (event.key === " ") {
    return "Space";
  }

  return isOneCodePoint(event.key) ? event.key : null;
};

const namedKey = (key: string) => NAMED[key] ?? (FUNCTION_KEY.test(key) ? key : null);

const heldModifiers = (event: KeyboardEvent) => [event.ctrlKey && "Ctrl", event.altKey && "Alt"].filter((part): part is string => Boolean(part));

const humanInput = (event: KeyboardEvent): HumanInput | null => {
  const named = namedKey(event.key);
  const held = heldModifiers(event);

  if (named) {
    return { pressed: [...held, ...(event.shiftKey ? ["Shift"] : []), named].join("+") };
  }

  if (held.length === 0) {
    return isOneCodePoint(event.key) ? { typed: event.key } : null;
  }

  const key = baseKey(event);

  return key ? { pressed: [...held, key].join("+") } : null;
};

const ctrlAnd = (event: KeyboardEvent, letter: string) => event.ctrlKey && !event.altKey && event.key.toLowerCase() === letter;

interface Keystroke { chord: boolean; event: KeyboardEvent<HTMLElement> }

const leftChord = (address: TabAddress, keystroke: Keystroke) => {
  if (!keystroke.chord) {
    return false;
  }

  if (ctrlAnd(keystroke.event, "q")) {
    document.getElementById(controlButtonId(address.session, address.tab))?.focus();

    return true;
  }

  send(address, { pressed: "Ctrl+p" });

  return false;
};

const pressedIn = (address: TabAddress, keystroke: Keystroke) => {
  if (leftChord(address, keystroke)) {
    return false;
  }

  if (ctrlAnd(keystroke.event, "p")) {
    return true;
  }

  const input = humanInput(keystroke.event);

  if (input) {
    send(address, input);
  }

  return false;
};

export const controlKeys = (session: string, tab: string) => {
  const address = { session, tab };
  let chord = false;
  const keys = (event: KeyboardEvent<HTMLElement>) => {
    if (event.metaKey || MODIFIER_KEYS.has(event.key)) {
      return;
    }

    event.preventDefault();
    event.stopPropagation();
    chord = pressedIn(address, { chord, event });
  };

  const paste = (event: ClipboardEvent<HTMLElement>) => {
    event.preventDefault();
    const text = event.clipboardData.getData("text");

    if (text) {
      send(address, { pasted: text });
    }
  };

  return { keys, paste };
};
