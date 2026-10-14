import type { TabInfo } from "../lib/protocol";
import type { Layout } from "../lib/split";
import { TOOLBAR_ACTIONS, type ToolbarTab } from "../toolbar-actions";
import { LayoutSwitch } from "./layout-switch";

const CommandLine = ({ tab }: { tab: TabInfo }) => (
  <div role="group" aria-label="Command line" className="flex min-w-0 flex-1 flex-wrap items-baseline gap-x-2 rounded-md bg-muted px-2.5 py-1 font-mono text-xs">
    <span className="break-all text-foreground">{tab.command}</span>
    <span className="break-all text-muted-foreground/70">{tab.cwd}</span>
  </div>
);

type ToolbarProps = { layout: Layout; onLayout: (count: Layout) => void; current?: ToolbarTab };

export const Toolbar = ({ layout, onLayout, current }: ToolbarProps) =>
  current ? (
    <div className="flex shrink-0 flex-wrap items-center gap-1.5 px-2 py-1.5">
      <CommandLine tab={current.tab} />
      <div className="flex flex-wrap items-center gap-1.5">
        {TOOLBAR_ACTIONS.map(({ id, Action }) => (
          <Action key={id} {...current} layout={layout} onLayout={onLayout} />
        ))}
      </div>
    </div>
  ) : (
    <div className="flex shrink-0 flex-wrap items-center justify-end gap-1.5 px-2 py-1.5">
      <LayoutSwitch layout={layout} onLayout={onLayout} />
    </div>
  );
