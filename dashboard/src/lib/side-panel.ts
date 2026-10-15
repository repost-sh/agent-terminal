import { useSyncExternalStore } from "react";

type SidePanel = { panel: string; width: number };

const listeners = new Set<() => void>();
let shown: SidePanel = { panel: "activity", width: 30 };

const publish = (next: Partial<SidePanel>) => {
  shown = { ...shown, ...next };
  listeners.forEach((listener) => listener());
};

const subscribe = (listener: () => void) => {
  listeners.add(listener);
  return () => listeners.delete(listener);
};

export const showPanel = (panel: string) => publish({ panel });

export const resizePanel = (width: number) => publish({ width });

export const useSidePanel = () => useSyncExternalStore(subscribe, () => shown);
