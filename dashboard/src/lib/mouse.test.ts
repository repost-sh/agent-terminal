import fc from "fast-check";
import { beforeEach, expect, test, vi } from "vitest";

import { accept, deliver, everySocket, FakeSocket, forgetSockets } from "../fake-daemon/sockets";
import { frameOf } from "../fake-daemon/messages";
import type { Tracked, Tracking } from "./mouse";

const CONTROL = "/api/session/work/app/control";

const ADDRESS = { session: "work", tab: "app" };

const BOX = new DOMRect(10, 20, 800, 240);

const GRID = { cols: 80, rows: 24 };

const tracked = (tracking: Tracking): Tracked => ({ tracking, ...GRID });

const heldTerminal = async () => {
  const control = await import("./control");
  const { controlMouse, trackedIn } = await import("./mouse");

  control.takeControl(ADDRESS, "vim");
  const [socket = new FakeSocket("ws://daemon.test/none")] = everySocket().filter((made) => made.path === CONTROL);

  accept(socket);
  deliver(socket, { type: "held" });
  const node = document.createElement("div");
  const screen = document.createElement("div");

  screen.className = "xterm-screen";
  screen.getBoundingClientRect = () => BOX;
  node.append(screen);
  document.body.append(node);
  const mouse = controlMouse(ADDRESS);
  const detach = mouse.attach(node) ?? (() => undefined);

  return { socket, node, screen, mouse, detach, trackedIn, sent: () => socket.sent.map((input) => (input as { mouse: unknown }).mouse) };
};

interface At { col: number; row: number }

const clientAt = (at: At) => ({ clientX: BOX.left + (at.col - 0.5) * 10, clientY: BOX.top + (at.row - 0.5) * 10 });

const mouseEvent = (type: string, init: MouseEventInit) => new MouseEvent(type, { bubbles: true, cancelable: true, ...init });

const NONE = { shift: false, alt: false, ctrl: false };

beforeEach(() => {
  vi.resetModules();
  forgetSockets();
  vi.stubGlobal("WebSocket", FakeSocket);
  document.body.replaceChildren();
});

test("a press, a drag and a release reach the app as cells, a move once per cell, with the held modifiers", async () => {
  const held = await heldTerminal();

  held.mouse.track(tracked("drag"));
  held.screen.dispatchEvent(mouseEvent("mousedown", { ...clientAt({ row: 2, col: 3 }), button: 0, ctrlKey: true }));
  held.screen.dispatchEvent(mouseEvent("mousemove", { ...clientAt({ row: 2, col: 3 }), buttons: 1 }));
  held.screen.dispatchEvent(mouseEvent("mousemove", { ...clientAt({ row: 2, col: 4 }), buttons: 1 }));
  held.screen.dispatchEvent(mouseEvent("mousemove", { ...clientAt({ row: 2, col: 4 }), buttons: 1 }));
  window.dispatchEvent(mouseEvent("mouseup", { ...clientAt({ row: 3, col: 5 }), button: 0, altKey: true }));
  held.screen.dispatchEvent(mouseEvent("mousemove", clientAt({ row: 3, col: 6 })));
  expect(held.sent()).toEqual([
    { event: "down", row: 2, col: 3, button: "left", mods: { ...NONE, ctrl: true } },
    { event: "move", row: 2, col: 4, mods: NONE },
    { event: "up", row: 3, col: 5, button: "left", mods: { ...NONE, alt: true } },
  ]);
});

test("click tracking sends presses and releases only, motion tracking every cell the pointer crosses, and a press is kept from xterm", async () => {
  const held = await heldTerminal();
  const reachedXterm: string[] = [];

  held.screen.addEventListener("mousedown", (event) => reachedXterm.push(`${event.type} ${String(event.shiftKey)}`));
  held.mouse.track(tracked("click"));
  held.screen.dispatchEvent(mouseEvent("mousemove", clientAt({ row: 1, col: 1 })));
  held.screen.dispatchEvent(mouseEvent("mousedown", { ...clientAt({ row: 1, col: 1 }), button: 2 }));
  held.screen.dispatchEvent(mouseEvent("mousemove", clientAt({ row: 1, col: 2 })));
  window.dispatchEvent(mouseEvent("mouseup", { ...clientAt({ row: 1, col: 2 }), button: 2 }));
  held.mouse.track(tracked("motion"));
  held.screen.dispatchEvent(mouseEvent("mousemove", clientAt({ row: 5, col: 5 })));
  held.screen.dispatchEvent(mouseEvent("mousemove", clientAt({ row: 5, col: 5 })));
  held.screen.dispatchEvent(mouseEvent("mousemove", { ...clientAt({ row: 5, col: 6 }), shiftKey: true }));
  held.screen.dispatchEvent(mouseEvent("mousedown", { ...clientAt({ row: 5, col: 6 }), button: 1, shiftKey: true }));
  expect(held.sent()).toEqual([
    { event: "down", row: 1, col: 1, button: "right", mods: NONE },
    { event: "up", row: 1, col: 2, button: "right", mods: NONE },
    { event: "move", row: 5, col: 5, mods: NONE },
  ]);
  expect(reachedXterm).toEqual(["mousedown true"]);
});

test("shift leaves the right-click menu alone, and otherwise the menu is kept from the page", async () => {
  const held = await heldTerminal();
  const menu = (shiftKey: boolean) => {
    const event = mouseEvent("contextmenu", { ...clientAt({ row: 1, col: 1 }), button: 2, shiftKey });

    held.screen.dispatchEvent(event);

    return event.defaultPrevented;
  };

  expect(menu(false)).toBe(false);
  held.mouse.track(tracked("click"));
  expect([menu(false), menu(true)]).toEqual([true, false]);
});

