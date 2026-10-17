import type { ClipboardEvent, KeyboardEvent } from "react";
import { beforeEach, expect, test, vi } from "vitest";

import { accept, deliver, everySocket, FakeSocket, forgetSockets } from "../fake-daemon/sockets";

const CONTROL = "/api/session/my%20work/app/control";

const control = async () => {
  const lib = await import("./control");
  const { announcements } = await import("./announcements");

  return { ...lib, announcements };
};

const socketAt = (path: string) => everySocket().filter((socket) => socket.path === path);

const held = async () => {
  const lib = await control();

  lib.takeControl({ session: "my work", tab: "app" }, "vim");
  const [socket = new FakeSocket("ws://daemon.test/none")] = socketAt(CONTROL);

  accept(socket);
  deliver(socket, { type: "held" });

  return { ...lib, socket };
};

interface Pressed { key: string; code?: string; ctrlKey?: boolean; altKey?: boolean; shiftKey?: boolean; metaKey?: boolean }

const keyDown = (pressed: Pressed) => {
  const event = { code: "", ctrlKey: false, altKey: false, shiftKey: false, metaKey: false, ...pressed, preventDefault: vi.fn(), stopPropagation: vi.fn() };

  return { event: event as unknown as KeyboardEvent<HTMLElement>, prevented: event.preventDefault, stopped: event.stopPropagation };
};

const sentFor = async (presses: Pressed[]) => {
  const { controlKeys, socket } = await held();
  const { keys } = controlKeys("my work", "app");

  for (const pressed of presses) {
    keys(keyDown(pressed).event);
  }

  return socket.sent;
};

beforeEach(() => {
  vi.resetModules();
  forgetSockets();
  vi.stubGlobal("WebSocket", FakeSocket);
});

test("every named key and function key reaches the program by its name, and other keys do not", async () => {
  const named = ["Escape", "Backspace", "Delete", "Insert", "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "Home", "End", "PageUp", "PageDown"];
  const ignored = ["Control", "Alt", "AltGraph", "Meta", "CapsLock", "NumLock", "ScrollLock", "Fn", "OS", "F13", "F0", "XF1", "F10x"];

  expect(await sentFor([...named, "F1", "F9", "F10", "F12", ...ignored].map((key) => ({ key })))).toEqual(
    ["Esc", "Backspace", "Delete", "Insert", "Up", "Down", "Left", "Right", "Home", "End", "PageUp", "PageDown", "F1", "F9", "F10", "F12"].map((pressed) => ({ pressed })),
  );
});

test("a held key with ctrl reads the physical letter or digit, whatever the layout prints", async () => {
  const ctrl = (key: string, code: string) => ({ key, code, ctrlKey: true });

  expect(
    await sentFor([ctrl("ф", "KeyA"), ctrl("ф", "XKeyA"), ctrl("ф", "KeyAB"), ctrl("ф", "Key1"), ctrl("&", "Digit1"), ctrl("&", "XDigit1"), ctrl("&", "Digit12")]),
  ).toEqual(["Ctrl+a", "Ctrl+ф", "Ctrl+ф", "Ctrl+ф", "Ctrl+1", "Ctrl+&", "Ctrl+&"].map((pressed) => ({ pressed })));
});

test("ctrl+p and ctrl+q are keys like any other and reach the program at once", async () => {
  const presses = [
    { key: "p", code: "KeyP", ctrlKey: true },
    { key: "q", code: "KeyQ", ctrlKey: true },
    { key: "p", code: "KeyP", ctrlKey: true, altKey: true },
  ];

  expect(await sentFor(presses)).toEqual([{ pressed: "Ctrl+p" }, { pressed: "Ctrl+q" }, { pressed: "Ctrl+Alt+p" }]);
});

test("a key the terminal takes is kept from the page, and a lone modifier or a meta chord is not", async () => {
  const { controlKeys } = await held();
  const { keys } = controlKeys("my work", "app");
  const taken = keyDown({ key: "a" });
  const meta = keyDown({ key: "v", metaKey: true });
  const modifiers = ["Shift", "Control", "Alt", "AltGraph", "Meta", "CapsLock", "NumLock", "ScrollLock", "Fn", "OS"].map((key) => keyDown({ key }));

  keys(taken.event);
  keys(meta.event);
  modifiers.forEach((modifier) => {
    keys(modifier.event);
  });
  expect([taken.prevented, taken.stopped, meta.prevented].map((spy) => spy.mock.calls.length)).toEqual([1, 1, 0]);
  expect(modifiers.filter((modifier) => modifier.prevented.mock.calls.length > 0)).toEqual([]);
});

test("a paste is kept from the page and sends its text, and nothing when it is empty", async () => {
  const { controlKeys, socket } = await held();
  const { paste } = controlKeys("my work", "app");
  const pasted = (text: string) => {
    const event = { clipboardData: { getData: (format: string) => (format === "text" ? text : "") }, preventDefault: vi.fn() };

    paste(event as unknown as ClipboardEvent<HTMLElement>);

    return event.preventDefault.mock.calls.length;
  };

  expect([pasted("ls\n"), pasted("")]).toEqual([1, 1]);
  expect(socket.sent).toEqual([{ pasted: "ls\n" }]);
});

test("keys sent while the socket is closing are dropped", async () => {
  const { controlKeys, socket } = await held();

  socket.readyState = FakeSocket.CLOSING;
  controlKeys("my work", "app").keys(keyDown({ key: "a" }).event);
  expect(socket.sent).toEqual([]);
});

test("a second take of the same tab is ignored, and a release closes the socket and says so once", async () => {
  const taken = await held();

  taken.takeControl({ session: "my work", tab: "app" }, "vim");
  expect(socketAt(CONTROL)).toHaveLength(1);
  const before = taken.announcements.get().times;

  taken.releaseControl("my work", "app");
  taken.releaseControl("my work", "app");
  expect([taken.socket.readyState, taken.announcements.get()]).toEqual([FakeSocket.CLOSED, { text: "Control returned to the agent", times: before + 1 }]);
});
