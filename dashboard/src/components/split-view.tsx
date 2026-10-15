import { Maximize2, X } from "lucide-react";
import { useState } from "react";
import { createPortal } from "react-dom";

import { focusNext, focusWhenShown } from "../lib/focus";
import type { SessionInfo, TabInfo } from "../lib/protocol";
import { followLink, navigate, splitRoute, tabRoute, type Cell } from "../lib/route";
import { isShown, resizeCells, shownCells, withCell, type Layout } from "../lib/split";
import { useTabStream } from "../lib/stream";
import { tabName, tabState } from "../lib/tabs";
import { TabChip, TabGlyph } from "./sessions-sidebar";
import { StatusBar } from "./status-bar";
import { Viewport } from "./terminal-view";
import { Toolbar } from "./toolbar";
import { Chip, IconButton, IconLink, cn } from "./ui";

const focusMain = () => document.getElementById("main-content")?.focus({ preventScroll: true });

type SplitViewProps = { cells: Cell[]; sessions: SessionInfo[]; focus: number; onFocus: (at: number) => void };

export const SplitView = ({ cells, sessions, focus, onFocus }: SplitViewProps) => {
  const [statusSlot, setStatusSlot] = useState<HTMLDivElement | null>(null);
  const [toolbarSlot, setToolbarSlot] = useState<HTMLDivElement | null>(null);
  const shown = shownCells(sessions, cells);
  const focused = shown[focus];
  const count = cells.length === 4 ? 4 : 2;

  const layout = (next: Layout) => {
    if (next === 1) {
      navigate(focused ? tabRoute(focused.cell.session, focused.cell.tab) : "/");
      return;
    }
    if (focus >= next) onFocus(0);
    navigate(splitRoute(resizeCells(cells, next)));
  };

  const remove = (at: number) => {
    focusMain();
    navigate(splitRoute(withCell(cells, at, null)));
  };

  const pick = (at: number, cell: Cell) => {
    onFocus(at);
    focusNext(`cell-${at}`);
    navigate(splitRoute(withCell(cells, at, cell)));
  };

  return (
    <>
      {focused ? <div ref={setToolbarSlot} className="flex shrink-0 flex-col" /> : <Toolbar layout={count} onLayout={layout} />}
      <div className="h-px shrink-0 bg-border" role="none" />
      <div
        className="split-grid grid min-h-0 flex-1 gap-px bg-border"
        style={{ gridTemplateColumns: "repeat(2, minmax(0, 1fr))", gridTemplateRows: count === 4 ? "repeat(2, minmax(0, 1fr))" : "minmax(0, 1fr)" }}
      >
        {shown.map((entry, at) =>
          entry ? (
            <SplitCell
              key={`${at}:${entry.cell.session}/${entry.cell.tab}`}
              at={at}
              session={entry.cell.session}
              tab={entry.tab}
              focused={at === focus}
              statusSlot={at === focus ? statusSlot : null}
              toolbarSlot={at === focus ? toolbarSlot : null}
              layout={count}
              onLayout={layout}
              onFocus={() => onFocus(at)}
              onRemove={() => remove(at)}
            />
          ) : (
            <Picker key={`${at}:picker`} at={at} cells={cells} sessions={sessions} onPick={(cell) => pick(at, cell)} />
          ),
        )}
      </div>
      {focused && (
        <>
          <div className="h-px shrink-0 bg-border" role="none" />
          <div ref={setStatusSlot} className="flex shrink-0 flex-col" />
        </>
      )}
    </>
  );
};

type SplitCellProps = {
  at: number;
  session: string;
  tab: TabInfo;
  focused: boolean;
  statusSlot: HTMLElement | null;
  toolbarSlot: HTMLElement | null;
  layout: Layout;
  onLayout: (count: Layout) => void;
  onFocus: () => void;
  onRemove: () => void;
};

