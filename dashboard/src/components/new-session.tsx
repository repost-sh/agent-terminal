import { Copy } from "lucide-react";
import { useId, useState } from "react";

import { openTab, reason } from "../lib/api";
import type { SessionInfo } from "../lib/protocol";
import { navigate, tabRoute } from "../lib/route";
import { closeDialog, Dialog } from "./dialog";
import { toast } from "./toaster";
import { Button, cn, copy, IconButton } from "./ui";

const SIZE_PRESETS = ["80x24", "120x40", "200x60"];
const SHELLS = ["zsh", "bash", "fish"];
const NAME = /^[A-Za-z0-9_-]+$/;

const LABEL = "text-xs font-medium text-muted-foreground";
const INPUT =
  "w-full rounded-md border border-border bg-transparent px-3 py-2 text-sm outline-none focus:ring-2 focus:ring-ring placeholder:text-muted-foreground/60 aria-invalid:border-destructive aria-invalid:focus:ring-destructive/40";

type Mode = "new" | "existing";

type Form = {
  mode: Mode;
  name: string;
  existing: string;
  command: string;
  cwd: string;
  size: string;
  cols: string;
  rows: string;
  integration: boolean | null;
};

type Errors = { name?: string; command?: string; size?: string };

const freeName = (sessions: SessionInfo[]) => {
  let n = sessions.length + 1;
  while (sessions.some((entry) => entry.session === `session-${n}`)) n += 1;
  return `session-${n}`;
};

const shellOf = (command: string) => {
  const name = command.trim().split(/\s+/)[0]?.split("/").pop() ?? "";
  return SHELLS.includes(name) ? name : null;
};

const sizeOf = (form: Form) => {
  const [cols = "", rows = ""] = form.size === "custom" ? [form.cols.trim(), form.rows.trim()] : form.size.split("x");
  return { cols, rows };
};

const sessionOf = (form: Form) => (form.mode === "new" ? form.name.trim() : form.existing);

const cliLine = (form: Form, shell: string | null) => {
  const session = sessionOf(form) || "<name>";
  const { cols, rows } = sizeOf(form);
  const cwd = form.cwd.trim() && form.cwd.trim() !== "~" ? ` --cwd ${form.cwd.trim()}` : "";
  const verb = shell ? `shell --shell ${shell}` : `open ${form.command.trim() || "<program>"}`;
  return `agent-terminal --session ${session} ${verb}${cwd} --size ${cols || "?"}x${rows || "?"}`;
};

const inRange = (text: string, low: number, high: number) => /^\d+$/.test(text) && Number(text) >= low && Number(text) <= high;

const refusals = (form: Form, sessions: SessionInfo[]): Errors => {
  const errors: Errors = {};
  const name = form.name.trim();
  if (form.mode === "new") {
    if (!name) errors.name = "Enter a session name.";
    else if (!NAME.test(name)) errors.name = "Use letters, numbers, - and _.";
    else if (sessions.some((entry) => entry.session === name)) errors.name = `A session named ${name} already exists.`;
  }
  if (!form.command.trim()) errors.command = "Enter a command.";
  if (form.size === "custom" && !(inRange(form.cols.trim(), 20, 500) && inRange(form.rows.trim(), 5, 200))) {
    errors.size = "Columns 20–500, rows 5–200.";
  }
  return errors;
};

const FieldError = ({ id, text }: { id: string; text: string | undefined }) =>
  text ? (
    <p id={id} role="alert" className="rounded-md bg-destructive/10 px-3 py-2 text-xs text-destructive">
      {text}
    </p>
  ) : null;

type TextFieldProps = {
  id: string;
  label: string;
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  error?: string;
  autoFocus?: boolean;
};

const TextField = ({ id, label, value, onChange, placeholder, error, autoFocus = false }: TextFieldProps) => (
  <div className="grid gap-1.5">
    <label htmlFor={id} className={LABEL}>
      {label}
    </label>
    <input
      id={id}
      type="text"
      className={cn(INPUT, "font-mono text-[13px]")}
      value={value}
      placeholder={placeholder}
      spellCheck={false}
      autoComplete="off"
      aria-invalid={error ? true : undefined}
      aria-describedby={error ? `${id}-error` : undefined}
      data-autofocus={autoFocus || undefined}
      onChange={(event) => onChange(event.target.value)}
    />
    <FieldError id={`${id}-error`} text={error} />
  </div>
);

type SegmentedProps = {
  id: string;
  legend: string;
  value: string;
  options: [string, string][];
  onChange: (value: string) => void;
  error?: string;
};

const Segmented = ({ id, legend, value, options, onChange, error }: SegmentedProps) => (
  <fieldset className="grid gap-1.5" aria-describedby={error ? `${id}-error` : undefined}>
    <legend className={cn(LABEL, "mb-1.5")}>{legend}</legend>
    <div className="grid gap-2" style={{ gridTemplateColumns: `repeat(${options.length}, minmax(0, 1fr))` }}>
      {options.map(([option, label]) => (
        <label
          key={option}
          className="flex min-h-8 cursor-pointer items-center justify-center rounded-md border border-border px-2 py-1.5 text-center text-sm text-muted-foreground transition-colors hover:text-foreground has-checked:border-ring has-checked:bg-muted has-checked:text-foreground has-focus-visible:ring-2 has-focus-visible:ring-ring/50"
        >
          <input
            type="radio"
            className="sr-only"
            id={`${id}-${option}`}
            name={id}
            value={option}
            checked={value === option}
            onChange={() => onChange(option)}
          />
          {label}
        </label>
      ))}
    </div>
    <FieldError id={`${id}-error`} text={error} />
  </fieldset>
);

type NewSessionDialogProps = { sessions: SessionInfo[]; existing: string | null };

