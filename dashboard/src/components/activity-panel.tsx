import { ArrowDown, ArrowDownToLine, LoaderCircle, Trash2 } from "lucide-react";
import { useRef, useState, type UIEvent } from "react";

import { activityRows, resultOf, useActivity, type ActivityRow as ActivityRowData } from "../lib/activity";
import { screenshotUrl } from "../lib/api";
import { clockMs, formatDuration } from "../lib/format";
import type { PendingCall, Screenshot } from "../lib/protocol";
import { fileName, useScreenshots } from "../lib/screenshots";
import { tabKey, tabName } from "../lib/tabs";
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

interface Cleared { first: string; count: number }

const cleared = new Map<string, Cleared>();

const stickToLatest = (list: HTMLOListElement | null) => {
  if (!list) {
    return;
  }

  const toLatest = () => {
    if (list.dataset["follow"] === "true") {
      list.scrollTop = list.scrollHeight;
    }
  };

  toLatest();
  const watcher = new MutationObserver(toLatest);

  watcher.observe(list, { childList: true, subtree: true, attributes: true, attributeFilter: ["data-follow"] });

  return () => {
    watcher.disconnect();
  };
};

const THUMBNAIL =
  "block w-40 max-w-full overflow-hidden rounded-sm ring-1 ring-border outline-none hover:ring-ring focus-visible:ring-2 focus-visible:ring-ring";

interface ThumbnailProps { session: string; path: string; shot: Screenshot | undefined }

const Thumbnail = (props: ThumbnailProps) => {
  const { shot } = props;
  const picture = (
    <>
      <span className="sr-only">Open screenshot {fileName(props.path)}</span>
      <img src={screenshotUrl(props.session, props.path)} alt="" className="block w-full" />
    </>
  );

  return (
    <div className="mb-1.5 flex gap-2">
      <span className="invisible shrink-0 font-mono text-xs" aria-hidden="true">
        00:00:00.000
      </span>
      {shot ? (
        <button type="button" onClick={() => {
          openLightbox(props.session, shot);
        }} className={THUMBNAIL}>
          {picture}
        </button>
      ) : (
        <a href={screenshotUrl(props.session, props.path)} target="_blank" rel="noreferrer" className={THUMBNAIL}>
          {picture}
        </a>
      )}
    </div>
  );
};

interface ThumbnailsProps { session: string; paths: string[] }

const Thumbnails = ({ session, paths }: ThumbnailsProps) => {
  const shots = useScreenshots(session).value;

  return (
    <>
      {paths
        .filter((path) => path.toLowerCase().endsWith(".png"))
        .map((path) => (
          <Thumbnail key={path} session={session} path={path} shot={shots.find((listed) => listed.path === path)} />
        ))}
    </>
  );
};

interface RowProps { session: string; row: ActivityRowData; expanded: boolean; onToggle: () => void }

interface CallTextProps { row: ActivityRowData }

const CallText = ({ row }: CallTextProps) => {
  const entry = row.entry;
  const result = resultOf(entry);

  return (
    <span className="min-w-0 flex-1 font-mono [overflow-wrap:anywhere]">
      {row.human === null ? (
        <>
          {entry.command} <span className="whitespace-nowrap text-muted-foreground">{formatDuration(entry.duration_ms)}</span>
        </>
      ) : (
        <>
          <span className="text-violet-600 dark:text-violet-400">human:</span> {row.human}
        </>
      )}
      {result && <span className={cn("mt-0.5 flex items-center gap-1", RESULT_TONE[result.tone].text)}>{result.label}</span>}
    </span>
  );
};

const RowDetails = (props: RowProps) => {
  const entry = props.row.entry;

  return (
    <>
      {entry.screenshots && <Thumbnails session={props.session} paths={entry.screenshots} />}
      {props.expanded && entry.output !== "" && (
        <pre className="mb-1.5 max-h-48 overflow-auto text-[10px] leading-snug text-muted-foreground">{entry.output}</pre>
      )}
    </>
  );
};

const rowClass = (props: RowProps) => {
  const result = resultOf(props.row.entry);

  return cn("border-b border-border", result && RESULT_TONE[result.tone].row, props.row.human !== null && "bg-violet-500/5", props.expanded && "bg-muted/60");
};

const ActivityRow = (props: RowProps) => {
  const entry = props.row.entry;
  const opens = entry.output !== "";

  return (
    <li className={rowClass(props)}>
      <div className="px-3">
        <button
          type="button"
          aria-expanded={opens ? props.expanded : undefined}
          onClick={props.onToggle}
          className="flex w-full items-start gap-2 rounded-sm py-1.5 text-left text-xs outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
        >
          <span className="shrink-0 font-mono text-muted-foreground">
            <time dateTime={entry.time} className="tabular-nums">
              {clockMs(entry.time)}
            </time>
          </span>
          <CallText row={props.row} />
          {opens && (
            <span className="ml-auto shrink-0 font-mono text-muted-foreground" aria-hidden="true">
              {props.expanded ? "-" : "+"}
            </span>
          )}
        </button>
        <RowDetails {...props} />
      </div>
    </li>
  );
};

