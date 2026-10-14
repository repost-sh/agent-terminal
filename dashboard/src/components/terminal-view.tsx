import type { Terminal } from "@xterm/xterm";
import { LoaderCircle, RotateCw } from "lucide-react";
import { useEffect, useRef, type RefObject } from "react";

import type { StreamState, Shown } from "../lib/stream";
import { drawWithWebgl, LARGEST_FONT_PX } from "../lib/xterm";
import { Button, cn } from "./ui";

const PADDING_PX = 32;
const FONT_STEP_PX = 0.25;

const useFit = (terminal: Terminal | null, box: RefObject<HTMLDivElement | null>) => {
  useEffect(() => {
    const area = box.current;
    const screen = terminal?.element?.querySelector<HTMLElement>(".xterm-screen");
    if (!terminal || !area || !screen) return;
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
  }, [terminal, box]);
};

type ExitActions = { onRestart: () => void; onClose: () => void };

type ViewportProps = {
  terminal: Terminal | null;
  state: StreamState;
  shown: Shown | null;
  program: string;
  label: string;
} & ({ compact: true } | ({ compact?: false } & ExitActions));

export const Viewport = (props: ViewportProps) => {
  const { terminal, state, shown, program, label } = props;
  const box = useRef<HTMLDivElement>(null);
  const host = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!terminal || !host.current || terminal.element) return;
    terminal.open(host.current);
    drawWithWebgl(terminal);
  }, [terminal]);
  useFit(terminal, box);

  const loading = !shown || (shown.frame === 0 && !shown.exited);
  const dim = shown?.exited || state === "reconnecting";

  return (
    <div id={props.compact ? undefined : "viewport-region"} ref={box} className="relative flex min-h-0 flex-1 items-center justify-center overflow-hidden">
      {loading && (
        <p className="absolute flex items-center gap-2 text-xs text-muted-foreground">
          <LoaderCircle aria-hidden="true" className="size-3.5 animate-spin" />
          Waiting for first frame...
        </p>
      )}
      <div role="group" aria-label={label} className={cn("term-frame", dim && "is-dim", loading && "opacity-0")}>
        <div ref={host} />
      </div>
      {shown?.exited && <ExitBar program={program} code={shown.exit_code} actions={props.compact ? null : props} />}
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
