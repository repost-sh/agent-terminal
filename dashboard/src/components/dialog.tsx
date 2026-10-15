import { X } from "lucide-react";
import { useId, useSyncExternalStore, type FormEvent, type KeyboardEvent, type ReactNode } from "react";
import { flushSync } from "react-dom";

import type { SessionInfo } from "../lib/protocol";
import { Button } from "./ui";

type Render = (sessions: SessionInfo[]) => ReactNode;

type Open = { render: Render; trigger: Element | null };

let open: Open | null = null;
const listeners = new Set<() => void>();

const publish = (next: Open | null) => {
  open = next;
  listeners.forEach((listener) => listener());
};

const subscribe = (listener: () => void) => {
  listeners.add(listener);
  return () => listeners.delete(listener);
};

window.addEventListener("popstate", () => publish(null));

export const showDialog = (render: Render) => publish({ render, trigger: document.activeElement });

const focusable = (element: Element | null | undefined): element is HTMLElement =>
  element instanceof HTMLElement && element.isConnected && !element.matches(":disabled");

export const closeDialog = (focus?: HTMLElement | null) => {
  const trigger = open?.trigger;
  flushSync(() => publish(null));
  const target = focus ?? (focusable(trigger) ? trigger : document.getElementById("main-content"));
  target?.focus({ preventScroll: true });
};

export const useOpenDialog = () => useSyncExternalStore(subscribe, () => open);

export const DialogLayer = ({ sessions }: { sessions: SessionInfo[] }) => {
  const shown = useOpenDialog();
  return shown ? <>{shown.render(sessions)}</> : null;
};

const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]):not([type="hidden"]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

const focusFirst = (node: HTMLDivElement | null) => {
  if (!node) return;
  const first = node.querySelector<HTMLElement>("[data-autofocus]") ?? node.querySelector<HTMLElement>(FOCUSABLE) ?? node;
  first.focus({ preventScroll: true });
};

const keepFocusInside = (event: KeyboardEvent<HTMLDivElement>) => {
  if (event.key === "Escape") {
    event.preventDefault();
    closeDialog();
    return;
  }
  if (event.key !== "Tab") return;
  const content = event.currentTarget;
  const focusables = [...content.querySelectorAll<HTMLElement>(FOCUSABLE)].filter(
    (element) => element.offsetParent !== null || element === document.activeElement,
  );
  const first = focusables[0];
  const last = focusables[focusables.length - 1];
  if (!first || !last) return;
  const inside = content.contains(document.activeElement);
  if (event.shiftKey && (document.activeElement === first || !inside)) {
    event.preventDefault();
    last.focus();
  } else if (!event.shiftKey && (document.activeElement === last || !inside)) {
    event.preventDefault();
    first.focus();
  }
};

const OVERLAY = "fixed inset-0 isolate z-50 bg-black/10 supports-backdrop-filter:backdrop-blur-xs";
const CONTENT =
  "dialog-content fixed top-1/2 left-1/2 z-50 grid max-h-[calc(100dvh-2rem)] w-full -translate-x-1/2 -translate-y-1/2 gap-4 overflow-y-auto rounded-xl bg-popover p-4 text-sm text-popover-foreground ring-1 ring-foreground/10 outline-none";
const FOOTER = "-mx-4 -mb-4 flex flex-col-reverse gap-2 rounded-b-xl border-t bg-muted/50 p-4 sm:flex-row sm:justify-end";

type DialogProps = {
  title: ReactNode;
  description?: string;
  alert?: boolean;
  width: number;
  children?: ReactNode;
  footer: ReactNode;
  onSubmit?: () => void;
};

export const Dialog = ({ title, description, alert = false, width, children, footer, onSubmit }: DialogProps) => {
  const id = useId();
  const inner = (
    <>
      <div className="flex flex-col gap-2 pr-8">
        <h2 id={`${id}-title`} className="font-heading text-base leading-none font-medium [overflow-wrap:anywhere]">
          {title}
        </h2>
        {description && (
          <p id={`${id}-desc`} className="text-sm text-muted-foreground">
            {description}
          </p>
        )}
      </div>
      {children}
      <div className={FOOTER}>{footer}</div>
    </>
  );
  const submit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    onSubmit?.();
  };
  return (
    <div>
      <div className={OVERLAY} onClick={alert ? undefined : () => closeDialog()} />
      <div
        role={alert ? "alertdialog" : "dialog"}
        aria-modal="true"
        aria-labelledby={`${id}-title`}
        aria-describedby={description ? `${id}-desc` : undefined}
        tabIndex={-1}
        ref={focusFirst}
        onKeyDown={keepFocusInside}
        className={CONTENT}
        style={{ maxWidth: `min(calc(100vw - 2rem), ${width}rem)` }}
      >
        {onSubmit ? (
          <form onSubmit={submit} noValidate className="grid gap-4">
            {inner}
          </form>
        ) : (
          inner
        )}
        {!alert && (
          <Button variant="ghost" size="icon-sm" className="absolute top-2 right-2" onClick={() => closeDialog()}>
            <X aria-hidden="true" />
            <span className="sr-only">Close</span>
          </Button>
        )}
      </div>
    </div>
  );
};
