import { Copy } from "lucide-react";
import { useId, useState } from "react";

import { openTab, reason } from "../lib/api";
import { cliLine, initialForm, refusals, SIZE_PRESETS, sessionOf, shellOf, sizeOf, type Errors, type Form, type Mode } from "../lib/new-session-form";
import type { SessionInfo } from "../lib/protocol";
import { navigate, tabRoute } from "../lib/route";
import { settle } from "../lib/settle";
import { showPanel } from "../lib/side-panel";
import { announceWhenLive } from "../lib/stream";
import { closeDialog, Dialog } from "./dialog";
import { INPUT, LABEL, Segmented, TextField } from "./form-fields";
import { toast } from "./toaster";
import { Button, cn, copy, IconButton } from "./ui";

interface NewSessionDialogProps { sessions: SessionInfo[]; existing: string | null }

const useNewSession = (props: NewSessionDialogProps) => {
  const id = useId();
  const [form, setForm] = useState<Form>(() => initialForm(props.sessions, props.existing));
  const [errors, setErrors] = useState<Errors>({});
  const [opening, setOpening] = useState(false);
  const shell = shellOf(form.command);

  return {
    form,
    set: (change: Partial<Form>) => {
      setForm((current) => ({ ...current, ...change }));
    },
    errors,
    setErrors,
    opening,
    setOpening,
    shell,
    integration: shell !== null && (form.integration ?? true),
    fieldId: (field: string) => `${id}-${field}`,
  };
};

type NewSession = ReturnType<typeof useNewSession>;

interface DialogPartProps { dialog: NewSession }

const focusField = (dialog: NewSession, field: string) => document.getElementById(dialog.fieldId(field))?.focus({ preventScroll: true });

const firstRefused = (found: Errors, form: Form) => {
  if (found.name) {
    return "name";
  }

  if (found.command) {
    return "command";
  }

  return found.size ? `size-${form.size}` : null;
};

const openFrom = async (dialog: NewSession) => {
  const size = sizeOf(dialog.form);
  const tab = await openTab({
    session: sessionOf(dialog.form),
    new_session: dialog.form.mode === "new",
    command: dialog.form.command,
    cwd: dialog.form.cwd,
    cols: Number(size.cols),
    rows: Number(size.rows),
    shell_integration: dialog.integration,
  });

  announceWhenLive(tab.session, tab.tab);
  showPanel("activity");
  closeDialog(document.getElementById("main-content"));
  navigate(tabRoute(tab.session, tab.tab));
};

const submitForm = async (dialog: NewSession, sessions: SessionInfo[]) => {
  const found = refusals(dialog.form, sessions);
  const refused = firstRefused(found, dialog.form);

  dialog.setErrors(found);
  if (refused) {
    focusField(dialog, refused);

    return;
  }

  dialog.setOpening(true);
  const opened = await settle(() => openFrom(dialog));

  if (!opened.ok) {
    toast("error", { title: reason(opened.error) });
    focusField(dialog, "command");
  }

  dialog.setOpening(false);
};

interface SessionFieldsProps { dialog: NewSession; sessions: SessionInfo[] }

const ExistingSession = ({ dialog, sessions }: SessionFieldsProps) => (
  <div className="grid gap-1.5">
    <label htmlFor={dialog.fieldId("existing")} className={LABEL}>
      Session
    </label>
    <select
      id={dialog.fieldId("existing")}
      className={cn(INPUT, "font-mono text-[13px]")}
      value={dialog.form.existing}
      onChange={(event) => {
        dialog.set({ existing: event.target.value });
      }}
    >
      {sessions.map((entry) => (
        <option key={entry.session} value={entry.session}>
          {entry.session}
        </option>
      ))}
    </select>
  </div>
);

const SessionFields = ({ dialog, sessions }: SessionFieldsProps) => (
  <>
    {sessions.length > 0 && (
      <Segmented
        id={dialog.fieldId("mode")}
        legend="Open in"
        value={dialog.form.mode}
        options={[
          ["new", "New session"],
          ["existing", "Existing session"],
        ]}
        onChange={(mode) => {
          dialog.set({ mode: mode as Mode });
        }}
      />
    )}
    {dialog.form.mode === "new" ? (
      <TextField id={dialog.fieldId("name")} label="Session name" value={dialog.form.name} onChange={(name) => {
        dialog.set({ name });
      }} error={dialog.errors.name} autoFocus />
    ) : (
      <ExistingSession dialog={dialog} sessions={sessions} />
    )}
  </>
);

