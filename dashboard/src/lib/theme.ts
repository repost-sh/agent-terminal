import { useSyncExternalStore } from "react";

export type Theme = "dark" | "light";

const STORED_AS = "agent-terminal-theme";

const listeners = new Set<() => void>();
let theme: Theme = localStorage.getItem(STORED_AS) === "light" ? "light" : "dark";

const subscribe = (listener: () => void) => {
  listeners.add(listener);

  return () => listeners.delete(listener);
};

const toggle = () => {
  theme = theme === "dark" ? "light" : "dark";
  localStorage.setItem(STORED_AS, theme);
  document.documentElement.classList.toggle("dark", theme === "dark");
  document.documentElement.style.colorScheme = theme;
  listeners.forEach((listener) => {
    listener();
  });
};

export const useTheme = () => ({ theme: useSyncExternalStore(subscribe, () => theme), toggle });
