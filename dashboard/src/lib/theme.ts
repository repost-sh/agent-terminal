import { useEffect, useState } from "react";

export type Theme = "dark" | "light";

const STORED_AS = "agent-terminal-theme";

const stored = (): Theme => (localStorage.getItem(STORED_AS) === "light" ? "light" : "dark");

export const useTheme = () => {
  const [theme, setTheme] = useState<Theme>(stored);
  useEffect(() => {
    document.documentElement.classList.toggle("dark", theme === "dark");
    document.documentElement.style.colorScheme = theme;
  }, [theme]);
  const toggle = () => {
    const next = theme === "dark" ? "light" : "dark";
    localStorage.setItem(STORED_AS, next);
    setTheme(next);
  };
  return { theme, toggle };
};
