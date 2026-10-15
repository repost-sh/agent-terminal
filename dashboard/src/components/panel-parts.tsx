import type { ReactNode } from "react";

import { Badge, cn } from "./ui";

export const HEAD_TOGGLE =
  "flex h-6 min-w-6 items-center justify-center gap-1 rounded border border-border px-1.5 text-[10px] text-muted-foreground transition-colors outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring/50 disabled:pointer-events-none disabled:opacity-50 aria-pressed:text-foreground [&_svg]:size-3";

interface PanelHeadProps { title: string; count?: number | undefined; className?: string; children?: ReactNode }

export const PanelHead = (props: PanelHeadProps) => (
  <>
    <div className={cn("flex shrink-0 items-center gap-2 px-3 py-2", props.className)}>
      <h2 className="text-xs font-normal text-muted-foreground">{props.title}</h2>
      {props.count !== undefined && <Badge className="ml-auto">{props.count}</Badge>}
      {props.children}
    </div>
    <div className="h-px shrink-0 bg-border" role="none" />
  </>
);

interface EmptyNoteProps { title: string; children?: ReactNode }

export const EmptyNote = ({ title, children }: EmptyNoteProps) => (
  <div className="px-6 py-8 text-center text-xs text-muted-foreground">
    <p>{title}</p>
    {children && <div className="mt-2 text-muted-foreground/70">{children}</div>}
  </div>
);
