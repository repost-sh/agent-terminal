import { tabStreamUrl } from "../lib/api";
import type { TabInfo } from "../lib/protocol";
import type { Shown, StreamState } from "../lib/stream";
import { Badge, Dot, type Tone } from "./ui";

type StatusBarProps = {
  session: string;
  tab: TabInfo;
  state: StreamState;
  shown: Shown | null;
  hint?: string | null;
};

const modesOf = (shown: Shown) =>
  [shown.alt_screen && "alt-screen", shown.mouse && `mouse ${shown.mouse}`, shown.bracketed_paste && "bracketed paste"].filter(
    (mode): mode is string => Boolean(mode),
  );

export const StatusBar = ({ session, tab, state, shown, hint = null }: StatusBarProps) => {
  const waiting = !shown || (shown.frame === 0 && !shown.exited);
  const [tone, label]: [Tone, string] =
    state === "reconnecting"
      ? ["warning", "Reconnecting…"]
      : waiting
        ? ["success", "Connected"]
        : shown?.exited
          ? ["muted", "Exited"]
          : ["success", "Live"];
  const cols = shown?.cols ?? tab.cols;
  const rows = shown?.rows ?? tab.rows;
  const chips = [`${cols} × ${rows}`, ...(shown ? modesOf(shown) : []), ...(shown && !waiting && state === "open" ? [`frame ${shown.frame}`] : [])];
  return (
    <div id="status-region" className="flex shrink-0 flex-wrap items-center gap-x-2 gap-y-1 px-3 py-2">
      <Dot tone={tone} />
      <span className="text-xs text-muted-foreground">{label}</span>
      <span className="font-mono text-xs break-all text-muted-foreground/60">{tabStreamUrl(session, tab.tab)}</span>
      <div className="ml-auto flex flex-wrap items-center gap-2">
        {hint && (
          <span id="control-hint" className="text-[10px] text-muted-foreground">
            {hint}
          </span>
        )}
        {chips.map((chip) => (
          <Badge key={chip} variant="outline">
            {chip}
          </Badge>
        ))}
      </div>
    </div>
  );
};
