import { useSyncExternalStore } from "react";

let polite = "";
const listeners = new Set<() => void>();

const publish = (text: string) => {
  polite = text;
  listeners.forEach((listener) => listener());
};

export const announce = (text: string) => {
  publish("");
  window.setTimeout(() => publish(text), 30);
};

const subscribe = (listener: () => void) => {
  listeners.add(listener);
  return () => listeners.delete(listener);
};

export const Announcers = ({ title }: { title: string }) => {
  const said = useSyncExternalStore(subscribe, () => polite);
  return (
    <>
      <div className="sr-only" aria-live="assertive" aria-atomic="true">
        {title}
      </div>
      <div className="sr-only" aria-live="polite" aria-atomic="true">
        {said}
      </div>
    </>
  );
};
