import { expect, test } from "vitest";

import { sessionInfo } from "../fake-daemon/messages";
import { cliLine, freeName, initialForm, refusals, sessionOf, shellOf, SIZE_PRESETS, sizeLabel, sizeOf, type Form } from "./new-session-form";

const sessions = [sessionInfo("session-2", []), sessionInfo("work", [])];

const form = (change: Partial<Form>): Form => ({ ...initialForm(sessions, null), ...change });

test("a new form names a free session, or opens in the session it was asked for", () => {
  expect(freeName([])).toBe("session-1");
  expect(freeName(sessions)).toBe("session-3");
  expect(freeName([sessionInfo("session-1", [])])).toBe("session-2");
  expect(freeName([sessionInfo("session-2", []), sessionInfo("session-3", [])])).toBe("session-4");
  expect(initialForm(sessions, null)).toEqual({
    mode: "new",
    name: "session-3",
    existing: "session-2",
    command: "",
    cwd: "~",
    size: "120x40",
    cols: "120",
    rows: "40",
    integration: null,
  });
  expect([initialForm(sessions, "work").mode, initialForm(sessions, "work").existing]).toEqual(["existing", "work"]);
  expect(initialForm([], null).existing).toBe("");
});

test("the dialog offers three sizes besides a custom one", () => {
  expect(SIZE_PRESETS.map(sizeLabel)).toEqual(["80x24", "120x40", "200x60"]);
});

test("a known shell is recognised by its program name", () => {
  expect(["zsh", "  /bin/bash -l", "/usr/local/bin/fish", "python3", "", "  "].map(shellOf)).toEqual(["zsh", "bash", "fish", null, null, null]);
});

test("the size comes from the preset or from the custom columns and rows", () => {
  expect(sizeOf(form({ size: "80x24" }))).toEqual({ cols: "80", rows: "24" });
  expect(sizeOf(form({ size: "custom", cols: " 100 ", rows: " 30 " }))).toEqual({ cols: "100", rows: "30" });
  expect(sessionOf(form({ name: " fresh " }))).toBe("fresh");
  expect(sessionOf(form({ mode: "existing", existing: "work" }))).toBe("work");
});

test("the CLI line mirrors the form, creating the session first when it is new", () => {
  expect(cliLine(form({}), null)).toBe("agent-terminal session new session-3 && agent-terminal --session session-3 open <program> --size 120x40");
  expect(cliLine(form({ name: "", command: " htop ", cwd: "~" }), null)).toBe(
    "agent-terminal session new <name> && agent-terminal --session <name> open htop --size 120x40",
  );
  expect(cliLine(form({ mode: "existing", existing: "work", command: "zsh", cwd: " /srv " }), "zsh")).toBe(
    "agent-terminal --session work shell --shell zsh --cwd /srv --size 120x40",
  );
  expect(cliLine(form({ mode: "existing", existing: "my work", command: "htop" }), null)).toBe("agent-terminal --session 'my work' open htop --size 120x40");
  expect(cliLine(form({ size: "custom", cols: "", rows: "" }), null)).toBe(
    "agent-terminal session new session-3 && agent-terminal --session session-3 open <program> --size ?x?",
  );
});

test("refusals name every field that cannot be opened", () => {
  expect(refusals(form({ command: "bash" }), sessions)).toEqual({});
  expect(refusals(form({ name: " ", command: "" }), sessions)).toEqual({ name: "Enter a session name.", command: "Enter a command." });
  const ruled = { name: "Use up to 24 of a-z, 0-9 and -, starting with a letter or digit." };

  expect(["a b", "Work", "a_b", "-x", "abcdefghijklmnopqrstuvwxy"].map((name) => refusals(form({ name, command: "x" }), sessions))).toEqual(
    Array.from({ length: 5 }, () => ruled),
  );
  expect(["9lives", "x-", "abcdefghijklmnopqrstuvwx", " repost-forward "].map((name) => refusals(form({ name, command: "x" }), sessions))).toEqual(
    Array.from({ length: 4 }, () => ({})),
  );
  expect(refusals(form({ name: "work", command: "x" }), sessions)).toEqual({ name: "A session named work already exists." });
  expect(refusals(form({ mode: "existing", name: "", command: "x" }), sessions)).toEqual({});
  expect(refusals(form({ command: "  " }), sessions)).toEqual({ command: "Enter a command." });
  expect(refusals(form({ command: "x", size: "80x24", cols: "1", rows: "1" }), sessions)).toEqual({});
});

test("a custom size must fit the terminal limits", () => {
  const sized = (cols: string, rows: string) => refusals(form({ command: "x", size: "custom", cols, rows }), sessions);
  const refused = { size: "Columns 20–500, rows 5–200." };

  expect([sized("20", "5"), sized("500", "200"), sized(" 100 ", " 30 ")]).toEqual([{}, {}, {}]);
  expect(
    [sized("19", "5"), sized("501", "5"), sized("20", "4"), sized("20", "201"), sized("2x", "5"), sized("", ""), sized("1e2", "5"), sized("0x20", "5")],
  ).toEqual(Array.from({ length: 8 }, () => refused));
});
