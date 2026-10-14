import type { ClipboardEvent, KeyboardEvent } from "react";
import { useSyncExternalStore } from "react";

import { announce } from "../components/announcer";
import { toast } from "../components/toaster";
import { controlUrl } from "./api";
import { focusNext } from "./focus";
import type { ControlUpdate, HumanInput } from "./protocol";
import { tabName } from "./tabs";

export type ControlState = "off" | "taking" | "held";

type Held = { socket: WebSocket; state: ControlState };

const holds = new Map<string, Held>();
const listeners = new Set<() => void>();

const keyOf = (session: string, tab: string) => `${session}/${tab}`;

const publish = () => listeners.forEach((listener) => listener());

const subscribe = (listener: () => void) => {
  listeners.add(listener);
  return () => listeners.delete(listener);
};

export const controlButtonId = (session: string, tab: string) => `take-control-${encodeURIComponent(session)}-${tab}`;

export const controlTerminal = (session: string, tab: string) => `control-terminal:${keyOf(session, tab)}`;

export const useControl = (session: string, tab: string): ControlState =>
  useSyncExternalStore(subscribe, () => holds.get(keyOf(session, tab))?.state ?? "off");

const handedBack = (session: string, tab: string, socket: WebSocket) => {
  const key = keyOf(session, tab);
  if (holds.get(key)?.socket !== socket) return false;
  holds.delete(key);
  publish();
  return true;
};

export const takeControl = (session: string, tab: string, program: string) => {
  const key = keyOf(session, tab);
  if (holds.has(key)) return;
  const name = tabName(session, tab);
  const socket = new WebSocket(controlUrl(session, tab));
  holds.set(key, { socket, state: "taking" });
  publish();
  socket.onmessage = (event: MessageEvent<string>) => {
    const update = JSON.parse(event.data) as ControlUpdate;
    if (update.type === "held") {
      holds.set(key, { socket, state: "held" });
      focusNext(controlTerminal(session, tab));
      publish();
      announce(`You have control of ${name}`);
    } else if (update.type === "agent-refused") {
      toast("warning", "The agent is waiting for control", `${update.command} refused · exit 4 · now: wait --control`);
    } else if (update.type === "refused") {
      const taking = holds.get(key)?.state === "taking";
      if (taking) handedBack(session, tab, socket);
      toast("error", taking ? `Could not take control of ${name}` : `Could not send the key to ${program}`, update.message);
    } else if (handedBack(session, tab, socket)) {
      announce("Control returned to the agent");
    }
  };
  socket.onclose = () => {
    if (handedBack(session, tab, socket)) announce("Control returned to the agent");
  };
};

export const releaseControl = (session: string, tab: string) => {
  const held = holds.get(keyOf(session, tab));
  if (!held) return;
  handedBack(session, tab, held.socket);
  held.socket.close();
  announce("Control returned to the agent");
};

const send = (session: string, tab: string, input: HumanInput) => {
  const held = holds.get(keyOf(session, tab));
  if (held?.state === "held" && held.socket.readyState === WebSocket.OPEN) held.socket.send(JSON.stringify(input));
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

const baseKey = (event: KeyboardEvent) => {
  const letter = /^Key([A-Z])$/.exec(event.code)?.[1];
  if (letter) return letter.toLowerCase();
  const digit = /^Digit(\d)$/.exec(event.code)?.[1];
  if (digit) return digit;
  if (event.key === " ") return "Space";
  return [...event.key].length === 1 ? event.key : null;
};

const humanInput = (event: KeyboardEvent): HumanInput | null => {
  const named = NAMED[event.key] ?? (FUNCTION_KEY.test(event.key) ? event.key : null);
  const held = [event.ctrlKey && "Ctrl", event.altKey && "Alt"].filter((part): part is string => Boolean(part));
  if (named) return { pressed: [...held, ...(event.shiftKey ? ["Shift"] : []), named].join("+") };
  if (held.length === 0) return [...event.key].length === 1 ? { typed: event.key } : null;
  const key = baseKey(event);
  return key ? { pressed: [...held, key].join("+") } : null;
};

const ctrlAnd = (event: KeyboardEvent, letter: string) => event.ctrlKey && !event.altKey && event.key.toLowerCase() === letter;

export const controlKeys = (session: string, tab: string) => {
  let chord = false;
  const keys = (event: KeyboardEvent<HTMLElement>) => {
    if (event.metaKey || MODIFIER_KEYS.has(event.key)) return;
    event.preventDefault();
    event.stopPropagation();
    if (chord && ctrlAnd(event, "q")) {
      chord = false;
      document.getElementById(controlButtonId(session, tab))?.focus();
      return;
    }
    if (chord) send(session, tab, { pressed: "Ctrl+p" });
    chord = ctrlAnd(event, "p");
    if (chord) return;
    const input = humanInput(event);
    if (input) send(session, tab, input);
  };
  const paste = (event: ClipboardEvent<HTMLElement>) => {
    event.preventDefault();
    const text = event.clipboardData.getData("text");
    if (text) send(session, tab, { pasted: text });
  };
  return { keys, paste };
};
