import { ChevronLeft, ChevronRight, Copy, GitCompare } from "lucide-react";
import { Suspense, use, useId, useState, type ChangeEvent, type CSSProperties } from "react";

import { screenshotUrl } from "../lib/api";
import { focusNext, focusWhenShown } from "../lib/focus";
import { clockTime } from "../lib/format";
import type { CellBox, Screenshot } from "../lib/protocol";
import { comparisonOf, fileName, useScreenshots, type Comparison } from "../lib/screenshots";
import { Dialog, showDialog } from "./dialog";
import { Button, IconButton, cn, copy } from "./ui";

const SINGLE_HEIGHT = "h-[min(58vh,34rem)]";
const PAIR_HEIGHT = "h-[min(40vh,22rem)]";

const share = (part: number, whole: number) => `${String((part / whole) * 100)}%`;

interface FigureProps { session: string; shot: Screenshot; height: string; boxes: CellBox[] }

type FrameSize = CSSProperties & Record<"--shot-width" | "--shot-height", string>;

const frameSize = (shot: Screenshot): FrameSize => ({ "--shot-width": String(shot.width), "--shot-height": String(shot.height) });

interface ChangedProps { shot: Screenshot; boxes: CellBox[] }

const ChangedBoxes = ({ shot, boxes }: ChangedProps) =>
  boxes.map((box, index) => (
    <span
      key={index}
      data-testid="changed-cells"
      className="shot-changed absolute"
      style={{
        left: share(box.x, shot.width),
        top: share(box.y, shot.height),
        width: share(box.width, shot.width),
        height: share(box.height, shot.height),
      }}
    />
  ));

const Figure = (props: FigureProps) => {
  const { session, shot } = props;
  const file = fileName(shot.path);

  return (
    <figure className="grid min-w-0 gap-1.5">
      <div className={cn("term-box flex items-center justify-center rounded-md bg-background", props.height)}>
        <div className="term-frame shot-frame relative" style={frameSize(shot)}>
          <img src={screenshotUrl(session, shot.path)} alt={`${file}, frame ${String(shot.frame)}`} className="block size-full" />
          <ChangedBoxes shot={shot} boxes={props.boxes} />
        </div>
      </div>
      <figcaption className="font-mono text-[11px] break-all">
        {file}{" "}
        <span className="text-muted-foreground">
          · {shot.tab} · frame {shot.frame} ·{" "}
          <time dateTime={shot.time} className="tabular-nums">
            {clockTime(Date.parse(shot.time))}
          </time>{" "}
          · {shot.by ?? "dashboard"}
        </span>
      </figcaption>
    </figure>
  );
};

interface PairProps { session: string; older: Screenshot; newer: Screenshot }

interface ComparedProps extends PairProps { comparison: Comparison | null }

const counted = (comparison: Comparison | null) => {
  if (comparison === null) {
    return "";
  }

  if ("error" in comparison) {
    return comparison.error;
  }

  if (comparison.changed === null) {
    return "Different sizes: no cell comparison";
  }

  return `${String(comparison.changed)} ${comparison.changed === 1 ? "cell" : "cells"} changed`;
};

const Pair = (props: ComparedProps) => {
  const { comparison } = props;
  const boxes = comparison && "boxes" in comparison ? comparison.boxes : [];

  return (
    <div className="@container grid gap-2">
      <p className={cn("text-xs text-muted-foreground", comparison !== null && "error" in comparison && "text-destructive")} role="status">
        {counted(comparison)}
      </p>
      <div className="grid grid-cols-1 gap-3 @2xl:grid-cols-2">
        <Figure session={props.session} shot={props.older} height={PAIR_HEIGHT} boxes={boxes} />
        <Figure session={props.session} shot={props.newer} height={PAIR_HEIGHT} boxes={boxes} />
      </div>
    </div>
  );
};

const ComparedPair = (props: PairProps) => <Pair {...props} comparison={use(comparisonOf(props.session, { older: props.older, newer: props.newer }))} />;

interface Listed { shot: Screenshot; index: number }

const pairOf = (current: Listed, other: Listed) =>
  other.index < current.index ? { older: other.shot, newer: current.shot } : { older: current.shot, newer: other.shot };

interface LightboxProps { session: string; opened: Screenshot }

interface Stepping { shots: Screenshot[]; index: number; setPath: (path: string) => void }

const stepThrough = (stepping: Stepping) => (by: number) => {
  const to = stepping.shots[stepping.index + by];

  if (!to) {
    return;
  }

  if (stepping.index + by === 0) {
    focusNext("shot-next");
  }

  if (stepping.index + by === stepping.shots.length - 1) {
    focusNext("shot-prev");
  }

  stepping.setPath(to.path);
};

