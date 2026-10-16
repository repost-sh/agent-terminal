import { ChevronRight, Hand, Moon, Plus, SquareTerminal, Sun, Trash2 } from "lucide-react";
import { useState } from "react";

import type { Connection } from "../lib/daemon";
import type { SessionInfo, TabInfo } from "../lib/protocol";
import { followLink, tabRoute } from "../lib/route";
import { tabName, tabState } from "../lib/tabs";
import type { Theme } from "../lib/theme";
import { CloseAllDialog } from "./close-all";
import { showDialog } from "./dialog";
import { NewSessionDialog } from "./new-session";
import { Badge, Chip, Dot, IconButton, cn } from "./ui";

export type Selected = { session: string; tab: string } | null;

interface TabProps { tab: TabInfo }

export const TabGlyph = ({ tab }: TabProps) => {
  const kind = tabState(tab).kind;

  if (kind === "human") {
    return (
      <span className="flex size-4 shrink-0 items-center justify-center text-yellow-700 dark:text-warning" aria-hidden="true">
        <Hand className="size-[13px]" />
      </span>
    );
  }

  return kind === "running" ? (
    <span className="flex size-4 shrink-0 items-center justify-center">
      <Dot tone="success" />
    </span>
  ) : (
    <span className="flex size-4 shrink-0 items-center justify-center" aria-hidden="true">
      <span className="size-2 rounded-full border border-muted-foreground/70" />
    </span>
  );
};

export const TabChip = ({ tab }: TabProps) => {
  const state = tabState(tab);

  return state.kind === "running" ? null : <Chip tone={state.tone}>{state.label}</Chip>;
};

export type RouteOf = (session: string, tab: string) => string;

interface TabRowProps { session: string; tab: TabInfo; selected: boolean; routeOf: RouteOf }

const TabRow = (props: TabRowProps) => {
  const { session, tab } = props;

  return (
    <li>
      <a
        href={tabRoute(session, tab.tab)}
        onClick={(event) => {
          followLink(event, props.routeOf(session, tab.tab));
        }}
        aria-current={props.selected ? "page" : undefined}
        className={cn(
          "flex w-full min-w-0 items-center gap-2 py-1 pr-1 pl-7 text-left text-xs outline-none select-none focus-visible:ring-2 focus-visible:ring-ring/50 focus-visible:ring-inset",
          props.selected ? "bg-card text-foreground" : "text-muted-foreground hover:text-foreground",
        )}
      >
        <TabGlyph tab={tab} />
        <span className="font-mono break-all">{tab.tab}</span>
        <span className="min-w-0 flex-1 break-all text-muted-foreground">{tab.program}</span>
        <span className="sr-only">
          , {tabName(session, tab.tab)}, {tabState(tab).label}
        </span>
        <TabChip tab={tab} />
      </a>
    </li>
  );
};

interface GroupProps { info: SessionInfo; selected: Selected; routeOf: RouteOf }

interface GroupHeadProps { info: SessionInfo; current: boolean; open: boolean; onToggle: () => void }

const GroupHead = (props: GroupHeadProps) => {
  const count = props.info.tabs.length;

  return (
    <button
      type="button"
      aria-expanded={props.open}
      aria-controls={`tabs-${props.info.session}`}
      onClick={props.onToggle}
      className={cn(
        "flex w-full items-center text-xs transition-colors outline-none select-none focus-visible:ring-2 focus-visible:ring-ring/50 focus-visible:ring-inset",
        props.current ? "text-foreground" : "text-muted-foreground hover:text-foreground",
      )}
    >
      <span className="flex size-6 shrink-0 items-center justify-center">
        <ChevronRight aria-hidden="true" className={cn("size-3 transition-transform", props.open && "rotate-90")} />
      </span>
      <span className="flex min-w-0 flex-1 items-center gap-2 py-1.5 pr-3 pl-1 text-left">
        <SquareTerminal aria-hidden="true" className="size-[15px] shrink-0" />
        <span className="min-w-0 font-mono font-semibold break-all">{props.info.session}</span>
        <Badge className="ml-auto">{count}</Badge>
        <span className="sr-only">
          , {count} {count === 1 ? "tab" : "tabs"}
        </span>
      </span>
    </button>
  );
};

