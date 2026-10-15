import { useState, type ReactNode } from "react";

import { Announcers } from "./components/announcer";
import { DialogLayer, useOpenDialog } from "./components/dialog";
import { Disconnected, NoSessions, NotFound, NotOpen } from "./components/empty-screens";
import { ResizeHandle } from "./components/resize-handle";
import { SessionsSidebar, type RouteOf, type Selected } from "./components/sessions-sidebar";
import { SidePanel } from "./components/side-panel";
import { SplitView } from "./components/split-view";
import { TabScreen } from "./components/tab-screen";
import { Toaster } from "./components/toaster";
import { TooltipLayer } from "./components/tooltip";
import { useDaemon } from "./lib/daemon";
import { navigate, parseRoute, splitRoute, usePath } from "./lib/route";
import { fillCells, focusedAt, shownCells, withCell } from "./lib/split";
import { findTab, tabName } from "./lib/tabs";
import { useTheme } from "./lib/theme";

type Screen = { title: string | null; heading?: string; center: ReactNode; right?: ReactNode; selected?: Selected; routeOf?: RouteOf };

export const App = () => {
  const { connection, sessions, retry } = useDaemon();
  const { theme, toggle } = useTheme();
  const [sidebarPct, setSidebarPct] = useState(15);
  const [splitFocus, setSplitFocus] = useState(0);
  const route = parseRoute(usePath());
  const dialog = useOpenDialog();

  const screen = ((): Screen => {
    if (connection === "down") {
      const reconnect = () => {
        navigate("/");
        retry();
      };
      return { title: "Disconnected", center: <Disconnected onRetry={reconnect} /> };
    }
    if (route.kind === "missing") return { title: "404: This page could not be found", center: <NotFound /> };
    if (!sessions) return { title: null, center: null };
    if (route.kind === "home") return sessions.length ? { title: null, center: null } : { title: "No active sessions", center: <NoSessions /> };
    if (route.kind === "split") {
      const { cells } = route;
      const shown = shownCells(sessions, cells);
      const at = focusedAt(shown, splitFocus);
      const focused = shown[at];
      return {
        title: `Split · ${cells.length} terminals`,
        heading: `Split view, ${cells.length} terminals`,
        center: <SplitView cells={cells} sessions={sessions} focus={at} onFocus={setSplitFocus} />,
        right: focused && <SidePanel key={`${focused.cell.session}/${focused.cell.tab}`} session={focused.cell.session} tab={focused.tab} />,
        selected: focused?.cell,
        routeOf: (session, tab) => splitRoute(withCell(cells, at, { session, tab })),
      };
    }
    const tab = findTab(sessions, route.session, route.tab);
    const name = tabName(route.session, route.tab);
    if (!tab) return { title: "Not open", center: <NotOpen name={name} /> };
    return {
      title: `${name} · ${tab.program}`,
      center: (
        <TabScreen
          key={`${route.session}/${route.tab}`}
          session={route.session}
          tab={tab}
          sessions={sessions}
          onLayout={(count) => {
            setSplitFocus(0);
            navigate(splitRoute(fillCells(sessions, { session: route.session, tab: route.tab }, count)));
          }}
        />
      ),
      right: <SidePanel key={`${route.session}/${route.tab}`} session={route.session} tab={tab} />,
      selected: { session: route.session, tab: route.tab },
    };
  })();

  return (
    <>
      <title>{screen.title ? `${screen.title} · agent-terminal` : "agent-terminal"}</title>
      <a
        href="#main-content"
        className="sr-only focus:not-sr-only focus:fixed focus:top-2 focus:left-2 focus:z-[200] focus:rounded-md focus:bg-background focus:px-3 focus:py-2 focus:text-sm focus:shadow-md"
      >
        Skip to content
      </a>
      <div className="shell fixed inset-0 bg-background" inert={dialog !== null}>
        <div className="shell-cols flex h-full min-h-0 w-full">
          <SessionsSidebar
            connection={connection}
            sessions={sessions}
            selected={screen.selected ?? null}
            theme={theme}
            onToggleTheme={toggle}
            width={sidebarPct}
            routeOf={screen.routeOf}
          />
          <ResizeHandle side="left" pct={sidebarPct} label="Resize the sessions sidebar" onResize={setSidebarPct} />
          <main id="main-content" tabIndex={-1} className="col-center flex min-h-0 min-w-0 flex-1 flex-col outline-none">
            <h1 className="sr-only">{screen.heading ?? screen.title ?? "agent-terminal"}</h1>
            {screen.center}
          </main>
          {screen.right}
        </div>
      </div>
      <DialogLayer sessions={sessions ?? []} />
      <Toaster />
      <TooltipLayer />
      <Announcers title={screen.title ?? ""} />
    </>
  );
};
