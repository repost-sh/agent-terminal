import type { ReactNode } from "react";

import { Badge } from "./ui";

export const PanelHead = ({ title, count }: { title: string; count?: number }) => (
  <>
    <div className="flex shrink-0 items-center gap-2 px-3 py-2">
      <h2 className="text-xs font-normal text-muted-foreground">{title}</h2>
      {count !== undefined && <Badge className="ml-auto">{count}</Badge>}
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
