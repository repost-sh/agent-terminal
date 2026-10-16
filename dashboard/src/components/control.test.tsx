import { act, fireEvent, render, screen } from "@testing-library/react";
import { expect, test } from "vitest";

import { App } from "../app";
import { startDaemon, type FakeDaemon } from "../fake-daemon/daemon";
import { frameOf, sessionInfo, tabInfo } from "../fake-daemon/messages";

const streamOf = (session: string) => `/api/session/${session}/t1/stream`;
const controlOf = (session: string) => `/api/session/${session}/t1/control`;

const announced = () => document.querySelector('[aria-live="polite"][aria-atomic="true"]')?.textContent;

const liveApp = async (session: string) => {
  const daemon = startDaemon([sessionInfo(session, [tabInfo("t1", { program: "vim" })])]);

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

  return screen.getByRole("application", { name: `Terminal ${session} t1, you have control` });
};

test("taking control focuses the terminal, and its keys and pastes reach the program", async () => {
  const daemon = await liveApp("keys");

  expect([screen.getByRole("button", { name: "Take control" }).outerHTML, screen.queryByText(/Keys go to/)]).toMatchSnapshot();
  const terminal = await held(daemon, "keys");

  expect(screen.getByRole("button", { name: "Release control" }).outerHTML).toMatchSnapshot();

  expect([document.activeElement, announced()]).toEqual([terminal, "You have control of keys t1"]);
  expect(screen.getByText("Keys go to vim · ctrl+p ctrl+q moves focus out")).toBeTruthy();
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
    { typed: "a" },
    { typed: "é" },
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

test("ctrl+p ctrl+q leaves the terminal, while ctrl+p then another key sends both", async () => {
  const daemon = await liveApp("chord");
  const terminal = await held(daemon, "chord");

  fireEvent.keyDown(terminal, { key: "p", code: "KeyP", ctrlKey: true });
  fireEvent.keyDown(terminal, { key: "x" });
  expect(daemon.sent(controlOf("chord"))).toEqual([{ pressed: "Ctrl+p" }, { typed: "x" }]);
  fireEvent.keyDown(terminal, { key: "p", code: "KeyP", ctrlKey: true });
  fireEvent.keyDown(terminal, { key: "q", code: "KeyQ", ctrlKey: true });
  expect(document.activeElement).toBe(screen.getByRole("button", { name: "Release control" }));
  expect(daemon.sent(controlOf("chord"))).toHaveLength(2);
  fireEvent.click(screen.getByRole("button", { name: "Release control" }));
  expect(announced()).toBe("Control returned to the agent");
  expect(screen.getByRole("button", { name: "Take control" })).toBeTruthy();
});

test("a refused take says so, a refused key names the program, and the agent's refusals are shown", async () => {
  const daemon = await liveApp("refused");

  fireEvent.click(await screen.findByRole("button", { name: "Take control" }));
  await daemon.connected(controlOf("refused"));
  await daemon.push(controlOf("refused"), { type: "refused", message: "the agent is mid-command" });
  expect(screen.getByText("Could not take control of refused t1")).toBeTruthy();
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
  expect(screen.queryByText(/Keys go to/)).toBeNull();
});
