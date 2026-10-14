import { useSyncExternalStore } from "react";

export type Route = { kind: "home" } | { kind: "tab"; session: string; tab: string } | { kind: "missing" };

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

export const tabRoute = (session: string, tab: string) => `/s/${encodeURIComponent(session)}/${encodeURIComponent(tab)}`;

export const parseRoute = (path: string): Route => {
  if (path === "/") return { kind: "home" };
  const tab = path.match(/^\/s\/([^/]+)\/([^/]+)$/);
  if (tab?.[1] && tab[2]) return { kind: "tab", session: decodeURIComponent(tab[1]), tab: decodeURIComponent(tab[2]) };
  return { kind: "missing" };
};
