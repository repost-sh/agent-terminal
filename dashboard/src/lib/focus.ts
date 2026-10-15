let pending: string | null = null;

export const focusNext = (name: string) => {
  pending = name;
};

export const focusWhenShown = (name: string) => (element: HTMLElement | null) => {
  if (!element || pending !== name) {
    return;
  }

  pending = null;
  element.focus({ preventScroll: true });
};

export const focusNowOrWhenShown = (id: string) => {
  const element = document.getElementById(id);

  if (element && !element.matches(":disabled")) {
    element.focus({ preventScroll: true });
  } else {
    focusNext(id);
  }
};