interface PendingRowProps { call: PendingCall }

const PendingRow = ({ call }: PendingRowProps) => (
  <li aria-busy="true" className="border-b border-border">
    <div className="flex w-full items-start gap-2 px-3 py-1.5 text-xs">
      <span className="shrink-0 font-mono text-muted-foreground">
        <time dateTime={call.time} className="tabular-nums">
          {clockMs(call.time)}
        </time>
      </span>
      <span className="min-w-0 flex-1 font-mono [overflow-wrap:anywhere]">
        {call.command}
        <span className="mt-0.5 flex items-center gap-1 text-muted-foreground">
          <LoaderCircle aria-hidden="true" className="size-[11px] animate-spin" />
          waiting
        </span>
      </span>
    </div>
  </li>
);

const useActivityList = (session: string, tab: string) => {
  const key = tabKey(session, tab);
  const activity = useActivity(session, tab);
  const [clearedAt, setClearedAt] = useState(() => cleared.get(key) ?? null);
  const [follow, setFollow] = useState(true);
  const [expanded, setExpanded] = useState<number | null>(null);
  const followButton = useRef<HTMLButtonElement>(null);
  const hidden = clearedAt && activity.entries[0]?.time === clearedAt.first ? clearedAt.count : 0;
  const clear = () => {
    const first = activity.entries[0];

    if (!first) {
      return;
    }

    const at = { first: first.time, count: activity.entries.length };

    cleared.set(key, at);
    setClearedAt(at);
    setExpanded(null);
    announce("Activity cleared");
  };

  return { activity, shown: activityRows(activity.entries.slice(hidden), hidden), follow, setFollow, expanded, setExpanded, followButton, clear };
};

type ActivityList = ReturnType<typeof useActivityList>;

interface ListProps { list: ActivityList }

const ActivityHead = ({ list }: ListProps) => (
  <PanelHead title="Activity" count={list.shown.length + list.activity.pending.length}>
    <button
      ref={list.followButton}
      type="button"
      aria-pressed={list.follow}
      onClick={() => {
        list.setFollow(!list.follow);
      }}
      className={HEAD_TOGGLE}
      title="Follow the newest call"
    >
      <ArrowDownToLine aria-hidden="true" />
      Follow
    </button>
    <button type="button" onClick={list.clear} disabled={list.shown.length === 0} className={HEAD_TOGGLE} title="Clear activity">
      <Trash2 aria-hidden="true" />
      <span className="sr-only">Clear activity</span>
    </button>
  </PanelHead>
);

interface CallsProps { list: ActivityList; session: string; name: string }

const Calls = (props: CallsProps) => {
  const { list } = props;
  const scrolled = (event: UIEvent<HTMLOListElement>) => {
    const element = event.currentTarget;
    const atLatest = element.scrollHeight - element.scrollTop - element.clientHeight < FOLLOWING_WITHIN_PX;

    if (atLatest !== list.follow) {
      list.setFollow(atLatest);
    }
  };

  const toggle = (index: number) => {
    list.setFollow(false);
    list.setExpanded(list.expanded === index ? null : index);
  };

  return (
    <ol ref={stickToLatest} data-follow={String(list.follow)} onScroll={scrolled} aria-label={`Calls to ${props.name}`} className="h-full overflow-y-auto">
      {list.shown.map((row) => (
        <ActivityRow key={row.at} session={props.session} row={row} expanded={list.expanded === row.at} onToggle={() => {
          toggle(row.at);
        }} />
      ))}
      {list.activity.pending.map((call, index) => (
        <PendingRow key={`${call.time} ${String(index)}`} call={call} />
      ))}
    </ol>
  );
};

const JumpToLatest = ({ list }: ListProps) => (
  <button
    type="button"
    hidden={list.follow}
    onClick={() => {
      list.setFollow(true);
      list.followButton.current?.focus();
    }}
    className="absolute bottom-3 left-1/2 flex h-6 -translate-x-1/2 items-center gap-1 rounded-4xl border border-border bg-popover px-2.5 text-[11px] text-popover-foreground shadow-md outline-none hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring/50 [&_svg]:size-3"
  >
    <ArrowDown aria-hidden="true" />
    Jump to latest
  </button>
);

export const ActivityPanel = ({ session, tab }: PanelProps) => {
  const list = useActivityList(session, tab.tab);

  return (
    <>
      <ActivityHead list={list} />
      {list.shown.length + list.activity.pending.length === 0 ? (
        <p className="py-8 text-center text-xs text-muted-foreground">Waiting for calls...</p>
      ) : (
        <div className="relative min-h-0 flex-1">
          <Calls list={list} session={session} name={tabName(session, tab.tab)} />
          <JumpToLatest list={list} />
        </div>
      )}
    </>
  );
};
