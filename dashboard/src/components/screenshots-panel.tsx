import { screenshotUrl } from "../lib/api";
import { clockTime } from "../lib/format";
import type { Screenshot } from "../lib/protocol";
import { fileName, useScreenshots } from "../lib/screenshots";
import type { PanelProps } from "../panels";
import { EmptyNote, PanelHead } from "./panel-parts";
import { openLightbox } from "./screenshot-lightbox";

interface ShotProps { session: string; shot: Screenshot }

const Shot = ({ session, shot }: ShotProps) => (
  <li>
    <button
      type="button"
      onClick={() => {
        openLightbox(session, shot);
      }}
      className="flex w-full flex-col gap-1 rounded-md p-1.5 text-left outline-none hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring/50"
    >
      <span className="block w-full overflow-hidden rounded-sm" style={{ aspectRatio: `${String(shot.width)} / ${String(shot.height)}` }}>
        <img src={screenshotUrl(session, shot.path)} alt="" className="block size-full" />
      </span>
      <span className="mt-0.5 font-mono text-[11px] break-all">{fileName(shot.path)}</span>
      <span className="text-[10px] text-muted-foreground">
        {shot.tab} · frame {shot.frame} ·{" "}
        <time dateTime={shot.time} className="tabular-nums">
          {clockTime(Date.parse(shot.time))}
        </time>
      </span>
      <span className="font-mono text-[10px] break-all text-muted-foreground">{shot.by ?? "dashboard"}</span>
    </button>
  </li>
);

export const ScreenshotsPanel = ({ session }: PanelProps) => {
  const { status, value: shots } = useScreenshots(session);

  return (
    <>
      <PanelHead title="Screenshots" count={shots.length} />
      {shots.length > 0 ? (
        <ul
          aria-label={`Screenshots of ${session}`}
          className="grid min-h-0 flex-1 grid-cols-[repeat(auto-fill,minmax(9rem,1fr))] content-start gap-2 overflow-y-auto p-2"
        >
          {shots.map((shot) => (
            <Shot key={shot.path} session={session} shot={shot} />
          ))}
        </ul>
      ) : (
        status !== "connecting" && <EmptyNote title="No screenshots yet" />
      )}
    </>
  );
};
