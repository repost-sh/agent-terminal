import { useSyncExternalStore } from "react";

type Said = { text: string; times: number };

let said: Said = { text: "", times: 0 };
const listeners = new Set<() => void>();

export const announce = (text: string) => {
  said = { text, times: said.times + 1 };
  listeners.forEach((listener) => listener());
};

const subscribe = (listener: () => void) => {
  listeners.add(listener);
  return () => listeners.delete(listener);
};

export const Announcers = ({ title }: { title: string }) => {
  const { text, times } = useSyncExternalStore(subscribe, () => said);
  return (
    <>
      <div className="sr-only" aria-live="assertive" aria-atomic="true">
        {title}
      </div>
      <div className="sr-only" aria-live="polite" aria-atomic="true">
        <span key={times}>{text}</span>
      </div>
    </>
  );
};
