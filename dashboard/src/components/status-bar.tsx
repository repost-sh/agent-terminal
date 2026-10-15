import { tabStreamUrl } from "../lib/api";
import type { TabInfo } from "../lib/protocol";
import type { Shown, StreamState } from "../lib/stream";
import { Badge, Dot, type Tone } from "./ui";

interface StatusBarProps {
  session: string;
  tab: TabInfo;
  state: StreamState;
  shown: Shown | null;
  hint?: string | null;
}

const modesOf = (shown: Shown) =>
  [shown.alt_screen && "alt-screen", shown.mouse && `mouse ${shown.mouse}`, shown.bracketed_paste && "bracketed paste"].filter(
    (mode): mode is string => Boolean(mode),
  );

export const controlHint = (typing: boolean, program: string) => (typing ? `Keys go to ${program} · ctrl+p ctrl+q moves focus out` : null);

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
  const hint = props.hint ?? null;

  return (
    <div id="status-region" className="flex shrink-0 flex-wrap items-center gap-x-2 gap-y-1 px-3 py-2">
      <Dot tone={status.tone} />
      <span className="text-xs text-muted-foreground">{status.label}</span>
      <span className="font-mono text-xs break-all text-muted-foreground/60">{tabStreamUrl(props.session, props.tab.tab)}</span>
      <div className="ml-auto flex flex-wrap items-center gap-2">
        {hint && (
          <span id="control-hint" className="text-[10px] text-muted-foreground">
            {hint}
          </span>
        )}
        {chipsOf(props).map((chip) => (
          <Badge key={chip} variant="outline">
            {chip}
          </Badge>
        ))}
      </div>
    </div>
  );
};
