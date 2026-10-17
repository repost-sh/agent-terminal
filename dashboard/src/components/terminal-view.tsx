import type { Terminal } from "@xterm/xterm";
import { LoaderCircle, RotateCw } from "lucide-react";
import { useRef, useState, type ClipboardEventHandler, type KeyboardEventHandler, type ReactNode, type RefCallback, type RefObject } from "react";

import { controlKeys, useControl } from "../lib/control";
import { focusWhenShown } from "../lib/focus";
import { controlMouse, trackedIn, type Tracked } from "../lib/mouse";
import { useMouseWanted } from "../lib/mouse-preference";
import type { TabInfo } from "../lib/protocol";
import { useTabStream, type StreamState, type Shown } from "../lib/stream";
import { tabKey, tabName, type TabAddress } from "../lib/tabs";
import { LARGEST_FONT_PX, openTerminal } from "../lib/xterm";
import { ControlOverlay } from "./control-button";
import { Button, cn } from "./ui";

const PADDING_PX = 32;
const FONT_STEP_PX = 0.25;

const fitFont = (terminal: Terminal, box: RefObject<HTMLDivElement | null>) => {
  const area = box.current;
  const screen = terminal.element?.querySelector<HTMLElement>(".xterm-screen");

  if (!area || !screen) {
    return null;
  }

  let fittedTo = "";
  let overflowsFrom = Infinity;

  const fit = () => {
    const drawn = screen.getBoundingClientRect();

    if (!drawn.width || !drawn.height) {
      return;
    }

    const layout = `${String(area.clientWidth)}x${String(area.clientHeight)} ${String(terminal.cols)}x${String(terminal.rows)}`;

    if (layout !== fittedTo) {
      fittedTo = layout;
      overflowsFrom = Infinity;
    }

    const current = terminal.options.fontSize ?? LARGEST_FONT_PX;
    const room = Math.min((area.clientWidth - PADDING_PX) / drawn.width, (area.clientHeight - PADDING_PX) / drawn.height);

    if (room < 1) {
      overflowsFrom = Math.min(overflowsFrom, current);
    }

    const wanted = Math.min(Math.floor((current * room) / FONT_STEP_PX) * FONT_STEP_PX, overflowsFrom - FONT_STEP_PX);

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
  focusIn?: (() => void) | undefined;
  focusOut?: (() => void) | undefined;
  mouse?: RefCallback<HTMLElement> | undefined;
}

export interface Holding {
  held: boolean;
  typing: boolean;
  keyboard: ReturnType<typeof controlKeys>;
  mouse: ReturnType<typeof controlMouse>;
  tracked: Tracked | null;
}

const typedInto = (holding: Holding): Partial<HeldTerminal> =>
  holding.typing ? { ...holding.keyboard, mouse: holding.tracked ? holding.mouse.attach : undefined } : {};

export const heldTerminal = (address: TabAddress, holding: Holding): HeldTerminal | null => {
  holding.mouse.track(holding.typing ? holding.tracked : null);

  return holding.held ? { name: tabName(address.session, address.tab), focus: tabKey(address.session, address.tab), ...typedInto(holding) } : null;
};

export const liveLabel = (title: string, shown: Shown | null) => `${title}, live frame ${String(shown?.frame ?? 0)}`;

export interface HeldTab { session: string; tab: TabInfo; focused: boolean }

export const useHeldViewport = (heldTab: HeldTab) => {
  const { session, tab } = heldTab;
  const stream = useTabStream(session, tab.tab);
  const [keyboard] = useState(() => controlKeys(session, tab.tab));
  const [mouse] = useState(() => controlMouse({ session, tab: tab.tab }));
  const tracked = useMouseWanted() ? trackedIn(stream.shown) : null;
  const held = useControl(session, tab.tab) === "held";
  const typing = held && heldTab.focused && !stream.shown?.exited;
  const name = tabName(session, tab.tab);

  return {
    stream,
    viewport: {
      paint: stream.paint,
      state: stream.state,
      shown: stream.shown,
      program: tab.program,
      label: liveLabel(`${name} ${tab.program}`, stream.shown),
      held: heldTerminal({ session, tab: tab.tab }, { held, typing, keyboard, mouse, tracked }),
      overlay: <ControlOverlay session={session} tab={tab} stream={stream.state} shown={stream.shown} />,
    },
  };
};

interface ViewportProps {
  paint: Paint;
  state: StreamState;
  shown: Shown | null;
  program: string;
  label: string;
  held?: HeldTerminal | null;
  compact?: boolean;
  exit?: ExitActions;
  overlay?: ReactNode;
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
      onKeyDown={held.keys}
      onPaste={held.paste}
      onFocus={held.focusIn}
      onBlur={held.focusOut}
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

interface FrameProps { look: FrameLook; label: string; mouse: RefCallback<HTMLElement> | undefined; host: RefCallback<HTMLDivElement> }

const Frame = (props: FrameProps) => {
  const { look, mouse } = props;

  return (
    <div ref={mouse} role="group" aria-label={props.label} className={cn("term-frame", look.dim && "is-dim", look.loading && "opacity-0", mouse && "is-mouse")}>
      <div ref={props.host} />
    </div>
  );
};

const POINTER_REST_MS = 1500;

const usePointerMoving = () => {
  const [moving, setMoving] = useState(false);
  const rest = useRef<number | null>(null);
  const settle = (next: boolean) => {
    if (rest.current !== null) {
      clearTimeout(rest.current);
    }

    rest.current = next ? window.setTimeout(settle, POINTER_REST_MS, false) : null;
    setMoving(next);
  };

  return {
    moving,
    moved: () => {
      settle(true);
    },
    left: () => {
      settle(false);
    },
  };
};

export const Viewport = (props: ViewportProps) => {
  const box = useRef<HTMLDivElement>(null);
  const host = useTerminalHost(props.paint, box);
  const look = frameLook(props);
  const pointer = usePointerMoving();

  return (
    <div id={props.exit ? "viewport-region" : undefined} data-testid="viewport" data-moving={pointer.moving} ref={box} onMouseMove={pointer.moved} onMouseLeave={pointer.left} className="group/viewport relative flex min-h-0 flex-1 items-center justify-center overflow-hidden">
      {look.loading && <WaitingForFirstFrame />}
      <HeldRing held={props.held ?? null} compact={Boolean(props.compact)}>
        <Frame look={look} label={props.label} mouse={props.held?.mouse} host={host} />
      </HeldRing>
      <div data-testid="viewport-bottom" className={cn("pointer-events-none absolute inset-x-0 z-10 flex flex-col items-center gap-2", props.exit ? "bottom-4" : "bottom-2")}>
        {props.overlay}
        <ExitOverlay {...props} />
      </div>
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
      "pointer-events-auto flex w-fit max-w-[calc(100%-2rem)] flex-wrap items-center justify-center gap-x-3 gap-y-2 rounded-xl bg-popover px-3 py-2 text-xs text-popover-foreground shadow-lg ring-1 ring-foreground/10",
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
