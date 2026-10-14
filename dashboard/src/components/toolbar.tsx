import type { ReactNode } from "react";

import type { Layout } from "../lib/split";
import { LayoutSwitch } from "./layout-switch";

type ToolbarProps = { layout: Layout; onLayout: (count: Layout) => void; children?: ReactNode };

export const Toolbar = ({ layout, onLayout, children }: ToolbarProps) => (
  <div className="flex shrink-0 flex-wrap items-center justify-end gap-1.5 px-2 py-1.5">
    <LayoutSwitch layout={layout} onLayout={onLayout} />
    {children}
  </div>
);
