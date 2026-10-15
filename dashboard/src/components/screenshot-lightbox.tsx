import { ChevronLeft, ChevronRight, Copy, GitCompare } from "lucide-react";
import { Suspense, use, useId, useState, type ChangeEvent } from "react";

import { screenshotUrl } from "../lib/api";
import { focusNext, focusWhenShown } from "../lib/focus";
import { clockTime } from "../lib/format";
import type { CellBox, Screenshot } from "../lib/protocol";
import { comparisonOf, fileName, useScreenshots, type Comparison } from "../lib/screenshots";
import { Dialog, showDialog } from "./dialog";
import { Button, IconButton, cn, copy } from "./ui";

const SINGLE_HEIGHT = "min(58vh, 34rem)";
const PAIR_HEIGHT = "min(40vh, 22rem)";

const share = (part: number, whole: number) => `${(part / whole) * 100}%`;

type FigureProps = { session: string; shot: Screenshot; height: string; boxes: CellBox[] };

const Figure = ({ session, shot, height, boxes }: FigureProps) => {
  const file = fileName(shot.path);
  return (
    <figure className="grid min-w-0 gap-1.5">
      <div className="term-box flex items-center justify-center rounded-md bg-background" style={{ height }}>
        <div
          className="term-frame relative"
          style={{ width: `min(${shot.width}px, 100cqw, calc(100cqh * ${shot.width} / ${shot.height}))`, aspectRatio: `${shot.width} / ${shot.height}` }}
        >
          <img src={screenshotUrl(session, shot.path)} alt={`${file}, frame ${shot.frame}`} className="block size-full" />
          {boxes.map((box) => (
            <span
              key={`${box.x},${box.y}`}
              data-changed=""
              className="shot-changed absolute"
              style={{
                left: share(box.x, shot.width),
                top: share(box.y, shot.height),
                width: share(box.width, shot.width),
                height: share(box.height, shot.height),
              }}
            />
          ))}
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

type PairProps = { session: string; older: Screenshot; newer: Screenshot };

const counted = (comparison: Comparison | null) => {
  if (comparison === null) return "";
  if ("error" in comparison) return comparison.error;
  if (comparison.changed === null) return "Different sizes: no cell comparison";
  return `${comparison.changed} ${comparison.changed === 1 ? "cell" : "cells"} changed`;
};

const Pair = ({ session, older, newer, comparison }: PairProps & { comparison: Comparison | null }) => {
  const boxes = comparison && "boxes" in comparison ? comparison.boxes : [];
  return (
    <div className="@container grid gap-2">
      <p className={cn("text-xs text-muted-foreground", comparison !== null && "error" in comparison && "text-destructive")} role="status">
        {counted(comparison)}
      </p>
      <div className="grid grid-cols-1 gap-3 @2xl:grid-cols-2">
        <Figure session={session} shot={older} height={PAIR_HEIGHT} boxes={boxes} />
        <Figure session={session} shot={newer} height={PAIR_HEIGHT} boxes={boxes} />
      </div>
    </div>
  );
};

const ComparedPair = (props: PairProps) => <Pair {...props} comparison={use(comparisonOf(props.session, props.older, props.newer))} />;

const pairOf = (shot: Screenshot, index: number, other: Screenshot, otherIndex: number) =>
  otherIndex < index ? { older: other, newer: shot } : { older: shot, newer: other };

type LightboxProps = { session: string; opened: Screenshot };

const Lightbox = ({ session, opened }: LightboxProps) => {
  const selectId = useId();
  const shots = useScreenshots(session).value;
  const [path, setPath] = useState(opened.path);
  const [picking, setPicking] = useState(false);
  const [against, setAgainst] = useState<string | null>(null);
  const index = shots.findIndex((shot) => shot.path === path);
  const shot = shots[index] ?? opened;
  const otherIndex = shots.findIndex((other) => other.path === against);
  const other = shots[otherIndex];
  const file = fileName(shot.path);

  const step = (by: number) => {
    const to = shots[index + by];
    if (!to) return;
    const last = index + by === 0 || index + by === shots.length - 1;
    if (last) focusNext(by < 0 ? "shot-next" : "shot-prev");
    setPath(to.path);
  };
  const pick = () => {
    focusNext("shot-compare");
    setPicking(true);
  };
  const compare = (event: ChangeEvent<HTMLSelectElement>) => {
    if (!event.target.value) return;
    focusNext("shot-back");
    setAgainst(event.target.value);
    setPicking(false);
  };
  const back = () => {
    focusNext("shot-pick");
    setAgainst(null);
  };

  const body = other ? (
    <Suspense fallback={<Pair session={session} {...pairOf(shot, index, other, otherIndex)} comparison={null} />}>
      <ComparedPair session={session} {...pairOf(shot, index, other, otherIndex)} />
    </Suspense>
  ) : (
    <Figure session={session} shot={shot} height={SINGLE_HEIGHT} boxes={[]} />
  );

  const comparePart = other ? (
    <Button variant="outline" ref={focusWhenShown("shot-back")} onClick={back}>
      Back to screenshot
    </Button>
  ) : picking ? (
    <>
      <label className="sr-only" htmlFor={selectId}>
        Compare with
      </label>
      <select
        id={selectId}
        ref={focusWhenShown("shot-compare")}
        defaultValue=""
        onChange={compare}
        className="h-7 rounded-md border border-border bg-transparent px-2 font-mono text-xs outline-none focus:ring-2 focus:ring-ring"
      >
        <option value="">Compare with…</option>
        {shots.map((candidate) =>
          candidate.path === shot.path ? null : (
            <option key={candidate.path} value={candidate.path}>
              {fileName(candidate.path)}
            </option>
          ),
        )}
      </select>
    </>
  ) : (
    <Button variant="outline" icon={GitCompare} ref={focusWhenShown("shot-pick")} disabled={shots.length < 2} onClick={pick}>
      Compare with…
    </Button>
  );

  const footer = (
    <>
      <div className="flex flex-1 items-center gap-1">
        <IconButton
          label="Previous screenshot"
          icon={ChevronLeft}
          ref={focusWhenShown("shot-prev")}
          disabled={index <= 0 || other !== undefined}
          onClick={() => step(-1)}
        />
        <span className="text-xs tabular-nums text-muted-foreground">{index < 0 ? "" : `${index + 1} of ${shots.length}`}</span>
        <IconButton
          label="Next screenshot"
          icon={ChevronRight}
          ref={focusWhenShown("shot-next")}
          disabled={index < 0 || index === shots.length - 1 || other !== undefined}
          onClick={() => step(1)}
        />
      </div>
      <Button variant="ghost" icon={Copy} onClick={() => copy(shot.path)}>
        Copy path
      </Button>
      {comparePart}
    </>
  );

  const title = (
    <>
      <span className="font-mono">{file}</span>
      {other && " compared"}
    </>
  );

  return (
    <Dialog title={title} width={72} footer={footer}>
      {body}
    </Dialog>
  );
};

export const openLightbox = (session: string, shot: Screenshot) => showDialog(() => <Lightbox session={session} opened={shot} />);
