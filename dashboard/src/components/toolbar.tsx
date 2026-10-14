import type { TabInfo } from "../lib/protocol";
import { TOOLBAR_ACTIONS, type ToolbarActionProps } from "../toolbar-actions";

const CommandLine = ({ tab }: { tab: TabInfo }) => (
  <div role="group" aria-label="Command line" className="flex min-w-0 flex-1 flex-wrap items-baseline gap-x-2 rounded-md bg-muted px-2.5 py-1 font-mono text-xs">
    <span className="break-all text-foreground">{tab.command}</span>
    <span className="break-all text-muted-foreground/70">{tab.cwd}</span>
  </div>
);

export const Toolbar = (props: ToolbarActionProps) => (
  <div className="flex shrink-0 flex-wrap items-center gap-1.5 px-2 py-1.5">
    <CommandLine tab={props.tab} />
    <div className="flex flex-wrap items-center gap-1.5">
      {TOOLBAR_ACTIONS.map(({ id, Action }) => (
        <Action key={id} {...props} />
      ))}
    </div>
  </div>
);
