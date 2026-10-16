import { Terminal } from "@xterm/xterm";
import { expect, test } from "vitest";

import { mirrorRows } from "./screen-rows";

const written = (terminal: Terminal, text: string) =>
  new Promise<void>((resolve) => {
    terminal.write(text, resolve);
  });

const mirrored = () => {
  const terminal = new Terminal({ cols: 10, rows: 3 });
  const host = document.createElement("div");
  const unmirror = mirrorRows(terminal, host);
  const rows = () => Array.from(host.querySelectorAll('[role="list"][aria-label="Screen"].sr-only > [role="listitem"]'), (item) => item.textContent);

  return { terminal, host, unmirror, rows };
};

test("the screen's rows are mirrored as a list that follows writes and resizes", async () => {
  const { terminal, rows } = mirrored();

  await written(terminal, "a\r\nb\r\nc");
  expect(rows()).toEqual(["a", "b", "c"]);
  terminal.resize(10, 2);
  expect(rows()).toEqual(["b", "c"]);
  terminal.resize(10, 3);
  expect(rows()).toEqual(["a", "b", "c"]);
  await written(terminal, "\x1b[2;1Hz");
  expect(rows()).toEqual(["a", "z", "c"]);
});

test("a row whose text did not change is left alone", async () => {
  const { terminal, host } = mirrored();

  await written(terminal, "a\r\nb");
  const changes: MutationRecord[] = [];
  const observer = new MutationObserver((records) => {
    changes.push(...records);
  });

  observer.observe(host, { subtree: true, childList: true, characterData: true });
  await written(terminal, "\x1b[2;1Hz");
  await Promise.resolve();
  observer.disconnect();
  expect([...new Set(changes.map((change) => change.target))]).toEqual([host.querySelectorAll('[role="listitem"]')[1]]);
});

test("after unmirroring the list is gone and writes no longer touch the host", async () => {
  const shown = mirrored();
  const { terminal, host } = shown;

  shown.unmirror();
  await written(terminal, "a");
  terminal.resize(10, 2);
  expect(host.childElementCount).toBe(0);
});

test("unmirroring stops listening to the terminal's writes and resizes", () => {
  const disposed: string[] = [];
  const listened = (name: string) => () => ({ dispose: () => disposed.push(name) });
  const terminal = { rows: 1, buffer: { active: { viewportY: 0, getLine: () => undefined } }, onWriteParsed: listened("writes"), onResize: listened("resizes") };

  mirrorRows(terminal as unknown as Terminal, document.createElement("div"))();
  expect(disposed).toEqual(["writes", "resizes"]);
});