test("the wheel turns once per line, keeps the rest for the next turn, and reads lines and pages as the browser gives them", async () => {
  const held = await heldTerminal();
  const turn = (init: WheelEventInit) => {
    const event = new WheelEvent("wheel", { bubbles: true, cancelable: true, ...clientAt({ row: 4, col: 4 }), ...init });

    const at = clientAt({ row: 4, col: 4 });

    Object.defineProperties(event, {
      shiftKey: { value: Boolean(init.shiftKey) },
      ctrlKey: { value: Boolean(init.ctrlKey) },
      altKey: { value: false },
      clientX: { value: at.clientX },
      clientY: { value: at.clientY },
    });
    held.screen.dispatchEvent(event);

    return event.defaultPrevented;
  };

  expect(turn({ deltaY: 15, deltaMode: 0 })).toBe(false);
  held.mouse.track(tracked("click"));
  const prevented = [
    turn({ deltaY: 15, deltaMode: 0 }),
    turn({ deltaY: 5, deltaMode: 0 }),
    turn({ deltaY: -10, deltaMode: 0, ctrlKey: true }),
    turn({ deltaY: 2, deltaMode: 1 }),
    turn({ deltaY: 0.1, deltaMode: 2 }),
    turn({ deltaY: 1, deltaMode: 0, shiftKey: true }),
  ];

  expect(prevented).toEqual([true, true, true, true, true, false]);
  expect(held.sent().map((sent) => (sent as { direction: string }).direction)).toEqual(["down", "down", "up", "down", "down", "down", "down"]);
  expect(held.sent()[2]).toEqual({ event: "wheel", row: 4, col: 4, direction: "up", mods: { ...NONE, ctrl: true } });
});

test("nothing is sent while the app does not track the mouse, the screen is not drawn, or the terminal is detached", async () => {
  const held = await heldTerminal();
  const press = () => {
    held.screen.dispatchEvent(mouseEvent("mousedown", { ...clientAt({ row: 1, col: 1 }), button: 0 }));
  };

  press();
  held.mouse.track(tracked("click"));
  held.screen.getBoundingClientRect = () => new DOMRect(0, 0, 0, 0);
  press();
  held.screen.getBoundingClientRect = () => BOX;
  held.detach();
  press();
  expect([held.sent(), held.mouse.attach(null)]).toEqual([[], undefined]);
});

test("a release without a press, a release with the screen gone, and a button with no terminal code are left alone", async () => {
  const held = await heldTerminal();

  held.mouse.track(tracked("click"));
  window.dispatchEvent(mouseEvent("mouseup", { ...clientAt({ row: 1, col: 1 }), button: 0 }));
  held.screen.dispatchEvent(mouseEvent("mousedown", { ...clientAt({ row: 1, col: 1 }), button: 3 }));
  held.screen.dispatchEvent(mouseEvent("mousedown", { ...clientAt({ row: 1, col: 1 }), button: 0 }));
  held.screen.remove();
  window.dispatchEvent(mouseEvent("mouseup", { ...clientAt({ row: 1, col: 1 }), button: 0 }));
  held.node.append(held.screen);
  held.mouse.track(tracked("drag"));
  held.screen.dispatchEvent(mouseEvent("mousemove", clientAt({ row: 2, col: 2 })));
  expect(held.sent()).toEqual([{ event: "down", row: 1, col: 1, button: "left", mods: NONE }]);
});

test("a press outside the drawn screen lands on its nearest edge cell", async () => {
  const held = await heldTerminal();

  held.mouse.track(tracked("click"));
  held.screen.dispatchEvent(mouseEvent("mousedown", { clientX: BOX.right + 50, clientY: BOX.bottom + 50, button: 0 }));
  held.screen.dispatchEvent(mouseEvent("mousedown", { clientX: BOX.left - 50, clientY: BOX.top - 50, button: 0 }));
  expect(held.sent().map((sent) => [(sent as At).row, (sent as At).col])).toEqual([
    [24, 80],
    [1, 1],
  ]);
});

test("every point on the drawn screen maps to a cell of the grid, and points in the same cell map alike", async () => {
  const held = await heldTerminal();

  held.mouse.track(tracked("motion"));
  fc.assert(
    fc.property(fc.double({ min: 0, max: 799.99, noNaN: true }), fc.double({ min: 0, max: 239.99, noNaN: true }), (x, y) => {
      held.socket.sent.length = 0;
      held.screen.dispatchEvent(mouseEvent("mousedown", { clientX: BOX.left + x, clientY: BOX.top + y, button: 0 }));
      const [sent] = held.sent() as At[];

      expect(sent).toEqual({ event: "down", row: Math.floor(y / 10) + 1, col: Math.floor(x / 10) + 1, button: "left", mods: NONE });
    }),
  );
});

test("a frame says which cells the app tracks, and nothing when it does not take the mouse", async () => {
  const { trackedIn } = await heldTerminal();

  expect([
    trackedIn(null),
    trackedIn(frameOf(1)),
    trackedIn(frameOf(1, { mouse: "sgr", mouse_tracking: "motion", cols: 100, rows: 30 })),
    trackedIn(frameOf(1, { mouse: "sgr", mouse_tracking: "drag" })),
    trackedIn(frameOf(1, { mouse: "sgr", mouse_tracking: "pixels" })),
  ]).toEqual([null, null, { tracking: "motion", cols: 100, rows: 30 }, { tracking: "drag", cols: 80, rows: 24 }, null]);
});
