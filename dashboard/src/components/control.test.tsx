import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import { App } from "../app";
import { startDaemon, type FakeDaemon } from "../fake-daemon/daemon";
import { frameOf, sessionInfo, tabInfo } from "../fake-daemon/messages";

const streamOf = (session: string) => `/api/session/${session}/app/stream`;
const controlOf = (session: string) => `/api/session/${session}/app/control`;

const announced = () => screen.getByRole("status", { name: "Announcements" }).textContent;

const liveApp = async (session: string) => {
  const daemon = startDaemon([sessionInfo(session, [tabInfo("app", { program: "vim" })])]);

  render(<App />);
  await daemon.serve();
  await daemon.connected(streamOf(session));
  expect(screen.getByRole<HTMLButtonElement>("button", { name: "Take control" }).disabled).toBe(true);
  await daemon.push(streamOf(session), frameOf(1));

  return daemon;
};

const held = async (daemon: FakeDaemon, session: string) => {
  fireEvent.click(await screen.findByRole("button", { name: "Take control" }));
  expect(screen.getByRole<HTMLButtonElement>("button", { name: "Release control" }).disabled).toBe(true);
  await daemon.connected(controlOf(session));
  await daemon.push(controlOf(session), { type: "held" });

  return screen.getByRole("application", { name: `Terminal ${session} app, you have control` });
};

test("taking control focuses the terminal, and its keys and pastes reach the program", async () => {
  const daemon = await liveApp("keys");

  expect(screen.getByRole("button", { name: "Take control" }).outerHTML).toMatchSnapshot();
  const terminal = await held(daemon, "keys");

  expect(screen.getByRole("button", { name: "Release control" }).outerHTML).toMatchSnapshot();

  expect([terminal.matches(":focus"), announced()]).toEqual([true, "You have control of keys app"]);
  const keys = [
    { key: "a" },
    { key: "é" },
    { key: "Enter" },
    { key: "Tab", shiftKey: true },
    { key: "F5", ctrlKey: true },
    { key: "c", code: "KeyC", ctrlKey: true },
    { key: "!", code: "Digit1", ctrlKey: true, altKey: true },
    { key: " ", code: "Space", ctrlKey: true },
    { key: "ß", code: "Minus", altKey: true },
    { key: "Dead", code: "Quote", altKey: true },
    { key: "Shift" },
    { key: "Unidentified" },
    { key: "v", metaKey: true },
  ];

  for (const init of keys) {
    fireEvent.keyDown(terminal, init);
  }

  fireEvent.paste(terminal, { clipboardData: { getData: () => "pasted text" } });
  fireEvent.paste(terminal, { clipboardData: { getData: () => "" } });
  expect(daemon.sent(controlOf("keys"))).toEqual([
    { focused: true },
    { typed: "a" }, { typed: "é" },
    { pressed: "Enter" },
    { pressed: "Shift+Tab" },
    { pressed: "Ctrl+F5" },
    { pressed: "Ctrl+c" },
    { pressed: "Ctrl+Alt+1" },
    { pressed: "Ctrl+Space" },
    { pressed: "Alt+ß" },
    { pasted: "pasted text" },
  ]);
});

test("ctrl+p reaches the program at once, and releasing control says so", async () => {
  const daemon = await liveApp("chord");
  const terminal = await held(daemon, "chord");

  fireEvent.keyDown(terminal, { key: "p", code: "KeyP", ctrlKey: true });
  fireEvent.keyDown(terminal, { key: "x" });
  expect(daemon.sent(controlOf("chord"))).toEqual([{ focused: true }, { pressed: "Ctrl+p" }, { typed: "x" }]);
  fireEvent.click(screen.getByRole("button", { name: "Release control" }));
  expect(announced()).toBe("Control returned to the agent");
  expect(screen.getByRole("button", { name: "Take control" })).toHaveFocus();
});

test("a refused take says so, a refused key names the program, and the agent's refusals are shown", async () => {
  const daemon = await liveApp("refused");

  fireEvent.click(await screen.findByRole("button", { name: "Take control" }));
  await daemon.connected(controlOf("refused"));
  await daemon.push(controlOf("refused"), { type: "refused", message: "the agent is mid-command" });
  expect(screen.getByText("Could not take control of refused app")).toBeTruthy();
  expect(screen.getByText("the agent is mid-command")).toBeTruthy();
  expect(screen.getByRole("button", { name: "Take control" })).toBeTruthy();
  await daemon.hangUp(controlOf("refused"));
  await held(daemon, "refused");
  await daemon.push(controlOf("refused"), { type: "refused", message: "the program exited" });
  expect(screen.getByText("Could not send the key to vim")).toBeTruthy();
  await daemon.push(controlOf("refused"), { type: "agent-refused", command: "press Enter" });
  expect(screen.getByText("The agent is waiting for control")).toBeTruthy();
  expect(screen.getByText("press Enter refused · exit 4 · now: wait --control")).toBeTruthy();
  await daemon.push(controlOf("refused"), { type: "closed" });
  expect([announced(), screen.queryByRole("application")]).toEqual(["Control returned to the agent", null]);
});

