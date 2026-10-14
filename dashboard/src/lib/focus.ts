let pending: string | null = null;

export const focusNext = (name: string) => {
  pending = name;
};

export const focusWhenShown = (name: string) => (element: HTMLElement | null) => {
  if (!element || pending !== name) return;
  pending = null;
  element.focus({ preventScroll: true });
};
