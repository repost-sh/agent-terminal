import { useSyncExternalStore, type MouseEvent } from "react";

export type Cell = { session: string; tab: string } | null;

export type Route = { kind: "home" } | { kind: "tab"; session: string; tab: string } | { kind: "split"; cells: Cell[] } | { kind: "missing" };

const listeners = new Set<() => void>();
let home: string | null = null;

const settle = () => {
  if (home && location.pathname === "/") {
    history.replaceState(null, "", home);
  }

  listeners.forEach((listener) => {
    listener();
  });
};

window.addEventListener("popstate", settle);

const subscribe = (listener: () => void) => {
  listeners.add(listener);

  return () => listeners.delete(listener);
};

export const usePath = () => useSyncExternalStore(subscribe, () => location.pathname);

export const navigate = (path: string, { replace = false } = {}) => {
  if (location.pathname !== path) {
    if (replace) {
      history.replaceState(null, "", path);
    } else {
      history.pushState(null, "", path);
    }
  }

  settle();
};

export const leadHomeTo = (path: string | null) => {
  home = path;
  settle();
};

export const followLink = (event: MouseEvent<HTMLAnchorElement>, to: string) => {
  if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) {
    return;
  }

  event.preventDefault();
  navigate(to);
};

export const tabRoute = (session: string, tab: string) => `/s/${encodeURIComponent(session)}/${encodeURIComponent(tab)}`;

const cellPart = (name: string) => encodeURIComponent(name).replaceAll(".", "%2E");

export const splitRoute = (cells: Cell[]) =>
  `/split/${cells.map((cell) => (cell ? `${cellPart(cell.session)}:${cellPart(cell.tab)}` : "_")).join(",")}`;

const parseCell = (part: string): Cell | undefined => {
  if (part === "_") {
    return null;
  }

  const [session, tab, ...rest] = part.split(":");

  return session && tab && !rest.length ? { session: decodeURIComponent(session), tab: decodeURIComponent(tab) } : undefined;
};

const SPLIT_SIZES = [2, 4];

const tabRouteOf = (path: string): Route | undefined => {
  const tab = /^\/s\/([^/]+)\/([^/]+)$/.exec(path);

  return tab?.[1] && tab[2] ? { kind: "tab", session: decodeURIComponent(tab[1]), tab: decodeURIComponent(tab[2]) } : undefined;
};

const splitRouteOf = (path: string): Route | undefined => {
  const split = (/^\/split\/([^/]+)$/.exec(path))?.[1]?.split(",").map(parseCell);
  const shown = split && SPLIT_SIZES.includes(split.length) && split.every((cell): cell is Cell => cell !== undefined);

  return shown ? { kind: "split", cells: split } : undefined;
};

export const parseRoute = (path: string): Route => {
  if (path === "/") {
    return { kind: "home" };
  }

  return tabRouteOf(path) ?? splitRouteOf(path) ?? { kind: "missing" };
};
