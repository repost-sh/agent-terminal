import { shellWord } from "./format";
import { NAME, NAME_RULE, type SessionInfo } from "./protocol";
import { findTab } from "./tabs";

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
  tab: string | null;
}

export interface Errors { name?: string; command?: string; tab?: string; size?: string }

export interface Chosen { shell: string | null; tab: string }

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
  tab: null,
});

const programOf = (command: string) => String(String(command.trim().split(/\s/)[0]).split("/").pop());

export const shellOf = (command: string) => {
  const name = programOf(command);

  return SHELLS.find((shell) => shell === name) ?? null;
};

export const sizeOf = (form: Form): Size =>
  SIZE_PRESETS.find((preset) => sizeLabel(preset) === form.size) ?? { cols: form.cols.trim(), rows: form.rows.trim() };

export const sessionOf = (form: Form) => (form.mode === "new" ? form.name.trim() : form.existing);

const heldIn = (form: Form, sessions: SessionInfo[]) => (tab: string) =>
  form.mode === "existing" && findTab(sessions, { session: form.existing, tab }) !== null;

export const tabField = (form: Form, sessions: SessionInfo[]) => {
  const program = programOf(form.command);

  return form.tab ?? (NAME.test(program) && !heldIn(form, sessions)(program) ? program : "");
};

export const tabOf = (form: Form, sessions: SessionInfo[]) => tabField(form, sessions).trim();

const cwdPart = (form: Form) => {
  const cwd = form.cwd.trim();

  return cwd && cwd !== "~" ? ` --cwd ${cwd}` : "";
};

const verbPart = (form: Form, chosen: Chosen) => {
  const tab = chosen.tab ? shellWord(chosen.tab) : "<tab>";

  return chosen.shell ? `shell --name ${tab} --shell ${chosen.shell}` : `open --name ${tab} ${form.command.trim() || "<program>"}`;
};

export const cliLine = (form: Form, chosen: Chosen) => {
  const session = sessionOf(form) ? shellWord(sessionOf(form)) : "<name>";
  const size = sizeOf(form);
  const created = form.mode === "new" ? `agent-terminal session new ${session} && ` : "";

  return `${created}agent-terminal --session ${session} ${verbPart(form, chosen)}${cwdPart(form)} --size ${size.cols || "?"}x${size.rows || "?"}`;
};

interface Range { low: number; high: number }

const inRange = (text: string, range: Range) => /^\d+$/.test(text) && Number(text) >= range.low && Number(text) <= range.high;

const nameRefusal = (form: Form, sessions: SessionInfo[]) => {
  const name = form.name.trim();

  if (!name) {
    return "Enter a session name.";
  }

  if (!NAME.test(name)) {
    return NAME_RULE;
  }

  return sessions.some((entry) => entry.session === name) ? `A session named ${name} already exists.` : undefined;
};

const tabRefusal = (form: Form, sessions: SessionInfo[]) => {
  const tab = tabOf(form, sessions);

  if (!tab) {
    return "Name the tab after the program or app it runs, e.g. api.";
  }

  if (!NAME.test(tab)) {
    return NAME_RULE;
  }

  return heldIn(form, sessions)(tab) ? `${form.existing} already has a tab named ${tab}.` : undefined;
};

const sizeFits = (form: Form) => inRange(form.cols.trim(), { low: 20, high: 500 }) && inRange(form.rows.trim(), { low: 5, high: 200 });

const commandRefusal = (form: Form) => (form.command.trim() ? undefined : "Enter a command.");

const sizeRefusal = (form: Form) => (form.size === "custom" && !sizeFits(form) ? "Columns 20–500, rows 5–200." : undefined);

export const refusals = (form: Form, sessions: SessionInfo[]): Errors => {
  const found = {
    name: form.mode === "new" ? nameRefusal(form, sessions) : undefined,
    command: commandRefusal(form),
    tab: tabRefusal(form, sessions),
    size: sizeRefusal(form),
  };

  return Object.fromEntries(Object.entries(found).filter(([, refusal]) => refusal !== undefined));
};
