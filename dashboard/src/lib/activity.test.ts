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
    ].map(resultOf),
  ).toEqual([
    null,
    { label: "exit 1", tone: "destructive" },
    { label: "refused: human has control", tone: "warning" },
    { label: "exit 2 expect failed", tone: "destructive" },
    { label: "timeout", tone: "destructive" },
    { label: "exit 5 process exited", tone: "destructive" },
    { label: "exit 7", tone: "destructive" },
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
});

test("typing that empties keeps its row until something else is typed", () => {
  const rows = activityRows([typed("a"), pressed("Backspace"), pressed("Backspace"), typed("b")], 0);

  expect(rows.map((row) => row.human)).toEqual(['typed ""', "pressed Backspace", 'typed "b"']);
});
