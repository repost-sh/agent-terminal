import { useActivity } from "../lib/activity";
import { useCommands } from "../lib/commands";
import { clockTime } from "../lib/format";
import type { TabInfo } from "../lib/protocol";
import { useScreenshots } from "../lib/screenshots";
import { tabName } from "../lib/tabs";
import { endOf, goLive, useTimeline, useViewedFrame, viewFrame } from "../lib/timeline";
import { announce } from "./announcer";
import { cn, Dot } from "./ui";

type Tick = { kind: "call" | "command" | "shot"; frame: number };

const TICK = {
  call: "top-[9px] h-1.5 w-px bg-muted-foreground/70",
  command: "top-[7px] h-2.5 w-0.5 bg-foreground/60",
  shot: "top-[10px] size-1 rounded-[1px] bg-foreground",
};

const along = (frame: number, last: number) => `calc(5px + (100% - 10px) * ${last > 1 ? (Math.min(frame, last) - 1) / (last - 1) : 0})`;

export const Timeline = ({ session, tab }: { session: string; tab: TabInfo }) => {
  const { count, started, failed, frame: frameAt } = useTimeline(session, tab.tab);
  const viewed = useViewedFrame(session, tab.tab);
  const { entries } = useActivity(session, tab.tab);
  const commands = useCommands(session, tab.tab);
  const { value: screenshots } = useScreenshots(session);
  const name = tabName(session, tab.tab);
  const last = Math.max(count, 1);
  const shown = viewed ?? last;
  const at = frameAt(shown);
  const live = viewed === null;
  const sinceStart = (time: number) => started !== null && time >= started;

  const ticks: Tick[] = [
    ...entries.flatMap((entry): Tick[] => (entry.frame !== undefined && !entry.human && sinceStart(endOf(entry)) ? [{ kind: "call", frame: entry.frame }] : [])),
    ...(commands?.commands ?? []).map((command): Tick => ({ kind: "command", frame: command.first_frame })),
    ...screenshots.flatMap((shot): Tick[] => (shot.tab === tab.tab && sinceStart(Date.parse(shot.time)) ? [{ kind: "shot", frame: shot.frame }] : [])),
  ];

  const backToLive = () => {
    if (live) return;
    goLive(session, tab.tab);
    announce("Live");
  };

  return (
    <div className="flex shrink-0 items-center gap-2 border-t border-border px-3 py-0.5">
      <div className="relative min-w-0 flex-1">
        <div aria-hidden="true" className="pointer-events-none absolute inset-0">
          {ticks.map((tick, index) => (
            <span key={index} data-tick={tick.kind} className={cn("absolute -translate-x-1/2", TICK[tick.kind])} style={{ left: along(tick.frame, last) }} />
          ))}
        </div>
        <input
          type="range"
          className="scrub relative"
          min={1}
          max={last}
          step={1}
          value={shown}
          onChange={(event) => viewFrame(session, tab.tab, Number(event.currentTarget.value))}
          aria-label={`Timeline of ${name}`}
          aria-valuetext={at ? `frame ${shown} · ${clockTime(at.time_ms)}` : `frame ${shown}`}
          disabled={last <= 1 || failed}
        />
      </div>
      <button
        type="button"
        aria-pressed={live}
        onClick={backToLive}
        className={cn(
          "flex h-6 shrink-0 items-center gap-1.5 rounded border border-border px-1.5 text-[10px] transition-colors outline-none focus-visible:ring-2 focus-visible:ring-ring/50",
          live ? "text-foreground" : "text-muted-foreground hover:text-foreground",
        )}
      >
        <Dot tone={live ? "success" : "muted"} className="size-1.5" />
        Live
      </button>
    </div>
  );
};
