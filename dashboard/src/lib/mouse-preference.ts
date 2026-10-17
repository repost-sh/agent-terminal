import { createStore, useStore } from "./store";

const STORED_AS = "agent-terminal-mouse";

const wanted = createStore(localStorage.getItem(STORED_AS) !== "off");

export const toggleMouse = () => {
  const next = !wanted.get();

  localStorage.setItem(STORED_AS, next ? "on" : "off");
  wanted.set(next);
};

export const useMouseWanted = () => useStore(wanted);
