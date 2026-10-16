import { createStore, useStore } from "./store";

interface SidePanel { panel: string; width: number }

const shown = createStore<SidePanel>({ panel: "activity", width: 30 });

export const showPanel = (panel: string) => {
  shown.set({ ...shown.get(), panel });
};

export const resizePanel = (width: number) => {
  shown.set({ ...shown.get(), width });
};

export const useSidePanel = () => useStore(shown);
