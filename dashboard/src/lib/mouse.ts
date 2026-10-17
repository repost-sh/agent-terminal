import { sendInput } from "./control";
import type { Mods, MouseButton, MouseEvent as MouseInput } from "./protocol";
import type { Shown } from "./stream";
import type { TabAddress } from "./tabs";

const TRACKINGS = ["click", "drag", "motion"] as const;

export type Tracking = (typeof TRACKINGS)[number];

export interface Tracked { tracking: Tracking; cols: number; rows: number }

const isTracking = (mode: string | null): mode is Tracking => TRACKINGS.some((known) => known === mode);

export const trackedIn = (shown: Shown | null): Tracked | null =>
  shown && isTracking(shown.mouse_tracking) ? { tracking: shown.mouse_tracking, cols: shown.cols, rows: shown.rows } : null;

interface Cell { row: number; col: number }

interface Gesture { held: MouseButton | null; hovered: Cell | null; wheelRest: number }

interface Forwarding { address: TabAddress; node: HTMLElement; gesture: Gesture; tracked: () => Tracked | null }

interface Located { cell: Cell; lineHeight: number; rows: number }

const BUTTONS: readonly MouseButton[] = ["left", "middle", "right"];

const DELTA_LINE = 1;

const DELTA_PAGE = 2;

const modsOf = (event: MouseEvent): Mods => ({ shift: event.shiftKey, alt: event.altKey, ctrl: event.ctrlKey });

const screenBox = (node: HTMLElement) => {
  const box = node.querySelector(".xterm-screen")?.getBoundingClientRect();

  return box !== undefined && box.width > 0 && box.height > 0 ? box : null;
};

const clamp = (value: number, last: number) => Math.min(Math.max(value, 1), last);

const locate = (forwarding: Forwarding, event: MouseEvent): Located | null => {
  const tracked = forwarding.tracked();
  const box = tracked ? screenBox(forwarding.node) : null;

  if (!tracked || !box) {
    return null;
  }

  const col = clamp(Math.floor(((event.clientX - box.left) / box.width) * tracked.cols) + 1, tracked.cols);
  const row = clamp(Math.floor(((event.clientY - box.top) / box.height) * tracked.rows) + 1, tracked.rows);

  return { cell: { row, col }, lineHeight: box.height / tracked.rows, rows: tracked.rows };
};

const send = (forwarding: Forwarding, mouse: MouseInput) => {
  sendInput(forwarding.address, { mouse });
};

const pressed = (forwarding: Forwarding, event: MouseEvent) => {
  const located = event.shiftKey ? null : locate(forwarding, event);
  const button = BUTTONS[event.button];

  if (!located || button === undefined) {
    return;
  }

  event.stopPropagation();
  forwarding.gesture.held = button;
  forwarding.gesture.hovered = located.cell;
  send(forwarding, { event: "down", ...located.cell, button, mods: modsOf(event) });
};

const released = (forwarding: Forwarding, event: MouseEvent) => {
  const button = forwarding.gesture.held;
  const located = button === null ? null : locate(forwarding, event);

  forwarding.gesture.held = null;
  if (button === null || !located) {
    return;
  }

  event.stopPropagation();
  send(forwarding, { event: "up", ...located.cell, button, mods: modsOf(event) });
};

const wantsMoves = (forwarding: Forwarding) => {
  const tracking = forwarding.tracked()?.tracking;

  return tracking === "motion" || (tracking === "drag" && forwarding.gesture.held !== null);
};

const sameCell = (hovered: Cell | null, cell: Cell) => hovered !== null && hovered.row === cell.row && hovered.col === cell.col;

const moved = (forwarding: Forwarding, event: MouseEvent) => {
  const located = !event.shiftKey && wantsMoves(forwarding) ? locate(forwarding, event) : null;

  if (!located || sameCell(forwarding.gesture.hovered, located.cell)) {
    return;
  }

  forwarding.gesture.hovered = located.cell;
  send(forwarding, { event: "move", ...located.cell, mods: modsOf(event) });
};

const deltaLines = (event: WheelEvent, located: Located) => {
  switch (event.deltaMode) {
    case DELTA_LINE:
      return event.deltaY;
    case DELTA_PAGE:
      return event.deltaY * located.rows;
    default:
      return event.deltaY / located.lineHeight;
  }
};

const wheeled = (forwarding: Forwarding, event: WheelEvent) => {
  const located = event.shiftKey ? null : locate(forwarding, event);

  if (!located) {
    return;
  }

  event.preventDefault();
  const { gesture } = forwarding;

  gesture.wheelRest += deltaLines(event, located);
  const notches = Math.trunc(gesture.wheelRest);

  gesture.wheelRest -= notches;
  for (let turned = 0; turned < Math.abs(notches); turned += 1) {
    send(forwarding, { event: "wheel", ...located.cell, direction: notches > 0 ? "down" : "up", mods: modsOf(event) });
  }
};

const menu = (forwarding: Forwarding, event: MouseEvent) => {
  if (forwarding.tracked() && !event.shiftKey) {
    event.preventDefault();
  }
};

const listen = (forwarding: Forwarding) => {
  const { node } = forwarding;
  const down = (event: MouseEvent) => {
    pressed(forwarding, event);
  };

  const up = (event: MouseEvent) => {
    released(forwarding, event);
  };

  const move = (event: MouseEvent) => {
    moved(forwarding, event);
  };

  const wheel = (event: WheelEvent) => {
    wheeled(forwarding, event);
  };

  const context = (event: MouseEvent) => {
    menu(forwarding, event);
  };

  node.addEventListener("mousedown", down, { capture: true });
  node.addEventListener("mousemove", move, { capture: true });
  node.addEventListener("wheel", wheel, { capture: true, passive: false });
  node.addEventListener("contextmenu", context, { capture: true });
  window.addEventListener("mouseup", up, { capture: true });

  return () => {
    node.removeEventListener("mousedown", down, { capture: true });
    node.removeEventListener("mousemove", move, { capture: true });
    node.removeEventListener("wheel", wheel, { capture: true });
    node.removeEventListener("contextmenu", context, { capture: true });
    window.removeEventListener("mouseup", up, { capture: true });
  };
};

export const controlMouse = (address: TabAddress) => {
  let tracked: Tracked | null = null;
  const gesture: Gesture = { held: null, hovered: null, wheelRest: 0 };

  return {
    track: (next: Tracked | null) => {
      tracked = next;
    },
    attach: (node: HTMLElement | null) => (node ? listen({ address, node, gesture, tracked: () => tracked }) : undefined),
  };
};
