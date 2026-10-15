import { Columns2, Grid2x2, Square, type LucideIcon } from "lucide-react";

import { focusNext, focusWhenShown } from "../lib/focus";
import type { Layout } from "../lib/split";
import { IconButton } from "./ui";

const LAYOUTS: { count: Layout; icon: LucideIcon; label: string }[] = [
  { count: 1, icon: Square, label: "Single terminal" },
  { count: 2, icon: Columns2, label: "Two terminals side by side" },
  { count: 4, icon: Grid2x2, label: "Four terminals" },
];

interface LayoutSwitchProps { layout: Layout; onLayout: (count: Layout) => void }

export const LayoutSwitch = ({ layout, onLayout }: LayoutSwitchProps) => (
  <div role="group" aria-label="Layout" className="flex items-center">
    {LAYOUTS.map((option) => (
      <IconButton look={{ label: option.label, icon: option.icon }} key={option.count} ref={focusWhenShown(`layout-${String(option.count)}`)} aria-pressed={layout === option.count} onClick={() => {
          if (option.count === layout) {
            return;
          }

          focusNext(`layout-${String(option.count)}`);
          onLayout(option.count);
        }} />
    ))}
  </div>
);
