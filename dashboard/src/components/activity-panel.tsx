import { ArrowDown, ArrowDownToLine, Trash2 } from "lucide-react";
import { useRef, useState, type UIEvent } from "react";

import { resultOf, useActivity } from "../lib/activity";
import { screenshotUrl } from "../lib/api";
import { clockMs, formatDuration } from "../lib/format";
import type { ActivityEntry } from "../lib/protocol";
import { fileName, useScreenshots } from "../lib/screenshots";
import { tabName } from "../lib/tabs";
import { endOf, goLive, showMoment, useViewingPast } from "../lib/timeline";
import type { PanelProps } from "../panels";
import { announce } from "./announcer";
import { HEAD_TOGGLE, PanelHead } from "./panel-parts";
import { openLightbox } from "./screenshot-lightbox";
import { cn } from "./ui";

const FOLLOWING_WITHIN_PX = 40;

const RESULT_TONE = {
  destructive: { text: "text-destructive", row: "bg-destructive/5" },
  warning: { text: "text-yellow-700 dark:text-warning", row: "bg-warning/5" },
};

type Cleared = { first: string; count: number };

const cleared = new Map<string, Cleared>();

const stickToLatest = (list: HTMLOListElement | null) => {
  if (!list) return;
  const toLatest = () => {
    if (list.dataset.follow === "true") list.scrollTop = list.scrollHeight;
  };
  toLatest();
  const watcher = new MutationObserver(toLatest);
  watcher.observe(list, { childList: true, subtree: true, attributes: true, attributeFilter: ["data-follow"] });
  return () => watcher.disconnect();
};

const THUMBNAIL =
  "block w-40 max-w-full overflow-hidden rounded-sm ring-1 ring-border outline-none hover:ring-ring focus-visible:ring-2 focus-visible:ring-ring";

const Thumbnails = ({ session, paths }: { session: string; paths: string[] }) => {
  const shots = useScreenshots(session).value;
  return (
    <>
      {paths
        .filter((path) => path.toLowerCase().endsWith(".png"))
        .map((path) => {
          const shot = shots.find((listed) => listed.path === path);
          const picture = (
            <>
              <span className="sr-only">Open screenshot {fileName(path)}</span>
              <img src={screenshotUrl(session, path)} alt="" className="block w-full" />
            </>
          );
          return (
            <div key={path} className="mb-1.5 flex gap-2">
              <span className="invisible shrink-0 font-mono text-xs" aria-hidden="true">
                00:00:00.000
              </span>
              {shot ? (
                <button type="button" onClick={() => openLightbox(session, shot)} className={THUMBNAIL}>
                  {picture}
                </button>
              ) : (
                <a href={screenshotUrl(session, path)} target="_blank" rel="noreferrer" className={THUMBNAIL}>
                  {picture}
                </a>
              )}
            </div>
          );
        })}
    </>
  );
};

type RowProps = { session: string; entry: ActivityEntry; expanded: boolean; onToggle: () => void };

const ActivityRow = ({ session, entry, expanded, onToggle }: RowProps) => {
  const result = resultOf(entry);
  const tone = result && RESULT_TONE[result.tone];
  const opens = entry.output !== "";
  return (
    <li className={cn("border-b border-border", tone?.row, expanded && "bg-muted/60")}>
      <div className="px-3">
        <button
          type="button"
          aria-expanded={opens ? expanded : undefined}
          onClick={onToggle}
          className="flex w-full items-start gap-2 rounded-sm py-1.5 text-left text-xs outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
        >
          <span className="shrink-0 font-mono text-muted-foreground">
            <time dateTime={entry.time} className="tabular-nums">
              {clockMs(entry.time)}
            </time>
          </span>
          <span className="min-w-0 flex-1 font-mono [overflow-wrap:anywhere]">
            {entry.command} <span className="whitespace-nowrap text-muted-foreground">{formatDuration(entry.duration_ms)}</span>
            {result && <span className={cn("mt-0.5 flex items-center gap-1", tone?.text)}>{result.label}</span>}
          </span>
          {opens && (
            <span className="ml-auto shrink-0 font-mono text-muted-foreground" aria-hidden="true">
              {expanded ? "-" : "+"}
            </span>
          )}
        </button>
        {entry.screenshots && <Thumbnails session={session} paths={entry.screenshots} />}
        {expanded && opens && <pre className="mb-1.5 max-h-48 overflow-auto text-[10px] leading-snug text-muted-foreground">{entry.output}</pre>}
      </div>
    </li>
  );
};