const SplitCell = ({ at, session, tab, focused, statusSlot, toolbarSlot, layout, onLayout, onFocus, onRemove }: SplitCellProps) => {
  const { state, shown, paint } = useTabStream(session, tab.tab);
  const name = tabName(session, tab.tab);
  const alone = tabRoute(session, tab.tab);

  return (
    <>
      {toolbarSlot && createPortal(<Toolbar layout={layout} onLayout={onLayout} current={{ session, tab, stream: state, shown }} />, toolbarSlot)}
      <section
        aria-label={`${name} ${tab.program}`}
        onClick={onFocus}
        className={cn("flex min-h-0 min-w-0 flex-col bg-background", focused && "ring-1 ring-foreground/40 ring-inset")}
      >
        <div className="flex shrink-0 items-center gap-1 border-b border-border px-1.5 py-0.5 text-[11px]">
          <button
            type="button"
            ref={focusWhenShown(`cell-${at}`)}
            aria-pressed={focused}
            className="flex min-w-0 flex-1 flex-wrap items-center gap-x-1.5 rounded px-1 py-1 text-left outline-none hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring/50"
          >
            <TabGlyph tab={tab} />
            <span className="font-mono font-semibold">{session}</span>
            <span className="text-muted-foreground" aria-hidden="true">
              /
            </span>
            <span className="font-mono">{tab.tab}</span>
            <span className="break-all text-muted-foreground">{tab.program}</span>
            <TabChip tab={tab} />
            <span className="sr-only">, {tabState(tab).label}</span>
          </button>
          <IconLink
            href={alone}
            label={`Maximize ${name}`}
            tooltip="Maximize"
            icon={Maximize2}
            className="[&_svg]:size-3.5"
            onClick={(event) => {
              focusMain();
              followLink(event, alone);
            }}
          />
          <IconButton
            label={`Remove ${name} from the grid`}
            tooltip="Remove"
            icon={X}
            onClick={(event) => {
              event.stopPropagation();
              onRemove();
            }}
          />
        </div>
        <Viewport compact paint={paint}state={state} shown={shown} program={tab.program} label={`${name} ${tab.program}, live frame ${shown?.frame ?? 0}`} />
        {statusSlot && createPortal(<StatusBar session={session} tab={tab} state={state} shown={shown} />, statusSlot)}
      </section>
    </>
  );
};

type PickerProps = { at: number; cells: Cell[]; sessions: SessionInfo[]; onPick: (cell: Cell) => void };

const Picker = ({ at, cells, sessions, onPick }: PickerProps) => (
  <section aria-labelledby={`pick-${at}`} className="flex min-h-0 min-w-0 flex-col items-center justify-center gap-2 overflow-y-auto bg-background p-4">
    <h2 id={`pick-${at}`} className="text-xs font-normal text-muted-foreground">
      Show a terminal
    </h2>
    <ul className="w-full max-w-64 overflow-hidden rounded-md border border-border text-xs">
      {sessions.map((info) => (
        <li key={info.session}>
          <p className="bg-muted/50 px-2.5 py-1 font-mono text-[11px] font-semibold text-muted-foreground">{info.session}</p>
          <ul>
            {info.tabs.map((tab) => {
              const cell = { session: info.session, tab: tab.tab };
              const already = isShown(cells, cell);
              return (
                <li key={tab.tab}>
                  <button
                    type="button"
                    disabled={already}
                    onClick={() => onPick(cell)}
                    className="flex min-h-7 w-full items-center gap-2 px-2.5 py-1 text-left outline-none hover:bg-muted focus-visible:bg-muted disabled:opacity-50"
                  >
                    <TabGlyph tab={tab} />
                    <span className="font-mono">{tab.tab}</span>
                    <span className="min-w-0 flex-1 break-all text-muted-foreground">{tab.program}</span>
                    {already ? <Chip tone="muted">shown</Chip> : <TabChip tab={tab} />}
                    <span className="sr-only">, {tabName(info.session, tab.tab)}</span>
                  </button>
                </li>
              );
            })}
          </ul>
        </li>
      ))}
    </ul>
  </section>
);
