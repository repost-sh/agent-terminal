import type { TabInfo } from "../lib/protocol";
import type { Shown, StreamState } from "../lib/stream";
import { Badge, Dot, type Tone } from "./ui";

interface StatusBarProps {
  tab: TabInfo;
  state: StreamState;
  shown: Shown | null;
}

const modesOf = (shown: Shown) =>
  [shown.alt_screen && "alt-screen", shown.mouse && `mouse ${shown.mouse}`, shown.bracketed_paste && "bracketed paste"].filter(
    (mode): mode is string => Boolean(mode),
  );

interface Status { tone: Tone; label: string }

const isWaiting = (shown: Shown | null) => !shown || (shown.frame === 0 && !shown.exited);

const statusOf = (props: StatusBarProps): Status => {
  const { shown } = props;

  if (props.state === "reconnecting") {
    return { tone: "warning", label: "Reconnecting…" };
  }

  if (!shown || isWaiting(shown)) {
    return { tone: "success", label: "Connected" };
  }

  return shown.exited ? { tone: "muted", label: "Exited" } : { tone: "success", label: "Live" };
};

const sizeChip = (props: StatusBarProps) => {
  const cols = props.shown?.cols ?? props.tab.cols;
  const rows = props.shown?.rows ?? props.tab.rows;

  return `${String(cols)} × ${String(rows)}`;
};

const chipsOf = (props: StatusBarProps) => {
  const { shown } = props;
  const size = sizeChip(props);

  if (!shown) {
    return [size];
  }

  const frame = !isWaiting(shown) && props.state === "open" ? [`frame ${String(shown.frame)}`] : [];

  return [size, ...modesOf(shown), ...frame];
};

export const StatusBar = (props: StatusBarProps) => {
  const status = statusOf(props);

  return (
    <div id="status-region" role="group" aria-label="Status" className="flex h-9 shrink-0 items-center gap-x-2 px-3">
      <Dot tone={status.tone} />
      <span className="shrink-0 text-xs text-muted-foreground">{status.label}</span>
      <ul aria-label="Details" className="ml-auto flex shrink-0 items-center gap-2">
        {chipsOf(props).map((chip) => (
          <li key={chip} className="contents">
            <Badge variant="outline">{chip}</Badge>
          </li>
        ))}
      </ul>
    </div>
  );
};