interface NewTabProps { session: string }

const NewTabRow = ({ session }: NewTabProps) => (
  <li>
    <button
      type="button"
      onClick={() => {
        showDialog((live) => <NewSessionDialog sessions={live} existing={session} />);
      }}
      className="flex w-full items-center gap-2 py-1 pr-1 pl-7 text-xs text-muted-foreground outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring/50 focus-visible:ring-inset"
    >
      <Plus aria-hidden="true" className="size-4" />
      New tab<span className="sr-only"> in {session}</span>
    </button>
  </li>
);

const SessionGroup = (props: GroupProps) => {
  const [open, setOpen] = useState(true);
  const { info, selected } = props;

  return (
    <div>
      <GroupHead info={info} current={selected?.session === info.session} open={open} onToggle={() => {
        setOpen(!open);
      }} />
      <ul id={`tabs-${info.session}`} hidden={!open} className="pb-1">
        {info.tabs.map((tab) => (
          <TabRow
            key={tab.tab}
            session={info.session}
            tab={tab}
            selected={selected?.session === info.session && selected.tab === tab.tab}
            routeOf={props.routeOf}
          />
        ))}
        <NewTabRow session={info.session} />
      </ul>
    </div>
  );
};

interface SidebarProps {
  connection: Connection;
  sessions: SessionInfo[] | null;
  selected: Selected;
  theme: Theme;
  onToggleTheme: () => void;
  width: number;
  routeOf?: RouteOf | undefined;
}

interface SessionListProps { connection: Connection; sessions: SessionInfo[] | null; selected: Selected; routeOf: RouteOf }

const SessionList = (props: SessionListProps) => {
  const { sessions } = props;

  if (props.connection === "down") {
    return <p className="py-4 text-center text-xs text-muted-foreground">Not connected</p>;
  }

  if (sessions === null) {
    return null;
  }

  return sessions.length ? (
    sessions.map((info) => <SessionGroup key={info.session} info={info} selected={props.selected} routeOf={props.routeOf} />)
  ) : (
    <p className="py-4 text-center text-xs text-muted-foreground">No sessions</p>
  );
};

interface SidebarButtonsProps { connection: Connection; sessions: SessionInfo[] | null; theme: Theme; onToggleTheme: () => void }

const SidebarButtons = (props: SidebarButtonsProps) => {
  const dark = props.theme === "dark";
  const down = props.connection === "down";

  return (
    <div className="ml-auto flex items-center gap-0.5">
      <IconButton look={{ kind: "head", label: dark ? "Switch to light mode" : "Switch to dark mode", icon: dark ? Sun : Moon }} onClick={props.onToggleTheme} />
      <IconButton look={{ kind: "head", label: "Close all sessions", icon: Trash2 }} disabled={down || !props.sessions?.length} onClick={() => {
        showDialog((live) => <CloseAllDialog sessions={live} />);
      }} />
      <IconButton look={{ kind: "head", label: "New session", icon: Plus }} disabled={down} onClick={() => {
        showDialog((live) => <NewSessionDialog sessions={live} existing={null} />);
      }} />
    </div>
  );
};

export const SessionsSidebar = (props: SidebarProps) => (
  <aside aria-label="Sessions" className="col-left flex min-h-0 shrink-0 flex-col" style={{ width: `${String(props.width)}%`, minWidth: "12rem", maxWidth: "30%" }}>
    <div className="flex shrink-0 items-center px-3 py-2">
      <h2 className="text-xs font-normal text-muted-foreground">Sessions</h2>
      <SidebarButtons connection={props.connection} sessions={props.sessions} theme={props.theme} onToggleTheme={props.onToggleTheme} />
    </div>
    <div className="h-px shrink-0 bg-border" />
    <nav aria-label="Sessions and tabs" className="min-h-0 flex-1 overflow-y-auto py-1">
      <SessionList connection={props.connection} sessions={props.sessions} selected={props.selected} routeOf={props.routeOf ?? tabRoute} />
    </nav>
  </aside>
);