export const ActivityPanel = ({ session, tab }: PanelProps) => {
  const key = `${session}/${tab.tab}`;
  const name = tabName(session, tab.tab);
  const entries = useActivity(session, tab.tab);
  const [clearedAt, setClearedAt] = useState(() => cleared.get(key) ?? null);
  const [follow, setFollow] = useState(true);
  const [expanded, setExpanded] = useState<number | null>(null);
  const followButton = useRef<HTMLButtonElement>(null);
  const past = useViewingPast(session, tab.tab);
  const following = follow && !past;
  const hidden = clearedAt && entries[0]?.time === clearedAt.first ? clearedAt.count : 0;
  const shown = entries.slice(hidden);

  const scrolled = (event: UIEvent<HTMLOListElement>) => {
    const list = event.currentTarget;
    const atLatest = list.scrollHeight - list.scrollTop - list.clientHeight < FOLLOWING_WITHIN_PX;
    if (atLatest !== follow) setFollow(atLatest);
  };

  const clear = () => {
    const first = entries[0];
    if (!first) return;
    const at = { first: first.time, count: entries.length };
    cleared.set(key, at);
    setClearedAt(at);
    setExpanded(null);
    announce("Activity cleared");
  };

  const followLatest = () => {
    setFollow(true);
    goLive(session, tab.tab);
  };

  const jumpToLatest = () => {
    followLatest();
    followButton.current?.focus();
  };

  const toggle = (index: number) => {
    setFollow(false);
    setExpanded(expanded === index ? null : index);
    const entry = entries[index];
    if (expanded !== index && entry?.frame !== undefined) showMoment(session, tab.tab, entry.frame, endOf(entry));
  };

  return (
    <>
      <PanelHead title="Activity" count={shown.length}>
        <button
          ref={followButton}
          type="button"
          aria-pressed={following}
          onClick={() => (following ? setFollow(false) : followLatest())}
          className={HEAD_TOGGLE}
          title="Follow the newest call"
        >
          <ArrowDownToLine aria-hidden="true" />
          Follow
        </button>
        <button type="button" onClick={clear} disabled={shown.length === 0} className={HEAD_TOGGLE} title="Clear activity">
          <Trash2 aria-hidden="true" />
          <span className="sr-only">Clear activity</span>
        </button>
      </PanelHead>
      {shown.length === 0 ? (
        <p className="py-8 text-center text-xs text-muted-foreground">Waiting for calls...</p>
      ) : (
        <div className="relative min-h-0 flex-1">
          <ol ref={stickToLatest} data-follow={String(following)} onScroll={scrolled} aria-label={`Calls to ${name}`} className="h-full overflow-y-auto">
            {shown.map((entry, index) => (
              <ActivityRow
                key={hidden + index}
                session={session}
                entry={entry}
                expanded={expanded === hidden + index}
                onToggle={() => toggle(hidden + index)}
              />
            ))}
          </ol>
          <button
            type="button"
            hidden={following}
            onClick={jumpToLatest}
            className="absolute bottom-3 left-1/2 flex h-6 -translate-x-1/2 items-center gap-1 rounded-4xl border border-border bg-popover px-2.5 text-[11px] text-popover-foreground shadow-md outline-none hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring/50 [&_svg]:size-3"
          >
            <ArrowDown aria-hidden="true" />
            Jump to latest
          </button>
        </div>
      )}
    </>
  );
};
