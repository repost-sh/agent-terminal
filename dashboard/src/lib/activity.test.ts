import { expect, test } from "vitest";

import { entryOf } from "../fake-daemon/messages";
import { activityRows, resultOf } from "./activity";

test("a failed call reads as its exit, named when the daemon explained it", () => {
  expect(
    [
      entryOf("snapshot"),
      entryOf("open", { exit: 1 }),
      entryOf("press", { exit: 4, error: "a human has control" }),
      entryOf("expect", { exit: 2, error: "no match" }),
      entryOf("wait", { exit: 3, error: "timed out" }),
      entryOf("wait", { exit: 5, error: "exited" }),
      entryOf("open", { exit: 7, error: "odd" }),
      entryOf("expect", { exit: 2 }),
      entryOf("press", { exit: 4 }),
    ].map(resultOf),
  ).toEqual([
    null,
    { label: "exit 1", tone: "destructive" },
    { label: "refused: human has control", tone: "warning" },
    { label: "exit 2 expect failed", tone: "destructive" },
    { label: "timeout", tone: "destructive" },
    { label: "exit 5 process exited", tone: "destructive" },
    { label: "exit 7", tone: "destructive" },
    { label: "exit 2", tone: "destructive" },
    { label: "exit 4", tone: "destructive" },
  ]);
});

const typed = (text: string) => entryOf("type", { human: { typed: text } });

const pressed = (key: string) => entryOf("press", { human: { pressed: key } });

test("consecutive typing reads as one row that Backspace edits", () => {
  const rows = activityRows([typed("ab"), typed("ç"), pressed("Backspace"), pressed("Backspace"), pressed("Enter"), typed("x")], 10);

  expect(rows.map((row) => [row.at, row.human])).toEqual([
    [10, 'typed "a"'],
    [14, "pressed Enter"],
    [15, 'typed "x"'],
  ]);
});

test("Backspace with nothing typed, pastes, refused typing and agent calls are rows of their own", () => {
  const refused = entryOf("type", { exit: 4, error: "agent has control", human: { typed: "z" } });
  const rows = activityRows([pressed("Backspace"), entryOf("paste", { human: { pasted: "p" } }), refused, entryOf("snapshot")], 0);

  expect(rows.map((row) => row.human)).toEqual(["pressed Backspace", 'pasted "p"', 'typed "z"', null]);
  expect(activityRows([typed("a"), refused], 0).map((row) => row.human)).toEqual(['typed "a"', 'typed "z"']);
});

test("typing that empties keeps its row until something else is typed", () => {
  const rows = activityRows([typed("a"), pressed("Backspace"), pressed("Backspace"), typed("b")], 0);

  expect(rows.map((row) => row.human)).toEqual(['typed ""', "pressed Backspace", 'typed "b"']);
});

test("mouse and focus input read as what the person did, with the button and the held modifiers", () => {
  const none = { shift: false, alt: false, ctrl: false };
  const rows = activityRows(
    [
      entryOf("mouse", { human: { mouse: { event: "down", row: 2, col: 3, button: "left", mods: { ...none, ctrl: true } } } }),
      entryOf("mouse", { human: { mouse: { event: "up", row: 2, col: 4, button: "right", mods: { shift: true, alt: true, ctrl: true } } } }),
      entryOf("mouse", { human: { mouse: { event: "wheel", row: 2, col: 4, direction: "down", mods: none } } }),
      entryOf("mouse", { human: { mouse: { event: "move", row: 5, col: 6, mods: { ...none, alt: true } } } }),
      entryOf("mouse", { human: { mouse: { event: "down", row: 1, col: 1, button: "middle", mods: none } } }),
      entryOf("focus", { human: { focused: true } }),
      entryOf("focus", { human: { focused: false } }),
    ],
    0,
  );

  expect(rows.map((row) => row.human)).toEqual([
    "mouse down at 2,3 with Ctrl",
    "mouse up at 2,4 right button with Ctrl+Alt+Shift",
    "mouse wheel down at 2,4",
    "mouse move to 5,6 with Alt",
    "mouse down at 1,1 middle button",
    "focus in",
    "focus out",
  ]);
});
