import type { ReactNode } from "react";

import { Badge, cn } from "./ui";

export const HEAD_TOGGLE =
  "flex h-6 min-w-6 items-center justify-center gap-1 rounded border border-border px-1.5 text-[10px] text-muted-foreground transition-colors outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring/50 disabled:pointer-events-none disabled:opacity-50 aria-pressed:text-foreground [&_svg]:size-3";

type PanelHeadProps = { title: string; count?: number; className?: string; children?: ReactNode };

export const PanelHead = ({ title, count, className, children }: PanelHeadProps) => (
  <>
    <div className={cn("flex shrink-0 items-center gap-2 px-3 py-2", className)}>
      <h2 className="text-xs font-normal text-muted-foreground">{title}</h2>
      {count !== undefined && <Badge className="ml-auto">{count}</Badge>}
      {children}
    </div>
    <div className="h-px shrink-0 bg-border" role="none" />
  </>
);

export const EmptyNote = ({ title, children }: { title: string; children?: ReactNode }) => (
  <div className="px-6 py-8 text-center text-xs text-muted-foreground">
    <p>{title}</p>
    {children && <div className="mt-2 text-muted-foreground/70">{children}</div>}
  </div>
);