const useLightbox = (props: LightboxProps) => {
  const shots = useScreenshots(props.session).value;
  const [path, setPath] = useState(props.opened.path);
  const [picking, setPicking] = useState(false);
  const [against, setAgainst] = useState<string | null>(null);
  const index = shots.findIndex((shot) => shot.path === path);
  const otherIndex = shots.findIndex((other) => other.path === against);
  const otherShot = shots[otherIndex];

  return {
    shots,
    current: { shot: shots[index] ?? props.opened, index },
    other: otherShot && { shot: otherShot, index: otherIndex },
    picking,
    step: stepThrough({ shots, index, setPath }),
    pick: () => {
      focusNext("shot-compare");
      setPicking(true);
    },
    compare: (event: ChangeEvent<HTMLSelectElement>) => {
      if (!event.target.value) {
        return;
      }

      focusNext("shot-back");
      setAgainst(event.target.value);
      setPicking(false);
    },
    back: () => {
      setAgainst(null);
    },
  };
};

type LightboxState = ReturnType<typeof useLightbox>;

interface StateProps { box: LightboxState; session: string }

const LightboxBody = ({ box, session }: StateProps) => {
  if (!box.other) {
    return <Figure session={session} shot={box.current.shot} height={SINGLE_HEIGHT} boxes={[]} />;
  }

  const pair = pairOf(box.current, box.other);

  return (
    <Suspense fallback={<Pair session={session} {...pair} comparison={null} />}>
      <ComparedPair session={session} {...pair} />
    </Suspense>
  );
};

interface PickerProps { box: LightboxState }

const ComparePicker = ({ box }: PickerProps) => {
  const selectId = useId();

  return (
    <>
      <label className="sr-only" htmlFor={selectId}>
        Compare with
      </label>
      <select
        id={selectId}
        ref={focusWhenShown("shot-compare")}
        defaultValue=""
        onChange={box.compare}
        className="h-7 rounded-md border border-border bg-transparent px-2 font-mono text-xs outline-none focus:ring-2 focus:ring-ring"
      >
        <option value="">Compare with…</option>
        {box.shots.map((candidate) =>
          candidate.path === box.current.shot.path ? null : (
            <option key={candidate.path} value={candidate.path}>
              {fileName(candidate.path)}
            </option>
          ),
        )}
      </select>
    </>
  );
};

const ComparePart = ({ box }: PickerProps) => {
  if (box.other) {
    return (
      <Button look={{ variant: "outline" }} ref={focusWhenShown("shot-back")} onClick={box.back}>
        Back to screenshot
      </Button>
    );
  }

  return box.picking ? (
    <ComparePicker box={box} />
  ) : (
    <Button look={{ variant: "outline", icon: GitCompare }} disabled={box.shots.length < 2} onClick={box.pick}>
      Compare with…
    </Button>
  );
};

const position = (box: LightboxState) => (box.current.index < 0 ? "" : `${String(box.current.index + 1)} of ${String(box.shots.length)}`);

const Stepper = ({ box }: PickerProps) => {
  const comparing = box.other !== undefined;
  const at = box.current.index;

  return (
    <div role="group" aria-label="Step through the screenshots" className="flex flex-1 items-center gap-1">
      <IconButton look={{ label: "Previous screenshot", icon: ChevronLeft }} ref={focusWhenShown("shot-prev")} disabled={at <= 0 || comparing} onClick={() => {
        box.step(-1);
      }} />
      <span className="text-xs tabular-nums text-muted-foreground">{position(box)}</span>
      <IconButton look={{ label: "Next screenshot", icon: ChevronRight }} ref={focusWhenShown("shot-next")} disabled={at < 0 || at === box.shots.length - 1 || comparing} onClick={() => {
        box.step(1);
      }} />
    </div>
  );
};

const Lightbox = (props: LightboxProps) => {
  const box = useLightbox(props);
  const shot = box.current.shot;
  const footer = (
    <>
      <Stepper box={box} />
      <Button look={{ variant: "ghost", icon: Copy }} onClick={() => {
        void copy(shot.path);
      }}>
        Copy path
      </Button>
      <ComparePart box={box} />
    </>
  );
  const title = (
    <>
      <span className="font-mono">{fileName(shot.path)}</span>
      {box.other && " compared"}
    </>
  );

  return (
    <Dialog title={title} width={72} footer={footer}>
      <LightboxBody box={box} session={props.session} />
    </Dialog>
  );
};

export const openLightbox = (session: string, shot: Screenshot) => {
  showDialog(() => <Lightbox session={session} opened={shot} />);
};