test("control cannot be taken while the stream reconnects", async () => {
  const daemon = await liveApp("reconnecting");

  await screen.findByRole("group", { name: /live frame 1/ });
  expect(screen.getByRole<HTMLButtonElement>("button", { name: "Take control" }).disabled).toBe(false);
  await daemon.hangUp(streamOf("reconnecting"));
  expect(screen.getByRole<HTMLButtonElement>("button", { name: "Take control" }).disabled).toBe(true);
});

test("control ends when the daemon closes the socket, and a second take is ignored while one is pending", async () => {
  const daemon = await liveApp("dropped");

  await held(daemon, "dropped");
  await daemon.hangUp(controlOf("dropped"));
  expect(announced()).toBe("Control returned to the agent");
  await act(async () => {
    fireEvent.click(screen.getByRole("button", { name: "Take control" }));
    await Promise.resolve();
  });
  await daemon.push(streamOf("dropped"), frameOf(2, { exited: true }));
  expect(screen.queryByRole("application")).toBeNull();
});

const terminalFrame = () => screen.getByRole("group", { name: /live frame/ });

test("the mouse goes to an app that tracks it until it is switched off on the toolbar, and shift keeps selecting text", async () => {
  const daemon = await liveApp("mouse");

  await daemon.push(streamOf("mouse"), frameOf(2, { cols: 80, rows: 24, mouse: "sgr", mouse_tracking: "click" }));
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue(new DOMRect(0, 0, 800, 240));
  const terminal = await held(daemon, "mouse");

  fireEvent.mouseDown(terminalFrame(), { clientX: 25, clientY: 15, button: 0 });
  fireEvent.mouseUp(terminalFrame(), { clientX: 25, clientY: 15, button: 0 });
  fireEvent.mouseDown(terminalFrame(), { clientX: 25, clientY: 15, button: 0, shiftKey: true });
  expect(daemon.sent(controlOf("mouse")).slice(1)).toEqual([
    { mouse: { event: "down", row: 2, col: 3, button: "left", mods: { shift: false, alt: false, ctrl: false } } },
    { mouse: { event: "up", row: 2, col: 3, button: "left", mods: { shift: false, alt: false, ctrl: false } } },
  ]);
  expect(terminalFrame().className).toContain("is-mouse");

  const toggle = screen.getByRole("button", { name: "Mouse on" });

  expect([toggle.getAttribute("aria-pressed"), toggle.getAttribute("data-tooltip")]).toEqual(["true", "While you hold control, the mouse goes to the app when it asks for it; shift+click selects text"]);
  const lookOn = toggle.className;

  fireEvent.click(toggle);
  const off = screen.getByRole("button", { name: "Mouse off" });

  expect([off.getAttribute("aria-pressed"), off.getAttribute("data-tooltip"), localStorage.getItem("agent-terminal-mouse"), off.className === lookOn]).toEqual(["false", "The mouse selects text and never reaches the app", "off", false]);
  fireEvent.mouseDown(terminalFrame(), { clientX: 25, clientY: 15, button: 0 });
  expect([daemon.sent(controlOf("mouse")).length, terminalFrame().className.includes("is-mouse")]).toEqual([3, false]);
  fireEvent.click(screen.getByRole("button", { name: "Mouse off" }));
  fireEvent.blur(terminal);
  fireEvent.focus(terminal);
  expect(daemon.sent(controlOf("mouse")).slice(3)).toEqual([{ focused: false }, { focused: true }]);
});

test("take control shows while the pointer moves over the terminal and fades once it rests or leaves", async () => {
  vi.useFakeTimers({ shouldAdvanceTime: true });
  const daemon = await liveApp("moving");
  const viewport = screen.getByTestId("viewport");
  const visible = () => viewport.dataset["moving"] === "true";

  expect(visible()).toBe(false);
  fireEvent.mouseMove(viewport);
  expect(visible()).toBe(true);
  act(() => {
    vi.advanceTimersByTime(1400);
  });
  fireEvent.mouseMove(viewport);
  act(() => {
    vi.advanceTimersByTime(1400);
  });
  expect(visible()).toBe(true);
  act(() => {
    vi.advanceTimersByTime(200);
  });
  expect(visible()).toBe(false);
  fireEvent.mouseMove(viewport);
  fireEvent.mouseLeave(viewport);
  expect(visible()).toBe(false);
  await held(daemon, "moving");
  expect([screen.queryByRole("button", { name: "Take control" }), within(screen.getByTestId("viewport-bottom")).getByRole("button").textContent]).toEqual([null, "Release control"]);
});
