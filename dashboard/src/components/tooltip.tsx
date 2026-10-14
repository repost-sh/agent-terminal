const followTooltips = (tip: HTMLDivElement) => {
  let target: HTMLElement | null = null;
  const hide = () => {
    tip.hidden = true;
    target = null;
  };
  const show = (element: HTMLElement) => {
    tip.textContent = element.dataset.tooltip ?? "";
    tip.hidden = false;
    const rect = element.getBoundingClientRect();
    const left = rect.left + rect.width / 2 - tip.offsetWidth / 2;
    tip.style.left = `${Math.max(4, Math.min(left, window.innerWidth - tip.offsetWidth - 4))}px`;
    tip.style.top = `${Math.max(4, Math.min(rect.bottom + 4, window.innerHeight - tip.offsetHeight - 4))}px`;
    target = element;
  };
  const tooltipOf = (event: Event) => (event.target instanceof Element ? event.target.closest<HTMLElement>("[data-tooltip]") : null);
  const over = (event: MouseEvent) => {
    const element = tooltipOf(event);
    if (element && element !== target) show(element);
    else if (!element && target && target !== document.activeElement) hide();
  };
  const focus = (event: FocusEvent) => {
    const element = tooltipOf(event);
    if (element?.matches(":focus-visible")) show(element);
    else hide();
  };
  const escape = (event: KeyboardEvent) => {
    if (event.key !== "Escape" || !target) return;
    event.stopPropagation();
    hide();
  };
  document.addEventListener("mouseover", over);
  document.addEventListener("focusin", focus);
  document.addEventListener("click", hide);
  document.addEventListener("keydown", escape, true);
  return () => {
    document.removeEventListener("mouseover", over);
    document.removeEventListener("focusin", focus);
    document.removeEventListener("click", hide);
    document.removeEventListener("keydown", escape, true);
  };
};

export const TooltipLayer = () => (
  <div
    ref={followTooltips}
    hidden
    aria-hidden="true"
    className="pointer-events-none fixed z-[70] w-fit max-w-xs rounded-md bg-foreground px-2 py-1 text-[11px] text-background shadow-md"
  />
);
