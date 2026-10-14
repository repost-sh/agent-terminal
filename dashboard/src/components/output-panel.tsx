import { Search } from "lucide-react";
import { Fragment, memo, useState, type CSSProperties } from "react";

import { itemsOf, PLAIN, type Color, type Line, type Style } from "../lib/ansi";
import { useOutput } from "../lib/output";
import { tabName } from "../lib/tabs";
import type { PanelProps } from "../panels";
import { EmptyNote, HEAD_TOGGLE, PanelHead } from "./panel-parts";

const colorOf = (color: Color) => ("index" in color ? `var(--ansi-${color.index})` : color.rgb);

const styleOf = (style: Style): CSSProperties => {
  const fg = style.fg && colorOf(style.fg);
  const bg = style.bg && colorOf(style.bg);
  const [color, background] = style.inverse ? [bg ?? "var(--background)", fg ?? "var(--foreground)"] : [fg, bg];
  const decoration = [style.underline && "underline", style.strike && "line-through"].filter(Boolean).join(" ");
  return {
    color: color ?? undefined,
    backgroundColor: background ?? undefined,
    fontWeight: style.bold ? 700 : undefined,
    opacity: style.dim ? 0.6 : undefined,
    fontStyle: style.italic ? "italic" : undefined,
    textDecorationLine: decoration || undefined,
  };
};

const LINE = "min-h-[1lh] whitespace-pre-wrap break-all";

const StyledLine = memo(({ line }: { line: Line }) => (
  <div className={LINE}>
    {line.runs.map((run, index) =>
      run.style === PLAIN ? (
        <Fragment key={index}>{run.text}</Fragment>
      ) : (
        <span key={index} style={styleOf(run.style)}>
          {run.text}
        </span>
      ),
    )}
  </div>
));

const occurrences = (text: string, query: string) => text.toLowerCase().split(query.toLowerCase()).length - 1;

const FoundLine = ({ text, query }: { text: string; query: string }) => {
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

const MarkDivider = ({ name }: { name: string }) => (
  <div role="separator" aria-label={`mark ${name}`} className="my-1.5 flex items-center gap-2 font-sans text-[10px] text-muted-foreground">
    <span className="h-px flex-1 bg-border" />
    mark: {name}
    <span className="h-px flex-1 bg-border" />
  </div>
);

export const OutputPanel = ({ session, tab }: PanelProps) => {
  const stream = useOutput(session, tab.tab);
  const [raw, setRaw] = useState(false);
  const [typed, setTyped] = useState("");
  const query = typed.trim();
  const items = itemsOf(raw ? stream.raw : stream.plain);
  const matches = query ? items.reduce((count, item) => count + (item.kind === "line" ? occurrences(item.line.text, query) : 0), 0) : 0;

  return (
    <>
      <PanelHead title="Output" className="flex-wrap">
        <label className="ml-auto flex h-6 min-w-24 flex-1 items-center gap-1.5 rounded border border-border px-1.5 text-muted-foreground focus-within:ring-2 focus-within:ring-ring/50">
          <Search aria-hidden="true" className="size-3 shrink-0" />
          <span className="sr-only">Search the output</span>
          <input
            type="text"
            value={typed}
            onChange={(event) => setTyped(event.target.value)}
            placeholder="Search"
            spellCheck={false}
            autoComplete="off"
            className="min-w-0 flex-1 bg-transparent text-[11px] text-foreground outline-none placeholder:text-muted-foreground/60"
          />
        </label>
        {query && (
          <span role="status" className="text-[10px] whitespace-nowrap text-muted-foreground">
            {matches} {matches === 1 ? "match" : "matches"}
          </span>
        )}
        <button type="button" aria-pressed={raw} onClick={() => setRaw(!raw)} className={HEAD_TOGGLE} title="Show escape sequences">
          Raw
        </button>
      </PanelHead>
      {items.length === 0 ? (
        <EmptyNote title="No output yet" />
      ) : (
        <div role="group" aria-label={`Output of ${tabName(session, tab.tab)}`} className="min-h-0 flex-1 overflow-y-auto px-3 py-2 font-mono text-[11px] leading-relaxed">
          {items.map((item, index) =>
            item.kind === "mark" ? (
              <MarkDivider key={index} name={item.name} />
            ) : query ? (
              <FoundLine key={index} text={item.line.text} query={query} />
            ) : (
              <StyledLine key={index} line={item.line} />
            ),
          )}
        </div>
      )}
    </>
  );
};
