import { fed, newStream, type OutputStream, type Segment } from "./ansi";
import { tabFeedPath } from "./api";
import { useFeed } from "./feed";

type OutputUpdate = { type: "output"; reset: boolean; segments: Segment[] } | { type: "closed" };

const EMPTY = newStream();

const streamed = (stream: OutputStream, update: OutputUpdate, first: boolean) =>
  update.type === "output" ? fed(first || update.reset ? newStream() : stream, update.segments) : stream;

export const useOutput = (session: string, tab: string) => useFeed(tabFeedPath(session, tab, "output"), EMPTY, streamed).value;
