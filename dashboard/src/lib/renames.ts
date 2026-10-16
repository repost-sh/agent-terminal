import type { SessionInfo } from "./protocol";
import { navigate, parseRoute, splitRoute, tabRoute, type Cell, type Route } from "./route";
import { findTab, type TabAddress } from "./tabs";

const renamedTo = (sessions: SessionInfo[], address: TabAddress) =>
  findTab(sessions, address) ? null : (sessions.find((entry) => entry.session === address.session)?.tabs.find((tab) => tab.renamed_from === address.tab)?.tab ?? null);

const followed = (sessions: SessionInfo[], cell: Cell): Cell => {
  const renamed = cell && renamedTo(sessions, cell);

  return cell && renamed ? { session: cell.session, tab: renamed } : cell;
};

export const routeAfterRenames = (route: Route, sessions: SessionInfo[]) => {
  if (route.kind === "tab") {
    const renamed = renamedTo(sessions, route);

    return renamed ? tabRoute(route.session, renamed) : null;
  }

  if (route.kind !== "split") {
    return null;
  }

  const cells = route.cells.map((cell) => followed(sessions, cell));

  return cells.some((cell, at) => cell !== route.cells[at]) ? splitRoute(cells) : null;
};

export const followRenames = (sessions: SessionInfo[]) => {
  if (!sessions.some((info) => info.tabs.some((tab) => tab.renamed_from !== undefined))) {
    return;
  }

  const moved = routeAfterRenames(parseRoute(location.pathname), sessions);

  if (moved) {
    navigate(moved, { replace: true });
  }
};
