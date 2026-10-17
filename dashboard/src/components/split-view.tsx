import { Maximize2, X } from "lucide-react";
import { useState, type ReactNode } from "react";
import { createPortal } from "react-dom";

import { focusNext, focusWhenShown } from "../lib/focus";
import type { SessionInfo, TabInfo } from "../lib/protocol";
import { followLink, navigate, splitRoute, tabRoute, type Cell } from "../lib/route";
import { isShown, resizeCells, shownCells, withCell, type Layout, type ShownCell } from "../lib/split";
import { tabName, tabState } from "../lib/tabs";
import { TabChip, TabGlyph } from "./sessions-sidebar";
import { StatusBar } from "./status-bar";
import { useHeldViewport, Viewport } from "./terminal-view";
import { Toolbar } from "./toolbar";
import { Chip, IconButton, IconLink, cn } from "./ui";

const focusMain = () => document.getElementById("main-content")?.focus({ preventScroll: true });

interface SplitViewProps { cells: Cell[]; sessions: SessionInfo[]; focus: number; onFocus: (at: number) => void }

interface Slots { status: HTMLDivElement | null; toolbar: HTMLDivElement | null }

const splitActions = (props: SplitViewProps, focused: ShownCell | undefined) => ({
  layout: (next: Layout) => {
    if (next === 1) {
      navigate(focused ? tabRoute(focused.cell.session, focused.cell.tab) : "/");

      return;
    }

    if (props.focus >= next) {
      props.onFocus(0);
    }

    navigate(splitRoute(resizeCells(props.cells, next)));
  },
  remove: (at: number) => {
    focusMain();
    navigate(splitRoute(withCell(props.cells, { at, cell: null })));
  },
  pick: (at: number, cell: Cell) => {
    props.onFocus(at);
    focusNext(`cell-${String(at)}`);
    navigate(splitRoute(withCell(props.cells, { at, cell })));
  },
});

type SplitActions = ReturnType<typeof splitActions>;

interface GridProps { view: SplitViewProps; slots: Slots; actions: SplitActions }

const SplitGrid = (props: GridProps) => {
  const { view, actions } = props;
  const count = view.cells.length === 4 ? 4 : 2;

  return (
    <div role="group" aria-label="Terminals" className={cn("split-grid grid min-h-0 flex-1 grid-cols-2 gap-px bg-border", count === 4 ? "grid-rows-2" : "grid-rows-1")}>
      {shownCells(view.sessions, view.cells).map((entry, at) =>
        entry ? (
          <SplitCell
            key={`${String(at)}:${entry.cell.session}/${entry.cell.tab}`}
            at={at}
            session={entry.cell.session}
            tab={entry.tab}
            focused={at === view.focus}
            slots={at === view.focus ? props.slots : { status: null, toolbar: null }}
            layout={count}
            onLayout={actions.layout}
            onFocus={() => {
              view.onFocus(at);
            }}
            onRemove={() => {
              actions.remove(at);
            }}
          />
        ) : (
          <Picker key={at} at={at} cells={view.cells} sessions={view.sessions} onPick={(cell) => {
            actions.pick(at, cell);
          }} />
        ),
      )}
    </div>
  );
};

export const SplitView = (props: SplitViewProps) => {
  const [statusSlot, setStatusSlot] = useState<HTMLDivElement | null>(null);
  const [toolbarSlot, setToolbarSlot] = useState<HTMLDivElement | null>(null);
  const focused = shownCells(props.sessions, props.cells)[props.focus];
  const actions = splitActions(props, focused);

  return (
    <>
      {focused ? <div ref={setToolbarSlot} className="flex shrink-0 flex-col" /> : <Toolbar layout={props.cells.length === 4 ? 4 : 2} onLayout={actions.layout} />}
      <div data-testid="divider" className="h-px shrink-0 bg-border" role="none" />
      <SplitGrid view={props} slots={{ status: statusSlot, toolbar: toolbarSlot }} actions={actions} />
      {focused && (
        <>
          <div data-testid="divider" className="h-px shrink-0 bg-border" role="none" />
          <div ref={setStatusSlot} className="flex shrink-0 flex-col" />
        </>
      )}
    </>
  );
};

interface SplitCellProps {
  at: number;
  session: string;
  tab: TabInfo;
  focused: boolean;
  slots: Slots;
  layout: Layout;
  onLayout: (count: Layout) => void;
  onFocus: () => void;
  onRemove: () => void;
}

