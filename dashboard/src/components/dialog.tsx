import { X } from "lucide-react";
import { useId, type KeyboardEvent, type ReactNode, type SubmitEvent } from "react";
import { flushSync } from "react-dom";

import type { SessionInfo } from "../lib/protocol";
import { createStore, useStore } from "../lib/store";
import { Button } from "./ui";

type Render = (sessions: SessionInfo[]) => ReactNode;

interface Open { render: Render; trigger: Element | null }

const open = createStore<Open | null>(null);

window.addEventListener("popstate", () => {
  open.set(null);
});

export const showDialog = (render: Render) => {
  open.set({ render, trigger: document.activeElement });
};

const focusable = (element: Element | null | undefined): element is HTMLElement =>
  element instanceof HTMLElement && element.isConnected && !element.matches(":disabled");

export const closeDialog = (focus?: HTMLElement | null) => {
  const trigger = open.get()?.trigger;

  flushSync(() => {
    open.set(null);
  });
  const target = focus ?? (focusable(trigger) ? trigger : document.getElementById("main-content"));

  target?.focus({ preventScroll: true });
};

export const useOpenDialog = () => useStore(open);

interface DialogLayerProps { sessions: SessionInfo[] }

export const DialogLayer = ({ sessions }: DialogLayerProps) => {
  const shown = useOpenDialog();

  return shown ? <>{shown.render(sessions)}</> : null;
};

const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]):not([type="hidden"]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

const focusFirst = (node: HTMLDivElement | null) => {
  if (!node) {
    return;
  }

  const first = node.querySelector<HTMLElement>("[data-autofocus]") ?? node.querySelector<HTMLElement>(FOCUSABLE) ?? node;

  first.focus({ preventScroll: true });
};

const focusablesIn = (content: HTMLElement) =>
  [...content.querySelectorAll<HTMLElement>(FOCUSABLE)].filter((element) => element.offsetParent !== null || element === document.activeElement);

interface Ends { first: HTMLElement; last: HTMLElement }

const endsOf = (focusables: HTMLElement[]): Ends | null => {
  const first = focusables[0];
  const last = focusables.at(-1);

  return first && last ? { first, last } : null;
};

const wrappedTo = (event: KeyboardEvent<HTMLDivElement>, ends: Ends) => {
  const outside = !event.currentTarget.contains(document.activeElement);
  const [from, to] = event.shiftKey ? [ends.first, ends.last] : [ends.last, ends.first];

  return document.activeElement === from || outside ? to : null;
};

const keepFocusInside = (event: KeyboardEvent<HTMLDivElement>) => {
  if (event.key === "Escape") {
    event.preventDefault();
    closeDialog();

    return;
  }

  if (event.key !== "Tab") {
    return;
  }

  const ends = endsOf(focusablesIn(event.currentTarget));
  const target = ends && wrappedTo(event, ends);

  if (target) {
    event.preventDefault();
    target.focus();
  }
};

const OVERLAY = "fixed inset-0 isolate z-50 bg-black/10 supports-backdrop-filter:backdrop-blur-xs";
const CONTENT =
  "dialog-content fixed top-1/2 left-1/2 z-50 grid max-h-[calc(100dvh-2rem)] w-full -translate-x-1/2 -translate-y-1/2 gap-4 overflow-y-auto rounded-xl bg-popover p-4 text-sm text-popover-foreground ring-1 ring-foreground/10 outline-none";
const FOOTER = "-mx-4 -mb-4 flex flex-col-reverse gap-2 rounded-b-xl border-t bg-muted/50 p-4 sm:flex-row sm:justify-end";

interface DialogProps {
  title: ReactNode;
  description?: string;
  alert?: boolean;
  width: number;
  children?: ReactNode;
  footer: ReactNode;
  onSubmit?: () => void;
}

interface HeadingProps { id: string; title: ReactNode; description: string | undefined }

const DialogHeading = (props: HeadingProps) => (
  <div className="flex flex-col gap-2 pr-8">
    <h2 id={`${props.id}-title`} className="font-heading text-base leading-none font-medium [overflow-wrap:anywhere]">
      {props.title}
    </h2>
    {props.description && (
      <p id={`${props.id}-desc`} className="text-sm text-muted-foreground">
        {props.description}
      </p>
    )}
  </div>
);

interface BodyProps { inner: ReactNode; onSubmit: (() => void) | undefined }

const DialogBody = ({ inner, onSubmit }: BodyProps) => {
  if (!onSubmit) {
    return inner;
  }

  const submit = (event: SubmitEvent<HTMLFormElement>) => {
    event.preventDefault();
    onSubmit();
  };

  return (
    <form onSubmit={submit} noValidate className="grid gap-4">
      {inner}
    </form>
  );
};

const CloseButton = () => (
  <Button look={{ variant: "ghost", size: "icon-sm" }} className="absolute top-2 right-2" onClick={() => {
    closeDialog();
  }}>
    <X aria-hidden="true" />
    <span className="sr-only">Close</span>
  </Button>
);

export const Dialog = (props: DialogProps) => {
  const id = useId();
  const alert = props.alert ?? false;
  const inner = (
    <>
      <DialogHeading id={id} title={props.title} description={props.description} />
      {props.children}
      <div className={FOOTER}>{props.footer}</div>
    </>
  );

  return (
    <div>
      <div className={OVERLAY} onClick={alert ? undefined : () => {
        closeDialog();
      }} />
      <div
        role={alert ? "alertdialog" : "dialog"}
        aria-modal="true"
        aria-labelledby={`${id}-title`}
        aria-describedby={props.description ? `${id}-desc` : undefined}
        tabIndex={-1}
        ref={focusFirst}
        onKeyDown={keepFocusInside}
        className={CONTENT}
        style={{ maxWidth: `min(calc(100vw - 2rem), ${String(props.width)}rem)` }}
      >
        <DialogBody inner={inner} onSubmit={props.onSubmit} />
        {!alert && <CloseButton />}
      </div>
    </div>
  );
};
