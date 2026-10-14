import { LoaderCircle } from "lucide-react";
import { Suspense, use } from "react";

import { runStyle, styledLines } from "../lib/ansi";
import { outputOf, rowOf, useCommands, useOpenCommand, type LoadedOutput } from "../lib/commands";
import { clockTime, formatDuration, shellWord, shortPath } from "../lib/format";
import type { CommandRecord } from "../lib/protocol";
import { tabName } from "../lib/tabs";
import type { PanelProps } from "../panels";
import { EmptyNote, PanelHead } from "./panel-parts";
import { CliBlock, cn } from "./ui";

const Mark = ({ command }: { command: CommandRecord }) => {
  if (command.ended_at_ms === null)
    return (
      <span className="w-12 shrink-0 font-mono text-muted-foreground">
        <LoaderCircle aria-hidden="true" className="inline size-3 animate-spin" />
        <span className="sr-only">running</span>
      </span>
    );
  const code = command.exit_code;
  return (
    <span className={cn("w-12 shrink-0 font-mono", code === 0 ? "text-green-600 dark:text-success" : code === null ? "text-muted-foreground" : "text-destructive")}>
      <span aria-hidden="true">{code === 0 ? "✓" : code === null ? "?" : `✗ ${code}`}</span>
      <span className="sr-only">exit {code ?? "unknown"}</span>
    </span>
  );
};

const Output = ({ loading }: { loading: Promise<LoadedOutput> }) => {
  const loaded = use(loading);
  if ("error" in loaded) return <span className="text-destructive">{loaded.error}</span>;
  if (!loaded.output) return <span className="text-muted-foreground">No output</span>;
  return styledLines(loaded.output).map((line, n) => (
    <div key={n} className="min-h-[1lh] whitespace-pre-wrap">
      {line.map((run, m) => (
        <span key={m} style={runStyle(run.style)}>
          {run.text}
        </span>
      ))}
    </div>
  ));
};

type CommandRowProps = { session: string; tab: string; command: CommandRecord; home: string | null; open: boolean; onToggle: () => void };

const CommandRow = ({ session, tab, command, home, open, onToggle }: CommandRowProps) => {
  const failed = command.exit_code !== null && command.exit_code !== 0;
  return (
    <li className={cn("border-b border-border", failed && "bg-destructive/5", open && "bg-muted/60")}>
      <div className="px-3">
        <button
          type="button"
          aria-expanded={open}
          onClick={onToggle}
          className="flex w-full items-start gap-2 rounded-sm py-1.5 text-left text-xs outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
        >
          <Mark command={command} />
          <span className="min-w-0 flex-1">
            <span className="block font-mono [overflow-wrap:anywhere]">{command.command}</span>
            <span className="mt-0.5 block font-mono text-[10px] break-all text-muted-foreground">
              {shortPath(command.cwd, home)} · {command.ended_at_ms === null ? "running" : formatDuration(command.duration_ms)} ·{" "}
              <time dateTime={new Date(command.started_at_ms).toISOString()} className="tabular-nums">
                {clockTime(command.started_at_ms)}
              </time>
            </span>
          </span>
          <span className="ml-auto shrink-0 font-mono text-muted-foreground" aria-hidden="true">
            {open ? "-" : "+"}
          </span>
        </button>
        {open && (
          <div className="mb-1.5 max-h-64 overflow-auto rounded-md bg-muted/50 px-2 py-1.5 font-mono text-[10px] leading-snug">
            <Suspense>
              <Output loading={outputOf(session, tab, command)} />
            </Suspense>
          </div>
        )}
      </div>
    </li>
  );
};

export const CommandsPanel = ({ session, tab }: PanelProps) => {
  const feed = useCommands(session, tab.tab);
  const { open, toggle } = useOpenCommand(session, tab.tab);
  if (!feed) return <PanelHead title="Commands" />;
  if (!feed.shell)
    return (
      <>
        <PanelHead title="Commands" />
        <EmptyNote title={`${tab.program} is not a shell`}>
          Commands come from shell tabs, opened with <CliBlock line="agent-terminal shell" />
        </EmptyNote>
      </>
    );
  if (!feed.integration)
    return (
      <>
        <PanelHead title="Commands" />
        <EmptyNote title="Shell integration is off for this tab">
          Commands are recorded in shells opened with integration: <CliBlock line={`agent-terminal --session ${shellWord(session)} shell`} />
        </EmptyNote>
      </>
    );
  if (!feed.commands.length)
    return (
      <>
        <PanelHead title="Commands" count={0} />
        <EmptyNote title="No commands yet" />
      </>
    );
  return (
    <>
      <PanelHead title="Commands" count={feed.commands.length} />
      <ol aria-label={`Commands in ${tabName(session, tab.tab)}`} className="min-h-0 flex-1 overflow-y-auto">
        {feed.commands.map((command) => {
          const row = rowOf(command);
          return (
            <CommandRow
              key={row}
              session={session}
              tab={tab.tab}
              command={command}
              home={feed.home}
              open={open === row}
              onToggle={() => toggle(row)}
            />
          );
        })}
      </ol>
    </>
  );
};
