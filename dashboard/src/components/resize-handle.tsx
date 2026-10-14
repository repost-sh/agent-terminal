import type { KeyboardEvent, PointerEvent } from "react";

export type Side = "left" | "right";

export const LIMITS: Record<Side, [number, number]> = { left: [10, 30], right: [15, 50] };

export const clampPct = (side: Side, pct: number) => {
  const [min, max] = LIMITS[side];
  return Math.round(Math.min(max, Math.max(min, pct)));
};

type HandleProps = {
  side: Side;
  pct: number;
  label: string;
  onResize: (pct: number) => void;
};

export const ResizeHandle = ({ side, pct, label, onResize }: HandleProps) => {
  const [min, max] = LIMITS[side];
  const grow = side === "left" ? 1 : -1;

  const drag = (event: PointerEvent<HTMLDivElement>) => {
    const handle = event.currentTarget;
    const columns = handle.parentElement?.getBoundingClientRect();
    if (!columns) return;
    event.preventDefault();
    handle.setPointerCapture(event.pointerId);
    const move = (moved: globalThis.PointerEvent) => {
      const from = side === "left" ? moved.clientX - columns.left : columns.right - moved.clientX;
      onResize(clampPct(side, (from / columns.width) * 100));
    };
    handle.addEventListener("pointermove", move);
    handle.addEventListener("pointerup", () => handle.removeEventListener("pointermove", move), { once: true });
  };

  const keys = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
    event.preventDefault();
    onResize(clampPct(side, pct + (event.key === "ArrowRight" ? 2 : -2) * grow));
  };

  return (
    <div
      role="separator"
      aria-orientation="vertical"
      aria-label={label}
      aria-valuenow={pct}
      aria-valuemin={min}
      aria-valuemax={max}
      tabIndex={0}
      onPointerDown={drag}
      onKeyDown={keys}
      className="relative z-10 -mx-3 w-6 shrink-0 cursor-col-resize touch-none outline-none before:absolute before:inset-y-0 before:left-1/2 before:w-px before:-translate-x-1/2 before:bg-border hover:before:bg-ring focus-visible:before:w-0.5 focus-visible:before:bg-ring"
    />
  );
};
