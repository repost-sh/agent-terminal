import { compareScreenshots, screenshotsFeedPath } from "./api";
import { useFeed, type Received } from "./feed";
import type { Compared, Screenshot, ScreenshotsUpdate } from "./protocol";
import { settle } from "./settle";

const NONE: Screenshot[] = [];

const listed = (shots: Screenshot[], received: Received<ScreenshotsUpdate>) =>
  received.message.type === "screenshots" ? received.message.screenshots : shots;

export const useScreenshots = (session: string) => useFeed(screenshotsFeedPath(session), { initial: NONE, reduce: listed });

export const fileName = (path: string) => path.split("/").pop() ?? path;

export type Comparison = Compared | { error: string };

export interface ScreenshotPair { older: Screenshot; newer: Screenshot }

const comparisons = new Map<string, Promise<Comparison>>();

const compared = async (session: string, pair: ScreenshotPair): Promise<Comparison> => {
  const result = await settle(() => compareScreenshots(session, { older: pair.older.path, newer: pair.newer.path }));

  return result.ok ? result.value : { error: result.error instanceof Error ? result.error.message : String(result.error) };
};

export const comparisonOf = (session: string, pair: ScreenshotPair) => {
  const key = JSON.stringify([session, pair.older.path, pair.older.time, pair.newer.path, pair.newer.time]);
  const known = comparisons.get(key);

  if (known) {
    return known;
  }

  const comparing = compared(session, pair);

  comparisons.set(key, comparing);

  return comparing;
};
