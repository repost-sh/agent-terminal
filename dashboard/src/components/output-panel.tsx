import { Search } from "lucide-react";
import { memo, useState } from "react";

import { runStyle } from "../lib/ansi";
import { plainItems, rawItems, useOutput, type Item, type Line } from "../lib/output";
import { tabName } from "../lib/tabs";
import type { PanelProps } from "./panels";
import { EmptyNote, HEAD_TOGGLE, PanelHead } from "./panel-parts";

const LINE = "min-h-[1lh] whitespace-pre-wrap break-all";

interface StyledLineProps { line: Line }

const StyledLine = memo(({ line }: StyledLineProps) => (
  <div className={LINE}>
    {line.runs.map((run, index) => (
      <span key={index} style={runStyle(run.style)}>
        {run.text}
      </span>
    ))}
  </div>
));

const occurrences = (text: string, query: string) => text.toLowerCase().split(query.toLowerCase()).length - 1;

interface FoundLineProps { text: string; query: string }

const FoundLine = ({ text, query }: FoundLineProps) => {
  const lower = text.toLowerCase();
  const wanted = query.toLowerCase();
  const parts = [];
  let from = 0;

  for (let at = lower.indexOf(wanted); at !== -1; at = lower.indexOf(wanted, from)) {
    parts.push(text.slice(from, at));
    parts.push(
      <mark key={at} className="rounded-[2px] bg-yellow-300 text-black">
        {text.slice(at, at + wanted.length)}
      </mark>,
    );
    from = at + wanted.length;
  }

  parts.push(text.slice(from));

  return <div className={LINE}>{parts}</div>;
};

interface MarkDividerProps { name: string }

const MarkDivider = ({ name }: MarkDividerProps) => (
  <div role="separator" aria-label={`mark ${name}`} className="my-1.5 flex items-center gap-2 font-sans text-[10px] text-muted-foreground">
    <span className="h-px flex-1 bg-border" />
    mark: {name}
    <span className="h-px flex-1 bg-border" />
  </div>
);

interface SearchProps { typed: string; onType: (typed: string) => void }

const SearchBox = ({ typed, onType }: SearchProps) => (
  <label className="ml-auto flex h-6 min-w-24 flex-1 items-center gap-1.5 rounded border border-border px-1.5 text-muted-foreground focus-within:ring-2 focus-within:ring-ring/50">
    <Search aria-hidden="true" className="size-3 shrink-0" />
    <span className="sr-only">Search the output</span>
    <input
      type="text"
      value={typed}
      onChange={(event) => {
        onType(event.target.value);
      }}
      placeholder="Search"
      spellCheck={false}
      autoComplete="off"
      className="min-w-0 flex-1 bg-transparent text-[11px] text-foreground outline-none placeholder:text-muted-foreground/60"
    />
  </label>
);

interface LinesProps { label: string; items: Item[]; query: string }

const OutputLines = (props: LinesProps) => (
  <div role="group" aria-label={props.label} className="min-h-0 flex-1 overflow-y-auto px-3 py-2 font-mono text-[11px] leading-relaxed">
    {props.items.map((item, index) =>
      item.kind === "mark" ? (
        <MarkDivider key={index} name={item.name} />
      ) : props.query ? (
        <FoundLine key={index} text={item.line.text} query={props.query} />
      ) : (
        <StyledLine key={index} line={item.line} />
      ),
    )}
  </div>
);

const matchCount = (items: Item[], query: string) =>
  query ? items.reduce((count, item) => count + (item.kind === "line" ? occurrences(item.line.text, query) : 0), 0) : 0;

export const OutputPanel = ({ session, tab }: PanelProps) => {
  const output = useOutput(session, tab.tab);
  const [raw, setRaw] = useState(false);
  const [typed, setTyped] = useState("");
  const query = typed.trim();
  const items = raw ? rawItems(output.raw) : plainItems(output.plain);
  const matches = matchCount(items, query);

  return (
    <>
      <PanelHead title="Output" className="flex-wrap">
        <SearchBox typed={typed} onType={setTyped} />
        {query && (
          <span role="status" className="text-[10px] whitespace-nowrap text-muted-foreground">
            {matches} {matches === 1 ? "match" : "matches"}
          </span>
        )}
        <button type="button" aria-pressed={raw} onClick={() => {
          setRaw(!raw);
        }} className={HEAD_TOGGLE} title="Show escape sequences">
          Raw
        </button>
      </PanelHead>
      {items.length === 0 ? (
        <EmptyNote title="No output yet" />
      ) : (
        <OutputLines label={`Output of ${tabName(session, tab.tab)}`} items={items} query={query} />
      )}
    </>
  );
};
