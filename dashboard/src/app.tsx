import { useEffect, useState, type ReactNode } from "react";

import { Announcers } from "./components/announcer";
import { Disconnected, NoSessions, NotFound, NotOpen } from "./components/empty-screens";
import { ResizeHandle } from "./components/resize-handle";
import { SessionsSidebar, type Selected } from "./components/sessions-sidebar";
import { SidePanel } from "./components/side-panel";
import { TabScreen } from "./components/tab-screen";
import { Toaster } from "./components/toaster";
import { TooltipLayer } from "./components/tooltip";
import { useDaemon } from "./lib/daemon";
import { navigate, parseRoute, tabRoute, usePath } from "./lib/route";
import { findTab, firstTab, tabName } from "./lib/tabs";
import { useTheme } from "./lib/theme";

type Screen = { title: string | null; center: ReactNode; right?: ReactNode; selected?: Selected };

export const App = () => {
  const { connection, sessions, retry } = useDaemon();
  const { theme, toggle } = useTheme();
  const [sidebarPct, setSidebarPct] = useState(15);
  const route = parseRoute(usePath());

  const first = sessions ? firstTab(sessions) : null;
  const redirect = route.kind === "home" && connection === "connected" && first ? tabRoute(first.session, first.tab) : null;
  useEffect(() => {
    if (redirect) navigate(redirect, { replace: true });
  }, [redirect]);

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
    const tab = findTab(sessions, route.session, route.tab);
    const name = tabName(route.session, route.tab);
    if (!tab) return { title: "Not open", center: <NotOpen name={name} /> };
    return {
      title: `${name} · ${tab.program}`,
      center: <TabScreen key={`${route.session}/${route.tab}`} session={route.session} tab={tab} sessions={sessions} />,
      right: <SidePanel key={`${route.session}/${route.tab}`} session={route.session} tab={tab} />,
      selected: { session: route.session, tab: route.tab },
    };
  })();

  useEffect(() => {
    document.title = screen.title ? `${screen.title} · agent-terminal` : "agent-terminal";
  }, [screen.title]);

  return (
    <>
      <a
        href="#main-content"
        className="sr-only focus:not-sr-only focus:fixed focus:top-2 focus:left-2 focus:z-[200] focus:rounded-md focus:bg-background focus:px-3 focus:py-2 focus:text-sm focus:shadow-md"
      >
        Skip to content
      </a>
      <div className="shell fixed inset-0 bg-background">
        <div className="shell-cols flex h-full min-h-0 w-full">
          <SessionsSidebar
            connection={connection}
            sessions={sessions}
            selected={screen.selected ?? null}
            theme={theme}
            onToggleTheme={toggle}
            width={sidebarPct}
          />
          <ResizeHandle side="left" pct={sidebarPct} label="Resize the sessions sidebar" onResize={setSidebarPct} />
          <main id="main-content" tabIndex={-1} className="col-center flex min-h-0 min-w-0 flex-1 flex-col outline-none">
            <h1 className="sr-only">{screen.title ?? "agent-terminal"}</h1>
            {screen.center}
          </main>
          {screen.right}
        </div>
      </div>
      <Toaster />
      <TooltipLayer />
      <Announcers title={screen.title ?? ""} />
    </>
  );
};
