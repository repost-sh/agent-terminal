import { createStore } from "./store";

interface Said { text: string; times: number }

export const announcements = createStore<Said>({ text: "", times: 0 });

export const announce = (text: string) => {
  announcements.set({ text, times: announcements.get().times + 1 });
};
