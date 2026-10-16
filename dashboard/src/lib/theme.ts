import { createStore, useStore } from "./store";

export type Theme = "dark" | "light";

const STORED_AS = "agent-terminal-theme";

const theme = createStore<Theme>(localStorage.getItem(STORED_AS) === "light" ? "light" : "dark");

const toggle = () => {
  const next = theme.get() === "dark" ? "light" : "dark";

  localStorage.setItem(STORED_AS, next);
  document.documentElement.classList.toggle("dark", next === "dark");
  document.documentElement.style.colorScheme = next;
  theme.set(next);
};

export const useTheme = () => ({ theme: useStore(theme), toggle });