const CellHeader = (props: SplitCellProps) => {
  const { session, tab } = props;
  const name = tabName(session, tab.tab);
  const alone = tabRoute(session, tab.tab);

  return (
    <div className="flex shrink-0 items-center gap-1 border-b border-border px-1.5 py-0.5 text-[11px]">
      <button
        type="button"
        ref={focusWhenShown(`cell-${String(props.at)}`)}
        aria-pressed={props.focused}
        className="flex min-w-0 flex-1 items-center gap-x-1.5 rounded px-1 py-1 text-left outline-none hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring/50"
      >
        <TabGlyph tab={tab} />
        <span className="flex min-w-0 items-center gap-x-1.5">
          <span data-tooltip={session} data-tooltip-overflow className="min-w-[2ch] flex-1 truncate font-mono font-semibold">{session}</span>
          <span className="text-muted-foreground" aria-hidden="true">
            /
          </span>
          <span data-tooltip={tab.tab} data-tooltip-overflow className="min-w-0 truncate font-mono">{tab.tab}</span>
        </span>
        <span data-tooltip={tab.program} data-tooltip-overflow className="min-w-0 max-w-max flex-1 truncate text-muted-foreground">{tab.program}</span>
        <TabChip tab={tab} />
        <span className="sr-only">, {tabState(tab).label}</span>
      </button>
      <IconLink look={{ label: `Maximize ${name}`, tooltip: "Maximize", icon: Maximize2 }} href={alone} className="[&_svg]:size-3.5" onClick={(event) => {
        focusMain();
        followLink(event, alone);
      }} />
      <IconButton look={{ label: `Remove ${name} from the grid`, tooltip: "Remove", icon: X }} onClick={(event) => {
        event.stopPropagation();
        props.onRemove();
      }} />
    </div>
  );
};

interface PortaledProps { target: HTMLElement | null; children: ReactNode }

const Portaled = ({ target, children }: PortaledProps) => (target ? createPortal(children, target) : null);

const SplitCell = (props: SplitCellProps) => {
  const { session, tab } = props;
  const view = useHeldViewport({ session, tab, focused: props.focused });
  const { stream } = view;
  const name = tabName(session, tab.tab);

  return (
    <>
      <Portaled target={props.slots.toolbar}>
        <Toolbar layout={props.layout} onLayout={props.onLayout} current={{ session, tab, stream: stream.state, shown: stream.shown }} />
      </Portaled>
      <section aria-label={`${name} ${tab.program}`} onClick={props.onFocus} className={cn("flex min-h-0 min-w-0 flex-col bg-background", props.focused && "ring-1 ring-foreground/40 ring-inset")}>
        <CellHeader {...props} />
        <Viewport compact={!props.focused} {...view.viewport} />
        <Portaled target={props.slots.status}>
          <StatusBar tab={tab} state={stream.state} shown={stream.shown} />
        </Portaled>
      </section>
    </>
  );
};

interface PickerProps { at: number; cells: Cell[]; sessions: SessionInfo[]; onPick: (cell: Cell) => void }

interface PickerTabProps { session: string; tab: TabInfo; cells: Cell[]; onPick: (cell: Cell) => void }

const PickerTab = (props: PickerTabProps) => {
  const { session, tab } = props;
  const cell = { session, tab: tab.tab };
  const already = isShown(props.cells, cell);

  return (
    <li>
      <button
        type="button"
        disabled={already}
        onClick={() => {
          props.onPick(cell);
        }}
        className="flex min-h-7 w-full items-center gap-2 px-2.5 py-1 text-left outline-none hover:bg-muted focus-visible:bg-muted disabled:opacity-50"
      >
        <TabGlyph tab={tab} />
        <span className="font-mono">{tab.tab}</span>
        <span data-tooltip={tab.program} data-tooltip-overflow className="min-w-0 flex-1 truncate text-muted-foreground">{tab.program}</span>
        {already ? <Chip tone="muted">shown</Chip> : <TabChip tab={tab} />}
        <span className="sr-only">, {tabName(session, tab.tab)}</span>
      </button>
    </li>
  );
};

const Picker = (props: PickerProps) => (
  <section aria-labelledby={`pick-${String(props.at)}`} className="flex min-h-0 min-w-0 flex-col items-center justify-center gap-2 overflow-y-auto bg-background p-4">
    <h2 id={`pick-${String(props.at)}`} className="text-xs font-normal text-muted-foreground">
      Show a terminal
    </h2>
    <ul className="w-full max-w-64 overflow-hidden rounded-md border border-border text-xs">
      {props.sessions.map((info) => (
        <li key={info.session}>
          <p className="bg-muted/50 px-2.5 py-1 font-mono text-[11px] font-semibold text-muted-foreground">{info.session}</p>
          <ul>
            {info.tabs.map((tab) => (
              <PickerTab key={tab.tab} session={info.session} tab={tab} cells={props.cells} onPick={props.onPick} />
            ))}
          </ul>
        </li>
      ))}
    </ul>
  </section>
);
