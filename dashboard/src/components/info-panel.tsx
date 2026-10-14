import { ChevronRight, Lock } from "lucide-react";
import type { ReactNode } from "react";

import { dateTime } from "../lib/format";
import { useInfo, type Modes, type TabSetup } from "../lib/info";
import type { PanelProps } from "../panels";
import { PanelHead } from "./panel-parts";
import { cn } from "./ui";

type Row = [string, ReactNode];

const Muted = ({ children }: { children: ReactNode }) => <span className="text-muted-foreground">{children}</span>;

const onOff = (on: boolean) => (on ? "on" : <Muted>off</Muted>);

const List = ({ rows, mono = true }: { rows: Row[]; mono?: boolean }) => (
  <dl className="grid grid-cols-[minmax(7rem,auto)_minmax(0,1fr)] gap-x-4 gap-y-1.5 text-xs">
    {rows.map(([term, value]) => (
      <div key={term} className="contents">
        <dt className="text-muted-foreground">{term}</dt>
        <dd className={cn(mono && "font-mono", "break-all")}>{value}</dd>
      </div>
    ))}
  </dl>
);

const modeRows = (modes: Modes): Row[] => [
  ["alt-screen", onOff(modes.alt_screen)],
  ["mouse tracking", modes.mouse ? `on (${modes.mouse.encoding})` : <Muted>off</Muted>],
  ["bracketed paste", onOff(modes.bracketed_paste)],
  ["application cursor keys", onOff(modes.app_cursor)],
  ["focus events", onOff(modes.focus_events)],
  ["kitty keyboard flags", modes.keyboard.length ? modes.keyboard.join(", ") : <Muted>off</Muted>],
];

const shellIntegration = (setup: TabSetup) => (setup.shell_integration === null ? <Muted>not a shell</Muted> : onOff(setup.shell_integration));

const generalRows = (setup: TabSetup): Row[] => [
  ["program", setup.program],
  ["args", setup.args || <Muted>none</Muted>],
  [
    "pid",
    <>
      {setup.pid ?? <Muted>unknown</Muted>}
      {setup.exited && <Muted> (exited{setup.exit_code === null ? "" : ` ${setup.exit_code}`})</Muted>}
    </>,
  ],
  ["cwd", setup.cwd],
  ["TERM", setup.term ?? <Muted>none</Muted>],
  ["size", `${setup.cols} × ${setup.rows}`],
  ["shell integration", shellIntegration(setup)],
  ["recording", setup.recording ?? <Muted>off</Muted>],
  [
    "started",
    <time dateTime={setup.started} className="tabular-nums">
      {dateTime(setup.started)}
    </time>,
  ],
];

const envRows = (setup: TabSetup): Row[] =>
  setup.env.map(({ name, value }) => [
    name,
    value ?? (
      <>
        <span aria-hidden="true">••••</span>
        <span className="sr-only">masked</span> <Lock aria-hidden="true" className="inline size-[11px] text-muted-foreground" />
      </>
    ),
  ]);

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
