interface Tip { element: HTMLDivElement; target: HTMLElement | null }

const hide = (tip: Tip) => {
  tip.element.hidden = true;
  tip.target = null;
};

const show = (tip: Tip, target: HTMLElement) => {
  const element = tip.element;

  element.textContent = target.getAttribute("data-tooltip");
  element.hidden = false;
  const rect = target.getBoundingClientRect();
  const left = rect.left + rect.width / 2 - element.offsetWidth / 2;

  element.style.left = `${String(Math.max(4, Math.min(left, window.innerWidth - element.offsetWidth - 4)))}px`;
  element.style.top = `${String(Math.max(4, Math.min(rect.bottom + 4, window.innerHeight - element.offsetHeight - 4)))}px`;
  tip.target = target;
};

const tooltipOf = (event: Event) => (event.target instanceof Element ? event.target.closest<HTMLElement>("[data-tooltip]") : null);

const tipHandlers = (tip: Tip) => ({
  over: (event: MouseEvent) => {
    const element = tooltipOf(event);

    if (element) {
      show(tip, element);
    } else if (tip.target !== document.activeElement) {
      hide(tip);
    }
  },
  focus: (event: FocusEvent) => {
    const element = tooltipOf(event);

    if (element?.matches(":focus-visible")) {
      show(tip, element);
    } else {
      hide(tip);
    }
  },
  escape: (event: KeyboardEvent) => {
    if (event.key !== "Escape" || !tip.target) {
      return;
    }

    event.stopPropagation();
    hide(tip);
  },
  hide: () => {
    hide(tip);
  },
});

const followTooltips = (element: HTMLDivElement) => {
  const on = tipHandlers({ element, target: null });
  const following = new AbortController();
  const { signal } = following;

  document.addEventListener("mouseover", on.over, { signal });
  document.addEventListener("focusin", on.focus, { signal });
  document.addEventListener("click", on.hide, { signal });
  document.addEventListener("keydown", on.escape, { capture: true, signal });

  return () => {
    following.abort();
  };
};

export const TooltipLayer = () => (
  <div
    ref={followTooltips}
    role="tooltip"
    hidden
    aria-hidden="true"
    className="pointer-events-none fixed z-[70] w-fit max-w-xs rounded-md bg-foreground px-2 py-1 text-[11px] text-background shadow-md"
  />
);
