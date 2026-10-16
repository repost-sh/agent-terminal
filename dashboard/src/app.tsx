import { useState, type ReactNode } from "react";

import { Announcers } from "./components/announcer";
import { DialogLayer, useOpenDialog } from "./components/dialog";
import { Disconnected, NotFound, NothingOpen, NotOpen } from "./components/empty-screens";
import { ResizeHandle } from "./components/resize-handle";
import { SessionsSidebar, type RouteOf, type Selected } from "./components/sessions-sidebar";
import { SidePanel } from "./components/side-panel";
import { SplitView } from "./components/split-view";
import { TabScreen } from "./components/tab-screen";
import { Toaster } from "./components/toaster";
import { TooltipLayer } from "./components/tooltip";
import { useDaemon } from "./lib/daemon";
import type { SessionInfo } from "./lib/protocol";
import { navigate, parseRoute, splitRoute, usePath, type Route } from "./lib/route";
import { fillCells, focusedAt, shownCells, withCell } from "./lib/split";
import { findTab, tabKey, tabName } from "./lib/tabs";
import { useTheme } from "./lib/theme";

interface Screen { title: string | null; heading?: string; center: ReactNode; right?: ReactNode; selected?: Selected | undefined; routeOf?: RouteOf | undefined }

type Daemon = ReturnType<typeof useDaemon>;

interface AppView { daemon: Daemon; route: Route; splitFocus: number; setSplitFocus: (at: number) => void }

interface Opened<Kind extends Route["kind"]> { route: Extract<Route, { kind: Kind }>; sessions: SessionInfo[] }

const NOT_FOUND: Screen = { title: "404: This page could not be found", center: <NotFound /> };

const disconnected = (daemon: Daemon): Screen => {
  const reconnect = () => {
    navigate("/");
    daemon.retry();
  };

  return { title: "Disconnected", center: <Disconnected onRetry={reconnect} /> };
};

const splitScreen = (view: AppView, opened: Opened<"split">): Screen => {
  const cells = opened.route.cells;
  const shown = shownCells(opened.sessions, cells);
  const at = focusedAt(shown, view.splitFocus);
  const focused = shown[at];

  return {
    title: `Split · ${String(cells.length)} terminals`,
    heading: `Split view, ${String(cells.length)} terminals`,
    center: <SplitView cells={cells} sessions={opened.sessions} focus={at} onFocus={view.setSplitFocus} />,
    right: focused && <SidePanel key={tabKey(focused.cell.session, focused.cell.tab)} session={focused.cell.session} tab={focused.tab} />,
    selected: focused?.cell,
    routeOf: (session, tab) => splitRoute(withCell(cells, { at, cell: { session, tab } })),
  };
};

const tabScreen = (view: AppView, opened: Opened<"tab">): Screen => {
  const { route, sessions } = opened;
  const tab = findTab(sessions, route);
  const name = tabName(route.session, route.tab);

  if (!tab) {
    return { title: "Not open", center: <NotOpen name={name} /> };
  }

  return {
    title: `${name} · ${tab.program}`,
    center: (
      <TabScreen
        key={tabKey(route.session, route.tab)}
        session={route.session}
        tab={tab}
        sessions={sessions}
        onLayout={(count) => {
          view.setSplitFocus(0);
          navigate(splitRoute(fillCells(sessions, { from: { session: route.session, tab: route.tab }, count })));
        }}
      />
    ),
    right: <SidePanel key={tabKey(route.session, route.tab)} session={route.session} tab={tab} />,
    selected: { session: route.session, tab: route.tab },
  };
};

const homeScreen = (sessions: SessionInfo[]): Screen => {
  if (sessions.some((info) => info.tabs.length > 0)) {
    return { title: null, center: null };
  }

  return { title: sessions.length ? "No open tabs" : "No active sessions", center: <NothingOpen sessions={sessions.map((info) => info.session)} /> };
};

interface Routed { route: Exclude<Route, { kind: "missing" }>; sessions: SessionInfo[] }

const routedScreen = (view: AppView, routed: Routed): Screen => {
  const { route, sessions } = routed;

  if (route.kind === "home") {
    return homeScreen(sessions);
  }

  return route.kind === "split" ? splitScreen(view, { route, sessions }) : tabScreen(view, { route, sessions });
};

const screenFor = (view: AppView): Screen => {
  const { route } = view;

  if (view.daemon.connection === "down") {
    return disconnected(view.daemon);
  }

  if (route.kind === "missing") {
    return NOT_FOUND;
  }

  return view.daemon.sessions ? routedScreen(view, { route, sessions: view.daemon.sessions }) : { title: null, center: null };
};

interface PageTitleProps { title: string | null }

const PageTitle = ({ title }: PageTitleProps) => <title>{title ? `${title} · agent-terminal` : "agent-terminal"}</title>;

const headingOf = (screen: Screen) => screen.heading ?? screen.title ?? "agent-terminal";

const SkipLink = () => (
  <a
    href="#main-content"
    className="sr-only focus:not-sr-only focus:fixed focus:top-2 focus:left-2 focus:z-[200] focus:rounded-md focus:bg-background focus:px-3 focus:py-2 focus:text-sm focus:shadow-md"
  >
    Skip to content
  </a>
);

export const App = () => {
  const daemon = useDaemon();
  const theme = useTheme();
  const [sidebarPct, setSidebarPct] = useState(15);
  const [splitFocus, setSplitFocus] = useState(0);
  const route = parseRoute(usePath());
  const dialog = useOpenDialog();
  const screen = screenFor({ daemon, route, splitFocus, setSplitFocus });

  return (
    <>
      <PageTitle title={screen.title} />
      <SkipLink />
      <div className="shell fixed inset-0 bg-background" inert={dialog !== null}>
        <div className="shell-cols flex h-full min-h-0 w-full">
          <SessionsSidebar
            connection={daemon.connection}
            sessions={daemon.sessions}
            selected={screen.selected ?? null}
            theme={theme.theme}
            onToggleTheme={theme.toggle}
            width={sidebarPct}
            routeOf={screen.routeOf}
          />
          <ResizeHandle side="left" pct={sidebarPct} label="Resize the sessions sidebar" onResize={setSidebarPct} />
          <main id="main-content" tabIndex={-1} className="col-center flex min-h-0 min-w-0 flex-1 flex-col outline-none">
            <h1 className="sr-only">{headingOf(screen)}</h1>
            {screen.center}
          </main>
          {screen.right}
        </div>
      </div>
      <DialogLayer sessions={daemon.sessions ?? []} />
      <Toaster />
      <TooltipLayer />
      <Announcers title={screen.title ?? ""} />
    </>
  );
};