const SIZES: [string, string][] = [...SIZE_PRESETS.map((preset): [string, string] => [preset, preset.replace("x", "×")]), ["custom", "Custom"]];

const SizeFields = ({ dialog }: DialogPartProps) => (
  <>
    <Segmented id={dialog.fieldId("size")} legend="Size" value={dialog.form.size} options={SIZES} onChange={(size) => {
      dialog.set({ size });
    }} error={dialog.errors.size} />
    {dialog.form.size === "custom" && (
      <div className="grid grid-cols-2 gap-2">
        <TextField id={dialog.fieldId("cols")} label="Columns" value={dialog.form.cols} onChange={(cols) => {
          dialog.set({ cols });
        }} />
        <TextField id={dialog.fieldId("rows")} label="Rows" value={dialog.form.rows} onChange={(rows) => {
          dialog.set({ rows });
        }} />
      </div>
    )}
  </>
);

const IntegrationSwitch = ({ dialog }: DialogPartProps) => (
  <div className="flex items-center justify-between gap-3">
    <div>
      <p id={dialog.fieldId("integration-label")} className="text-sm">
        Shell integration
      </p>
      <p id={dialog.fieldId("integration-note")} className="text-xs text-muted-foreground">
        {dialog.shell ? "Records each command with its exit code" : "Shells only"}
      </p>
    </div>
    <button
      type="button"
      role="switch"
      aria-checked={dialog.integration}
      aria-labelledby={dialog.fieldId("integration-label")}
      aria-describedby={dialog.fieldId("integration-note")}
      disabled={!dialog.shell}
      onClick={() => {
        dialog.set({ integration: !dialog.integration });
      }}
      className={cn(
        "relative inline-flex h-6 w-10 shrink-0 items-center rounded-full border border-transparent transition-colors outline-none focus-visible:ring-2 focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:opacity-50",
        dialog.integration ? "bg-primary" : "bg-input",
      )}
    >
      <span className={cn("block size-5 rounded-full bg-background shadow transition-transform", dialog.integration ? "translate-x-4" : "translate-x-0.5")} />
    </button>
  </div>
);

const CliPreview = ({ dialog }: DialogPartProps) => {
  const line = cliLine(dialog.form, dialog.integration ? dialog.shell : null);

  return (
    <div className="grid gap-1.5">
      <p id={dialog.fieldId("cli")} className={LABEL}>
        Same as
      </p>
      <div className="flex items-start gap-2 rounded-md bg-muted px-2.5 py-1.5">
        <code aria-labelledby={dialog.fieldId("cli")} className="min-w-0 flex-1 font-mono text-xs leading-6 break-all">
          {line}
        </code>
        <IconButton look={{ label: "Copy the command line", tooltip: "Copy", icon: Copy }} className="size-6 p-0 [&_svg]:size-[13px]" onClick={() => {
          void copy(line);
        }} />
      </div>
    </div>
  );
};

const Footer = ({ dialog }: DialogPartProps) => (
  <>
    <Button look={{ variant: "ghost" }} onClick={() => {
      closeDialog();
    }}>
      Cancel
    </Button>
    <Button type="submit" disabled={dialog.opening}>
      {dialog.form.mode === "new" ? "Create" : "Open tab"}
    </Button>
  </>
);

export const NewSessionDialog = (props: NewSessionDialogProps) => {
  const dialog = useNewSession(props);
  const { form } = dialog;

  return (
    <Dialog title={form.mode === "new" ? "New session" : "New tab"} width={28} onSubmit={() => void submitForm(dialog, props.sessions)} footer={<Footer dialog={dialog} />}>
      <SessionFields dialog={dialog} sessions={props.sessions} />
      <TextField
        id={dialog.fieldId("command")}
        label="Command"
        value={form.command}
        placeholder="lazygit"
        onChange={(command) => {
          dialog.set({ command });
        }}
        error={dialog.errors.command}
        autoFocus={form.mode === "existing"}
      />
      <TextField id={dialog.fieldId("cwd")} label="Working directory" value={form.cwd} placeholder="~" onChange={(cwd) => {
        dialog.set({ cwd });
      }} />
      <SizeFields dialog={dialog} />
      <IntegrationSwitch dialog={dialog} />
      <CliPreview dialog={dialog} />
    </Dialog>
  );
};
