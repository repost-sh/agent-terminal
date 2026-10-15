import type { Terminal } from "@xterm/xterm";
import { LoaderCircle, RotateCw } from "lucide-react";
import { useCallback, useRef, type ClipboardEventHandler, type KeyboardEventHandler, type ReactNode, type RefObject } from "react";

import { focusWhenShown } from "../lib/focus";
import type { StreamState, Shown } from "../lib/stream";
import { LARGEST_FONT_PX, openTerminal } from "../lib/xterm";
import { Button, cn } from "./ui";

const PADDING_PX = 32;
const FONT_STEP_PX = 0.25;

const fitFont = (terminal: Terminal, box: RefObject<HTMLDivElement | null>) => {
  const area = box.current;
  const screen = terminal.element?.querySelector<HTMLElement>(".xterm-screen");
  if (!area || !screen) return () => {};
  const fit = () => {
    const drawn = screen.getBoundingClientRect();
    if (!drawn.width || !drawn.height) return;
    const current = terminal.options.fontSize ?? LARGEST_FONT_PX;
    const room = Math.min((area.clientWidth - PADDING_PX) / drawn.width, (area.clientHeight - PADDING_PX) / drawn.height);
    const wanted = Math.floor((current * room) / FONT_STEP_PX) * FONT_STEP_PX;
    const next = Math.max(FONT_STEP_PX * 16, Math.min(LARGEST_FONT_PX, wanted));
    if (Math.abs(next - current) >= FONT_STEP_PX) terminal.options.fontSize = next;
  };
  const observer = new ResizeObserver(fit);
  observer.observe(area);
  observer.observe(screen);
  return () => observer.disconnect();
};

type ExitActions = { onRestart: () => void; onClose: () => void };

type Paint = (terminal: Terminal, drawn: () => void) => () => void;

export type Past = { label: string; paint: Paint };

export type HeldTerminal = {
  name: string;
  focus: string;
  keys?: KeyboardEventHandler<HTMLDivElement>;
  paste?: ClipboardEventHandler<HTMLDivElement>;
};

type ViewportProps = {
  paint: Paint;
  state: StreamState;
  shown: Shown | null;
  program: string;
  label: string;
  past?: Past | null;
  held?: HeldTerminal | null;
  compact?: boolean;
  exit?: ExitActions;
};

const HeldRing = ({ held, compact, children }: { held: HeldTerminal | null; compact: boolean; children: ReactNode }) =>
  held && !compact ? (
    <div
      ref={focusWhenShown(held.focus)}
      tabIndex={0}
      role="application"
      aria-roledescription="terminal"
      aria-label={`Terminal ${held.name}, you have control`}
      aria-describedby="control-hint"
      onKeyDown={held.keys}
      onPaste={held.paste}
      className="rounded-[2px] ring-2 ring-warning ring-offset-4 ring-offset-background outline-none focus-visible:ring-[3px]"
    >
      {children}
    </div>
  ) : (
    <div className={cn(held && "rounded-[2px] ring-2 ring-warning ring-offset-2 ring-offset-background")}>{children}</div>
  );

export const Viewport = (props: ViewportProps) => {
  const { paint, state, shown, program, label, past, held = null } = props;
  const box = useRef<HTMLDivElement>(null);
  const host = useCallback(
    (node: HTMLDivElement) =>
      openTerminal(node, (terminal, drawn) => {
        const stopFitting = fitFont(terminal, box);
        const stopPainting = paint(terminal, drawn);
        return () => {
          stopPainting();
          stopFitting();
        };
      }),
    [paint],
  );

  const loading = !shown || (shown.frame === 0 && !shown.exited);
  const dim = shown?.exited || state === "reconnecting";

  return (
    <div id={props.exit ? "viewport-region" : undefined} ref={box} className="relative flex min-h-0 flex-1 items-center justify-center overflow-hidden">
      {loading && (
        <p className="absolute flex items-center gap-2 text-xs text-muted-foreground">
          <LoaderCircle aria-hidden="true" className="size-3.5 animate-spin" />
          Waiting for first frame...
        </p>
      )}
      <HeldRing held={held} compact={Boolean(props.compact)}>
        <div
          role="group"
          aria-label={label}
          aria-hidden={past ? true : undefined}
          inert={Boolean(past)}
          className={cn("term-frame", dim && "is-dim", (loading || past) && "opacity-0")}
        >
          <div ref={host} />
        </div>
      </HeldRing>
      {past && <PastView box={box} past={past} dim={Boolean(dim)} />}
      {shown?.exited && !past && <ExitBar program={program} code={shown.exit_code} actions={props.exit ?? null} />}
    </div>
  );
};

const PastView = ({ box, past, dim }: { box: RefObject<HTMLDivElement | null>; past: Past; dim: boolean }) => {
  const { paint } = past;
  const host = useCallback(
    (node: HTMLDivElement) =>
      openTerminal(node, (terminal, drawn) => {
        const stopFitting = fitFont(terminal, box);
        const stopPainting = paint(terminal, drawn);
        return () => {
          stopPainting();
          stopFitting();
        };
      }),
    [box, paint],
  );
  return (
    <div className="absolute inset-0 flex items-center justify-center">
      <div role="group" aria-label={past.label} className={cn("term-frame", dim && "is-dim")}>
        <div ref={host} />
      </div>
    </div>
  );
};

const ExitBar = ({ program, code, actions }: { program: string; code: number | null; actions: ExitActions | null }) => (
  <div
    role="status"
    className={cn(
      "absolute inset-x-0 mx-auto flex w-fit max-w-[calc(100%-2rem)] flex-wrap items-center justify-center gap-x-3 gap-y-2 rounded-xl bg-popover px-3 py-2 text-xs text-popover-foreground shadow-lg ring-1 ring-foreground/10",
      actions ? "bottom-4" : "bottom-2",
    )}
  >
    <span>
      <span className="font-mono">{program}</span>
      {code === null ? (
        " exited"
      ) : (
        <>
          {" exited with code "}
          <span className={cn("font-mono", code !== 0 && "text-destructive")}>{code}</span>
        </>
      )}
    </span>
    {actions && (
      <span className="flex gap-2">
        <Button icon={RotateCw} onClick={actions.onRestart}>
          Restart
        </Button>
        <Button variant="ghost" onClick={actions.onClose}>
          Close tab
        </Button>
      </span>
    )}
  </div>
);
