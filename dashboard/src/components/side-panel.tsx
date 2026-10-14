import { useState, type KeyboardEvent } from "react";

import type { TabInfo } from "../lib/protocol";
import { tabName } from "../lib/tabs";
import { PANELS } from "../panels";
import { ResizeHandle } from "./resize-handle";

const TAB_TRIGGER =
  "relative inline-flex h-[calc(100%-1px)] flex-1 items-center justify-center gap-1.5 rounded-md border border-transparent px-1 py-0.5 text-[11px] font-medium whitespace-nowrap text-foreground/60 transition-all outline-none hover:text-foreground focus-visible:ring-[3px] focus-visible:ring-ring/50 dark:text-muted-foreground dark:hover:text-foreground after:absolute after:inset-x-0 after:bottom-[-4px] after:h-0.5 after:bg-foreground after:opacity-0 after:transition-opacity aria-selected:text-foreground dark:aria-selected:text-foreground aria-selected:after:opacity-100";

type SidePanelProps = { session: string; tab: TabInfo };

export const SidePanel = ({ session, tab }: SidePanelProps) => {
  const [active, setActive] = useState(PANELS[0]?.id);
  const [width, setWidth] = useState(30);
  const panel = PANELS.find((entry) => entry.id === active) ?? PANELS[0];
  if (!panel) return null;

  const keys = (event: KeyboardEvent<HTMLDivElement>) => {
    const at = PANELS.findIndex((entry) => entry.id === panel.id);
    const moves: Record<string, number> = { ArrowLeft: at - 1, ArrowRight: at + 1, Home: 0, End: PANELS.length - 1 };
    const to = moves[event.key];
    if (to === undefined) return;
    event.preventDefault();
    const next = PANELS[(to + PANELS.length) % PANELS.length];
    if (!next) return;
    setActive(next.id);
    document.getElementById(`ptab-${next.id}`)?.focus();
  };

  return (
    <>
      <ResizeHandle side="right" pct={width} label="Resize the side panel" onResize={setWidth} />
      <section
        aria-label={`Details of ${tabName(session, tab.tab)}`}
        className="col-right flex min-h-0 shrink-0 flex-col gap-2"
        style={{ width: `${width}%`, minWidth: "19.5rem", maxWidth: "50%" }}
      >
        <div className="shrink-0 px-2 pt-1">
          <div role="tablist" aria-label="Details" onKeyDown={keys} className="flex h-8 w-full items-center gap-0.5 overflow-x-auto p-[3px] text-muted-foreground">
            {PANELS.map((entry) => (
              <button
                key={entry.id}
                type="button"
                role="tab"
                id={`ptab-${entry.id}`}
                aria-controls={`ppanel-${entry.id}`}
                aria-selected={entry.id === panel.id}
                tabIndex={entry.id === panel.id ? 0 : -1}
                onClick={() => setActive(entry.id)}
                className={TAB_TRIGGER}
              >
                {entry.label}
              </button>
            ))}
          </div>
        </div>
        <div role="tabpanel" id={`ppanel-${panel.id}`} aria-labelledby={`ptab-${panel.id}`} className="flex min-h-0 flex-1 flex-col text-sm">
          <panel.Body session={session} tab={tab} />
        </div>
      </section>
    </>
  );
};
