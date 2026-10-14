import { ChevronRight, Moon, Plus, SquareTerminal, Sun, Trash2 } from "lucide-react";
import { useState, type MouseEvent } from "react";

import type { Connection } from "../lib/daemon";
import type { SessionInfo, TabInfo } from "../lib/protocol";
import { navigate, tabRoute } from "../lib/route";
import { tabName, tabState } from "../lib/tabs";
import type { Theme } from "../lib/theme";
import { CloseAllDialog } from "./close-all";
import { showDialog } from "./dialog";
import { NewSessionDialog } from "./new-session";
import { Badge, Chip, Dot, IconButton, cn } from "./ui";

export type Selected = { session: string; tab: string } | null;

export const TabGlyph = ({ tab }: { tab: TabInfo }) =>
  tabState(tab).kind === "running" ? (
    <span className="flex size-4 shrink-0 items-center justify-center">
      <Dot tone="success" />
    </span>
  ) : (
    <span className="flex size-4 shrink-0 items-center justify-center" aria-hidden="true">
      <span className="size-2 rounded-full border border-muted-foreground/70" />
    </span>
  );

export const TabChip = ({ tab }: { tab: TabInfo }) => {
  const state = tabState(tab);
  return state.kind === "running" ? null : <Chip tone={state.tone}>{state.label}</Chip>;
};

const follow = (event: MouseEvent<HTMLAnchorElement>, to: string) => {
  if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
  event.preventDefault();
  navigate(to);
};

const TabRow = ({ session, tab, selected }: { session: string; tab: TabInfo; selected: boolean }) => {
  const to = tabRoute(session, tab.tab);
  return (
    <li>
      <a
        href={to}
        onClick={(event) => follow(event, to)}
        aria-current={selected ? "page" : undefined}
        className={cn(
          "flex w-full min-w-0 items-center gap-2 py-1 pr-1 pl-7 text-left text-xs outline-none select-none focus-visible:ring-2 focus-visible:ring-ring/50 focus-visible:ring-inset",
          selected ? "bg-card text-foreground" : "text-muted-foreground hover:text-foreground",
        )}
      >
        <TabGlyph tab={tab} />
        <span className="min-w-0 flex-1 break-all">{tab.program}</span>
        <span className="sr-only">
          , {tabName(session, tab.tab)}, {tabState(tab).label}
        </span>
        <TabChip tab={tab} />
      </a>
    </li>
  );
};

const SessionGroup = ({ info, selected }: { info: SessionInfo; selected: Selected }) => {
  const [open, setOpen] = useState(true);
  const listId = `tabs-${info.session}`;
  const count = info.tabs.length;
  return (
    <div>
      <button
        type="button"
        aria-expanded={open}
        aria-controls={listId}
        onClick={() => setOpen(!open)}
        className={cn(
          "flex w-full items-center text-xs transition-colors outline-none select-none focus-visible:ring-2 focus-visible:ring-ring/50 focus-visible:ring-inset",
          selected?.session === info.session ? "text-foreground" : "text-muted-foreground hover:text-foreground",
        )}
      >
        <span className="flex size-6 shrink-0 items-center justify-center">
          <ChevronRight aria-hidden="true" className={cn("size-3 transition-transform", open && "rotate-90")} />
        </span>
        <span className="flex min-w-0 flex-1 items-center gap-2 py-1.5 pr-3 pl-1 text-left">
          <SquareTerminal aria-hidden="true" className="size-[15px] shrink-0" />
          <span className="min-w-0 font-mono font-semibold break-all">{info.session}</span>
          <Badge className="ml-auto">{count}</Badge>
          <span className="sr-only">
            , {count} {count === 1 ? "tab" : "tabs"}
          </span>
        </span>
      </button>
      <ul id={listId} hidden={!open} className="pb-1">
        {info.tabs.map((tab) => (
          <TabRow key={tab.tab} session={info.session} tab={tab} selected={selected?.session === info.session && selected.tab === tab.tab} />
        ))}
        <li>
          <button
            type="button"
            onClick={() => showDialog((live) => <NewSessionDialog sessions={live} existing={info.session} />)}
            className="flex w-full items-center gap-2 py-1 pr-1 pl-7 text-xs text-muted-foreground outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring/50 focus-visible:ring-inset"
          >
            <Plus aria-hidden="true" className="size-4" />
            New tab<span className="sr-only"> in {info.session}</span>
          </button>
        </li>
      </ul>
    </div>
  );
};

type SidebarProps = {
  connection: Connection;
  sessions: SessionInfo[] | null;
  selected: Selected;
  theme: Theme;
  onToggleTheme: () => void;
  width: number;
};

export const SessionsSidebar = ({ connection, sessions, selected, theme, onToggleTheme, width }: SidebarProps) => {
  const list =
    connection === "down" ? (
      <p className="py-4 text-center text-xs text-muted-foreground">Not connected</p>
    ) : sessions === null ? null : sessions.length ? (
      sessions.map((info) => <SessionGroup key={info.session} info={info} selected={selected} />)
    ) : (
      <p className="py-4 text-center text-xs text-muted-foreground">No sessions</p>
    );
  return (
    <aside aria-label="Sessions" className="col-left flex min-h-0 shrink-0 flex-col" style={{ width: `${width}%`, minWidth: "12rem", maxWidth: "30%" }}>
      <div className="flex shrink-0 items-center px-3 py-2">
        <h2 className="text-xs font-normal text-muted-foreground">Sessions</h2>
        <div className="ml-auto flex items-center gap-0.5">
          <IconButton
            kind="head"
            label={theme === "dark" ? "Switch to light mode" : "Switch to dark mode"}
            icon={theme === "dark" ? Sun : Moon}
            onClick={onToggleTheme}
          />
          <IconButton
            kind="head"
            label="Close all sessions"
            icon={Trash2}
            disabled={connection === "down" || !sessions?.length}
            onClick={() => showDialog((live) => <CloseAllDialog sessions={live} />)}
          />
          <IconButton
            kind="head"
            label="New session"
            icon={Plus}
            disabled={connection === "down"}
            onClick={() => showDialog((live) => <NewSessionDialog sessions={live} existing={null} />)}
          />
        </div>
      </div>
      <div className="h-px shrink-0 bg-border" />
      <nav aria-label="Sessions and tabs" className="min-h-0 flex-1 overflow-y-auto py-1">
        {list}
      </nav>
    </aside>
  );
};
