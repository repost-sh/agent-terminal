import { moreStyled, UNSTYLED, type Run, type Styling } from "./ansi";
import { tabFeedPath } from "./api";
import { useFeed, type Received } from "./feed";

export interface Line { text: string; runs: Run[] }

export type Item = { kind: "line"; line: Line } | { kind: "mark"; name: string };

type Segment = { text: string } | { mark: string };

type OutputUpdate = { type: "output"; reset: boolean; styled: Segment[]; raw: Segment[] } | { type: "closed" };

interface Plain { items: Item[]; open: Styling }

interface Raw { items: Item[]; open: string }

export interface OutputDoc { plain: Plain; raw: Raw }

const lineOf = (runs: Run[]): Item => ({ kind: "line", line: { runs, text: runs.map((run) => run.text).join("") } });

const plainText = (plain: Plain, text: string): Plain => {
  const styled = moreStyled(plain.open, text);
  const closed = styled.lines.slice(0, -1).map(lineOf);

  return { items: closed.length ? [...plain.items, ...closed] : plain.items, open: { lines: styled.lines.slice(-1), style: styled.style } };
};

const plainMark = (plain: Plain, name: string): Plain => {
  const open = plain.open.lines.at(-1) ?? [];
  const closed = open.length ? [lineOf(open)] : [];

  return { items: [...plain.items, ...closed, { kind: "mark", name }], open: { lines: [], style: plain.open.style } };
};

const pictured = (text: string) =>
  Array.from(text)
    .map((char) => {
      const code = char.charCodeAt(0);

      if (code < 0x20) {
        return String.fromCharCode(0x2400 + code);
      }

      return code === 0x7f ? "␡" : char;
    })
    .join("");

const rawLine = (text: string): Item => lineOf([{ text, style: {} }]);

const rawText = (raw: Raw, text: string): Raw => {
  const closed = text
    .split("\n")
    .slice(0, -1)
    .map((part, n) => rawLine(`${n === 0 ? raw.open : ""}${pictured(part)}␊`));
  const last = pictured(text.slice(text.lastIndexOf("\n") + 1));

  return { items: closed.length ? [...raw.items, ...closed] : raw.items, open: closed.length ? last : raw.open + last };
};

const rawMark = (raw: Raw, name: string): Raw => ({
  items: [...raw.items, ...(raw.open ? [rawLine(raw.open)] : []), { kind: "mark", name }],
  open: "",
});

const EMPTY: OutputDoc = { plain: { items: [], open: UNSTYLED }, raw: { items: [], open: "" } };

interface Segments<T> { segments: Segment[]; text: (lines: T, text: string) => T; mark: (lines: T, name: string) => T }

const applied = <T>(lines: T, stream: Segments<T>) =>
  stream.segments.reduce((next, segment) => ("mark" in segment ? stream.mark(next, segment.mark) : stream.text(next, segment.text)), lines);

const streamed = (doc: OutputDoc, received: Received<OutputUpdate>): OutputDoc => {
  const update = received.message;

  if (update.type !== "output") {
    return doc;
  }

  const from = received.first || update.reset ? EMPTY : doc;

  return {
    plain: applied(from.plain, { segments: update.styled, text: plainText, mark: plainMark }),
    raw: applied(from.raw, { segments: update.raw, text: rawText, mark: rawMark }),
  };
};

export const plainItems = ({ items, open }: Plain): Item[] => {
  const last = open.lines.at(-1) ?? [];

  return last.length ? [...items, lineOf(last)] : items;
};

export const rawItems = ({ items, open }: Raw): Item[] => (open ? [...items, rawLine(open)] : items);

export const useOutput = (session: string, tab: string) =>
  useFeed(tabFeedPath({ session, tab }, "output"), { initial: EMPTY, reduce: streamed }).value;
