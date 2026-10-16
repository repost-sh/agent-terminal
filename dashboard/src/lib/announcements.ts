interface Said { text: string; times: number }

let said: Said = { text: "", times: 0 };
const listeners = new Set<() => void>();

export const announce = (text: string) => {
  said = { text, times: said.times + 1 };
  listeners.forEach((listener) => {
    listener();
  });
};

export const subscribeToAnnouncements = (listener: () => void) => {
  listeners.add(listener);

  return () => listeners.delete(listener);
};

export const lastAnnouncement = () => said;
