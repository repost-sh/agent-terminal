import type { Terminal } from "@xterm/xterm";
import { LoaderCircle, RotateCw } from "lucide-react";
import { useRef, useState, type ClipboardEventHandler, type KeyboardEventHandler, type ReactNode, type RefObject } from "react";

import type { controlKeys } from "../lib/control";
import { focusWhenShown } from "../lib/focus";
import type { StreamState, Shown } from "../lib/stream";
import { tabKey, tabName, type TabAddress } from "../lib/tabs";
import { LARGEST_FONT_PX, openTerminal } from "../lib/xterm";
import { Button, cn } from "./ui";

const PADDING_PX = 32;
const FONT_STEP_PX = 0.25;

const fitFont = (terminal: Terminal, box: RefObject<HTMLDivElement | null>) => {
  const area = box.current;
  const screen = terminal.element?.querySelector<HTMLElement>(".xterm-screen");

  if (!area || !screen) {
    return null;
  }

  const fit = () => {
    const drawn = screen.getBoundingClientRect();

    if (!drawn.width || !drawn.height) {
      return;
    }

    const current = terminal.options.fontSize ?? LARGEST_FONT_PX;
    const room = Math.min((area.clientWidth - PADDING_PX) / drawn.width, (area.clientHeight - PADDING_PX) / drawn.height);
    const wanted = Math.floor((current * room) / FONT_STEP_PX) * FONT_STEP_PX;

    terminal.options.fontSize = Math.max(FONT_STEP_PX * 16, Math.min(LARGEST_FONT_PX, wanted));
  };

  const observer = new ResizeObserver(fit);

  observer.observe(area);
  observer.observe(screen);

  return () => {
    observer.disconnect();
  };
};

interface ExitActions { onRestart: () => void; onClose: () => void }

type Paint = (terminal: Terminal) => () => void;

export interface HeldTerminal {
  name: string;
  focus: string;
  keys?: KeyboardEventHandler<HTMLDivElement> | undefined;
  paste?: ClipboardEventHandler<HTMLDivElement> | undefined;
}

export interface Holding { held: boolean; typing: boolean; keyboard: ReturnType<typeof controlKeys> }

export const heldTerminal = (address: TabAddress, holding: Holding): HeldTerminal | null =>
  holding.held
    ? {
        name: tabName(address.session, address.tab),
        focus: tabKey(address.session, address.tab),
        keys: holding.typing ? holding.keyboard.keys : undefined,
        paste: holding.typing ? holding.keyboard.paste : undefined,
      }
    : null;

export const liveLabel = (title: string, shown: Shown | null) => `${title}, live frame ${String(shown?.frame ?? 0)}`;

interface ViewportProps {
  paint: Paint;
  state: StreamState;
  shown: Shown | null;
  program: string;
  label: string;
  held?: HeldTerminal | null;
  compact?: boolean;
  exit?: ExitActions;
}

interface HeldRingProps { held: HeldTerminal | null; compact: boolean; children: ReactNode }

const HeldRing = (props: HeldRingProps) => {
  const { held } = props;

  return held && !props.compact ? (
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
      {props.children}
    </div>
  ) : (
    <div className={cn(held && "rounded-[2px] ring-2 ring-warning ring-offset-2 ring-offset-background")}>{props.children}</div>
  );
};

const useTerminalHost = (paint: Paint, box: RefObject<HTMLDivElement | null>) =>
  useState(() => (node: HTMLDivElement) =>
    openTerminal(node, (terminal) => {
      const stopFitting = fitFont(terminal, box);
      const stopPainting = paint(terminal);

      return () => {
        stopPainting();
        stopFitting?.();
      };
    }),
  )[0];

const isLoading = (shown: Shown | null) => !shown || (shown.frame === 0 && !shown.exited);

const WaitingForFirstFrame = () => (
  <p className="absolute flex items-center gap-2 text-xs text-muted-foreground">
    <LoaderCircle aria-hidden="true" className="size-3.5 animate-spin" />
    Waiting for first frame...
  </p>
);

interface FrameLook { loading: boolean; dim: boolean }

const frameLook = (props: ViewportProps): FrameLook => ({
  loading: isLoading(props.shown),
  dim: Boolean(props.shown?.exited) || props.state === "reconnecting",
});

const ExitOverlay = (props: ViewportProps) =>
  props.shown?.exited ? <ExitBar program={props.program} code={props.shown.exit_code} actions={props.exit ?? null} /> : null;

export const Viewport = (props: ViewportProps) => {
  const box = useRef<HTMLDivElement>(null);
  const host = useTerminalHost(props.paint, box);
  const look = frameLook(props);

  return (
    <div id={props.exit ? "viewport-region" : undefined} data-testid="viewport" ref={box} className="relative flex min-h-0 flex-1 items-center justify-center overflow-hidden">
      {look.loading && <WaitingForFirstFrame />}
      <HeldRing held={props.held ?? null} compact={Boolean(props.compact)}>
        <div role="group" aria-label={props.label} className={cn("term-frame", look.dim && "is-dim", look.loading && "opacity-0")}>
          <div ref={host} />
        </div>
      </HeldRing>
      <ExitOverlay {...props} />
    </div>
  );
};

interface ExitBarProps { program: string; code: number | null; actions: ExitActions | null }

interface ExitCodeProps { code: number | null }

const ExitCode = ({ code }: ExitCodeProps) =>
  code === null ? (
    " exited"
  ) : (
    <>
      {" exited with code "}
      <span className={cn("font-mono", code !== 0 && "text-destructive")}>{code}</span>
    </>
  );

const ExitBar = (props: ExitBarProps) => (
  <div
    role="status"
    className={cn(
      "absolute inset-x-0 mx-auto flex w-fit max-w-[calc(100%-2rem)] flex-wrap items-center justify-center gap-x-3 gap-y-2 rounded-xl bg-popover px-3 py-2 text-xs text-popover-foreground shadow-lg ring-1 ring-foreground/10",
      props.actions ? "bottom-4" : "bottom-2",
    )}
  >
    <span>
      <span className="font-mono">{props.program}</span>
      <ExitCode code={props.code} />
    </span>
    {props.actions && (
      <span className="flex gap-2">
        <Button look={{ icon: RotateCw }} onClick={props.actions.onRestart}>
          Restart
        </Button>
        <Button look={{ variant: "ghost" }} onClick={props.actions.onClose}>
          Close tab
        </Button>
      </span>
    )}
  </div>
);
