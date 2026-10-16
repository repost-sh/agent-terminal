import { shellWord } from "./format";
import { NAME, type SessionInfo } from "./protocol";

interface Size { cols: string; rows: string }

export const SIZE_PRESETS: Size[] = [
  { cols: "80", rows: "24" },
  { cols: "120", rows: "40" },
  { cols: "200", rows: "60" },
];

export const sizeLabel = (size: Size) => `${size.cols}x${size.rows}`;

const SHELLS = ["zsh", "bash", "fish"];

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
  const taken = new Set(sessions.map((entry) => entry.session));
  const names = Array.from({ length: sessions.length + 1 }, (_, n) => `session-${String(sessions.length + 1 + n)}`);

  return String(names.find((name) => !taken.has(name)));
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
  const name = String(command.trim().split(/\s/)[0]).split("/").pop();

  return SHELLS.find((shell) => shell === name) ?? null;
};

export const sizeOf = (form: Form): Size =>
  SIZE_PRESETS.find((preset) => sizeLabel(preset) === form.size) ?? { cols: form.cols.trim(), rows: form.rows.trim() };

export const sessionOf = (form: Form) => (form.mode === "new" ? form.name.trim() : form.existing);

const cwdPart = (form: Form) => {
  const cwd = form.cwd.trim();

  return cwd && cwd !== "~" ? ` --cwd ${cwd}` : "";
};

const verbPart = (form: Form, shell: string | null) => (shell ? `shell --shell ${shell}` : `open ${form.command.trim() || "<program>"}`);

export const cliLine = (form: Form, shell: string | null) => {
  const session = sessionOf(form) ? shellWord(sessionOf(form)) : "<name>";
  const size = sizeOf(form);
  const created = form.mode === "new" ? `agent-terminal session new ${session} && ` : "";

  return `${created}agent-terminal --session ${session} ${verbPart(form, shell)}${cwdPart(form)} --size ${size.cols || "?"}x${size.rows || "?"}`;
};

interface Range { low: number; high: number }

const inRange = (text: string, range: Range) => /^\d+$/.test(text) && Number(text) >= range.low && Number(text) <= range.high;

const nameRefusal = (form: Form, sessions: SessionInfo[]) => {
  const name = form.name.trim();

  if (!name) {
    return "Enter a session name.";
  }

  if (!NAME.test(name)) {
    return "Use up to 24 of a-z, 0-9 and -, starting with a letter or digit.";
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
