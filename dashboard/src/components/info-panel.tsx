import { ChevronRight, Lock } from "lucide-react";
import type { PropsWithChildren, ReactNode } from "react";

import { dateTime } from "../lib/format";
import { useInfo, type Modes, type TabSetup } from "../lib/info";
import type { PanelProps } from "../panels";
import { PanelHead } from "./panel-parts";
import { cn } from "./ui";

interface Row { term: string; value: ReactNode }

const Muted = ({ children }: PropsWithChildren) => <span className="text-muted-foreground">{children}</span>;

const onOff = (on: boolean) => (on ? "on" : <Muted>off</Muted>);

interface ListProps { rows: Row[]; mono?: boolean }

const List = ({ rows, mono = true }: ListProps) => (
  <dl className="grid grid-cols-[minmax(7rem,auto)_minmax(0,1fr)] gap-x-4 gap-y-1.5 text-xs">
    {rows.map(({ term, value }) => (
      <div key={term} className="contents">
        <dt className="text-muted-foreground">{term}</dt>
        <dd className={cn(mono && "font-mono", "break-all")}>{value}</dd>
      </div>
    ))}
  </dl>
);

const modeRows = (modes: Modes): Row[] => [
  { term: "alt-screen", value: onOff(modes.alt_screen) },
  { term: "mouse tracking", value: modes.mouse ? `on (${modes.mouse.encoding})` : <Muted>off</Muted> },
  { term: "bracketed paste", value: onOff(modes.bracketed_paste) },
  { term: "application cursor keys", value: onOff(modes.app_cursor) },
  { term: "focus events", value: onOff(modes.focus_events) },
  { term: "kitty keyboard flags", value: modes.keyboard.length ? modes.keyboard.join(", ") : <Muted>off</Muted> },
];

const shellIntegration = (setup: TabSetup) => (setup.shell_integration === null ? <Muted>not a shell</Muted> : onOff(setup.shell_integration));

interface SetupProps { setup: TabSetup }

const Pid = ({ setup }: SetupProps) => (
  <>
    {setup.pid ?? <Muted>unknown</Muted>}
    {setup.exited && <Muted> (exited{setup.exit_code === null ? "" : ` ${String(setup.exit_code)}`})</Muted>}
  </>
);

interface StartedProps { at: string }

const Started = ({ at }: StartedProps) => (
  <time dateTime={at} className="tabular-nums">
    {dateTime(at)}
  </time>
);

const Masked = () => (
  <>
    <span aria-hidden="true">••••</span>
    <span className="sr-only">masked</span> <Lock aria-hidden="true" className="inline size-[11px] text-muted-foreground" />
  </>
);

const generalRows = (setup: TabSetup): Row[] => [
  { term: "program", value: setup.program },
  { term: "args", value: setup.args || <Muted>none</Muted> },
  { term: "pid", value: <Pid setup={setup} /> },
  { term: "cwd", value: setup.cwd },
  { term: "TERM", value: setup.term ?? <Muted>none</Muted> },
  { term: "size", value: `${String(setup.cols)} × ${String(setup.rows)}` },
  { term: "shell integration", value: shellIntegration(setup) },
  { term: "started", value: <Started at={setup.started} /> },
];

const envRows = (setup: TabSetup): Row[] => setup.env.map(({ name, value }) => ({ term: name, value: value ?? <Masked /> }));

export const InfoPanel = ({ session, tab }: PanelProps) => {
  const setup = useInfo(session, tab.tab);

  return (
    <>
      <PanelHead title="Info" />
      {setup && (
        <div className="min-h-0 flex-1 space-y-4 overflow-y-auto px-3 py-3">
          <List rows={generalRows(setup)} />
          <section aria-labelledby="modes-title">
            <h3 id="modes-title" className="mb-1.5 text-[11px] font-medium text-muted-foreground">
              Modes
            </h3>
            <List rows={modeRows(setup.modes)} />
          </section>
          <details className="group">
            <summary className="flex h-6 items-center gap-1 text-[11px] font-medium text-muted-foreground outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring/50">
              <ChevronRight aria-hidden="true" className="size-3 transition-transform group-open:rotate-90" />
              Environment <span className="font-normal">({setup.env.length})</span>
            </summary>
            <div className="mt-1.5">
              <List rows={envRows(setup)} />
            </div>
          </details>
        </div>
      )}
    </>
  );
};
