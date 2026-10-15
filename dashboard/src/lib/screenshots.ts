import { compareScreenshots, screenshotsFeedPath } from "./api";
import { useFeed } from "./feed";
import type { Compared, Screenshot, ScreenshotsUpdate } from "./protocol";

const NONE: Screenshot[] = [];

const listed = (shots: Screenshot[], update: ScreenshotsUpdate) => (update.type === "screenshots" ? update.screenshots : shots);

export const useScreenshots = (session: string) => useFeed(screenshotsFeedPath(session), NONE, listed);

export const fileName = (path: string) => path.split("/").pop() ?? path;

export type Comparison = Compared | { error: string };

const comparisons = new Map<string, Promise<Comparison>>();

export const comparisonOf = (session: string, older: Screenshot, newer: Screenshot) => {
  const key = JSON.stringify([session, older.path, older.time, newer.path, newer.time]);
  const known = comparisons.get(key);
  if (known) return known;
  const comparing = compareScreenshots(session, older.path, newer.path).catch(
    (error: unknown): Comparison => ({ error: error instanceof Error ? error.message : String(error) }),
  );
  comparisons.set(key, comparing);
  return comparing;
};
