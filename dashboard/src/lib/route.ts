import { useSyncExternalStore, type MouseEvent } from "react";

export type Cell = { session: string; tab: string } | null;

export type Route = { kind: "home" } | { kind: "tab"; session: string; tab: string } | { kind: "split"; cells: Cell[] } | { kind: "missing" };

const listeners = new Set<() => void>();
const notify = () => listeners.forEach((listener) => listener());
window.addEventListener("popstate", notify);

const subscribe = (listener: () => void) => {
  listeners.add(listener);
  return () => listeners.delete(listener);
};

export const usePath = () => useSyncExternalStore(subscribe, () => location.pathname);

export const navigate = (path: string, { replace = false } = {}) => {
  if (location.pathname !== path) {
    if (replace) history.replaceState(null, "", path);
    else history.pushState(null, "", path);
  }
  notify();
};

export const followLink = (event: MouseEvent<HTMLAnchorElement>, to: string) => {
  if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
  event.preventDefault();
  navigate(to);
};

export const tabRoute = (session: string, tab: string) => `/s/${encodeURIComponent(session)}/${encodeURIComponent(tab)}`;

const cellPart = (name: string) => encodeURIComponent(name).replaceAll(".", "%2E");

export const splitRoute = (cells: Cell[]) =>
  `/split/${cells.map((cell) => (cell ? `${cellPart(cell.session)}:${cellPart(cell.tab)}` : "_")).join(",")}`;

const parseCell = (part: string): Cell | undefined => {
  if (part === "_") return null;
  const [session, tab, ...rest] = part.split(":");
  return session && tab && !rest.length ? { session: decodeURIComponent(session), tab: decodeURIComponent(tab) } : undefined;
};

const SPLIT_SIZES = [2, 4];

export const parseRoute = (path: string): Route => {
  if (path === "/") return { kind: "home" };
  const tab = path.match(/^\/s\/([^/]+)\/([^/]+)$/);
  if (tab?.[1] && tab[2]) return { kind: "tab", session: decodeURIComponent(tab[1]), tab: decodeURIComponent(tab[2]) };
  const split = path.match(/^\/split\/([^/]+)$/)?.[1]?.split(",").map(parseCell);
  if (split && SPLIT_SIZES.includes(split.length) && split.every((cell): cell is Cell => cell !== undefined)) return { kind: "split", cells: split };
  return { kind: "missing" };
};
