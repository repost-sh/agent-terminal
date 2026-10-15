import { shellWord } from "./format";
import type { SessionInfo } from "./protocol";

export const SIZE_PRESETS = ["80x24", "120x40", "200x60"];

const SHELLS = ["zsh", "bash", "fish"];

const NAME = /^[A-Za-z0-9_-]+$/;

export type Mode = "new" | "existing";

export interface Form {
  mode: Mode;
  name: string;
  existing: string;
  command: string;
  cwd: string;
  size: string;
  cols: string;
  rows: string;
  integration: boolean | null;
}

export interface Errors { name?: string; command?: string; size?: string }

export const freeName = (sessions: SessionInfo[]) => {
  let n = sessions.length + 1;

  while (sessions.some((entry) => entry.session === `session-${String(n)}`)) {
    n += 1;
  }

  return `session-${String(n)}`;
};

export const initialForm = (sessions: SessionInfo[], existing: string | null): Form => ({
  mode: existing ? "existing" : "new",
  name: freeName(sessions),
  existing: existing ?? sessions[0]?.session ?? "",
  command: "",
  cwd: "~",
  size: "120x40",
  cols: "120",
  rows: "40",
  integration: null,
});

export const shellOf = (command: string) => {
  const name = command.trim().split(/\s+/)[0]?.split("/").pop() ?? "";

  return SHELLS.includes(name) ? name : null;
};

export const sizeOf = (form: Form) => {
  const [cols = "", rows = ""] = form.size === "custom" ? [form.cols.trim(), form.rows.trim()] : form.size.split("x");

  return { cols, rows };
};

export const sessionOf = (form: Form) => (form.mode === "new" ? form.name.trim() : form.existing);

const cwdPart = (form: Form) => {
  const cwd = form.cwd.trim();

  return cwd && cwd !== "~" ? ` --cwd ${cwd}` : "";
};

const verbPart = (form: Form, shell: string | null) => (shell ? `shell --shell ${shell}` : `open ${form.command.trim() || "<program>"}`);

export const cliLine = (form: Form, shell: string | null) => {
  const session = sessionOf(form) ? shellWord(sessionOf(form)) : "<name>";
  const size = sizeOf(form);

  return `agent-terminal --session ${session} ${verbPart(form, shell)}${cwdPart(form)} --size ${size.cols || "?"}x${size.rows || "?"}`;
};

interface Range { low: number; high: number }

const inRange = (text: string, range: Range) => /^\d+$/.test(text) && Number(text) >= range.low && Number(text) <= range.high;

const nameRefusal = (form: Form, sessions: SessionInfo[]) => {
  const name = form.name.trim();

  if (!name) {
    return "Enter a session name.";
  }

  if (!NAME.test(name)) {
    return "Use letters, numbers, - and _.";
  }

  return sessions.some((entry) => entry.session === name) ? `A session named ${name} already exists.` : undefined;
};

const sizeFits = (form: Form) => inRange(form.cols.trim(), { low: 20, high: 500 }) && inRange(form.rows.trim(), { low: 5, high: 200 });

export const refusals = (form: Form, sessions: SessionInfo[]): Errors => {
  const errors: Errors = {};
  const name = form.mode === "new" ? nameRefusal(form, sessions) : undefined;

  if (name) {
    errors.name = name;
  }

  if (!form.command.trim()) {
    errors.command = "Enter a command.";
  }

  if (form.size === "custom" && !sizeFits(form)) {
    errors.size = "Columns 20–500, rows 5–200.";
  }

  return errors;
};
