import { createStore, useStore } from "./store";

export type PanelId = "activity" | "commands" | "screenshots" | "output" | "info";

interface SidePanel { panel: PanelId; width: number }

const shown = createStore<SidePanel>({ panel: "activity", width: 30 });

export const showPanel = (panel: PanelId) => {
  shown.set({ ...shown.get(), panel });
};

export const resizePanel = (width: number) => {
  shown.set({ ...shown.get(), width });
};

export const useSidePanel = () => useStore(shown);