export const NewSessionDialog = ({ sessions, existing }: NewSessionDialogProps) => {
  const id = useId();
  const [form, setForm] = useState<Form>(() => ({
    mode: existing ? "existing" : "new",
    name: freeName(sessions),
    existing: existing ?? sessions[0]?.session ?? "",
    command: "",
    cwd: "~",
    size: "120x40",
    cols: "120",
    rows: "40",
    integration: null,
  }));
  const [errors, setErrors] = useState<Errors>({});
  const [opening, setOpening] = useState(false);
  const set = (change: Partial<Form>) => setForm((current) => ({ ...current, ...change }));

  const shell = shellOf(form.command);
  const integration = shell !== null && (form.integration ?? true);
  const line = cliLine(form, integration ? shell : null);
  const fieldId = (field: string) => `${id}-${field}`;
  const focus = (field: string) => document.getElementById(fieldId(field))?.focus({ preventScroll: true });

  const submit = async () => {
    const found = refusals(form, sessions);
    setErrors(found);
    if (found.name) return focus("name");
    if (found.command) return focus("command");
    if (found.size) return focus(`size-${form.size}`);
    const { cols, rows } = sizeOf(form);
    setOpening(true);
    try {
      const opened = await openTab({
        session: sessionOf(form),
        new_session: form.mode === "new",
        command: form.command,
        cwd: form.cwd,
        cols: Number(cols),
        rows: Number(rows),
        shell_integration: integration,
      });
      closeDialog(document.getElementById("main-content"));
      navigate(tabRoute(opened.session, opened.tab));
    } catch (error) {
      toast("error", reason(error));
      focus("command");
    } finally {
      setOpening(false);
    }
  };

  const sizes: [string, string][] = [...SIZE_PRESETS.map((preset): [string, string] => [preset, preset.replace("x", "×")]), ["custom", "Custom"]];

  return (
    <Dialog
      title={form.mode === "new" ? "New session" : "New tab"}
      width={28}
      onSubmit={() => void submit()}
      footer={
        <>
          <Button variant="ghost" onClick={() => closeDialog()}>
            Cancel
          </Button>
          <Button type="submit" disabled={opening}>
            {form.mode === "new" ? "Create" : "Open tab"}
          </Button>
        </>
      }
    >
      {sessions.length > 0 && (
        <Segmented
          id={fieldId("mode")}
          legend="Open in"
          value={form.mode}
          options={[
            ["new", "New session"],
            ["existing", "Existing session"],
          ]}
          onChange={(mode) => set({ mode: mode as Mode })}
        />
      )}
      {form.mode === "new" ? (
        <TextField id={fieldId("name")} label="Session name" value={form.name} onChange={(name) => set({ name })} error={errors.name} autoFocus />
      ) : (
        <div className="grid gap-1.5">
          <label htmlFor={fieldId("existing")} className={LABEL}>
            Session
          </label>
          <select
            id={fieldId("existing")}
            className={cn(INPUT, "font-mono text-[13px]")}
            value={form.existing}
            onChange={(event) => set({ existing: event.target.value })}
          >
            {sessions.map((entry) => (
              <option key={entry.session} value={entry.session}>
                {entry.session}
              </option>
            ))}
          </select>
        </div>
      )}
      <TextField
        id={fieldId("command")}
        label="Command"
        value={form.command}
        placeholder="lazygit"
        onChange={(command) => set({ command })}
        error={errors.command}
        autoFocus={form.mode === "existing"}
      />
      <TextField id={fieldId("cwd")} label="Working directory" value={form.cwd} placeholder="~" onChange={(cwd) => set({ cwd })} />
      <Segmented id={fieldId("size")} legend="Size" value={form.size} options={sizes} onChange={(size) => set({ size })} error={errors.size} />
      {form.size === "custom" && (
        <div className="grid grid-cols-2 gap-2">
          <TextField id={fieldId("cols")} label="Columns" value={form.cols} onChange={(cols) => set({ cols })} />
          <TextField id={fieldId("rows")} label="Rows" value={form.rows} onChange={(rows) => set({ rows })} />
        </div>
      )}
      <div className="flex items-center justify-between gap-3">
        <div>
          <p id={fieldId("integration-label")} className="text-sm">
            Shell integration
          </p>
          <p id={fieldId("integration-note")} className="text-xs text-muted-foreground">
            {shell ? "Records each command with its exit code" : "Shells only"}
          </p>
        </div>
        <button
          type="button"
          role="switch"
          aria-checked={integration}
          aria-labelledby={fieldId("integration-label")}
          aria-describedby={fieldId("integration-note")}
          disabled={!shell}
          onClick={() => set({ integration: !integration })}
          className={cn(
            "relative inline-flex h-6 w-10 shrink-0 items-center rounded-full border border-transparent transition-colors outline-none focus-visible:ring-2 focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:opacity-50",
            integration ? "bg-primary" : "bg-input",
          )}
        >
          <span className={cn("block size-5 rounded-full bg-background shadow transition-transform", integration ? "translate-x-4" : "translate-x-0.5")} />
        </button>
      </div>
      <div className="grid gap-1.5">
        <p id={fieldId("cli")} className={LABEL}>
          Same as
        </p>
        <div className="flex items-start gap-2 rounded-md bg-muted px-2.5 py-1.5">
          <code aria-labelledby={fieldId("cli")} className="min-w-0 flex-1 font-mono text-xs leading-6 break-all">
            {line}
          </code>
          <IconButton label="Copy the command line" tooltip="Copy" icon={Copy} className="size-6 p-0 [&_svg]:size-[13px]" onClick={() => void copy(line)} />
        </div>
      </div>
    </Dialog>
  );
};
