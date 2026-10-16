import type { CSSProperties } from "react";

export interface Style {
  fg?: string | undefined;
  bg?: string | undefined;
  bold?: boolean;
  dim?: boolean;
  italic?: boolean;
  underline?: boolean;
  inverse?: boolean;
  hidden?: boolean;
  strike?: boolean;
}

export interface Run { text: string; style: Style }

interface Attribute { code: number; sub: number[] }

const CSI = "\x1b[";

const SGR = /^([\d;:]*)m/;

const cube = (level: number) => (level === 0 ? 0 : 55 + 40 * level);

const indexed = (n: number | undefined) => {
  if (n === undefined || n > 255) {
    return undefined;
  }

  if (n < 16) {
    return `var(--ansi-${String(n)})`;
  }

  if (n < 232) {
    const at = n - 16;

    return `rgb(${String(cube(Math.floor(at / 36)))} ${String(cube(Math.floor(at / 6) % 6))} ${String(cube(at % 6))})`;
  }

  const gray = String(8 + 10 * (n - 232));

  return `rgb(${gray} ${gray} ${gray})`;
};

const rgb = (values: number[]) => {
  const [r, g, b] = values.slice(-3);

  return b === undefined ? undefined : `rgb(${String(r)} ${String(g)} ${String(b)})`;
};

const extendedColor = (spec: number[]) => {
  const [kind, ...values] = spec;

  if (kind === 5) {
    return indexed(values[0]);
  }

  return kind === 2 ? rgb(values) : undefined;
};

const COLORS = [
  { from: 30, to: 37, key: "fg", base: 30 },
  { from: 40, to: 47, key: "bg", base: 40 },
  { from: 90, to: 97, key: "fg", base: 82 },
  { from: 100, to: 107, key: "bg", base: 92 },
] as const;

const colored = (style: Style, code: number): Style | undefined => {
  const range = COLORS.find((candidate) => code >= candidate.from && code <= candidate.to);

  if (!range) {
    return undefined;
  }

  const color = indexed(code - range.base);

  return range.key === "fg" ? { ...style, fg: color } : { ...style, bg: color };
};

const switches = (sub: number[]): Record<number, Style> => ({
  1: { bold: true },
  2: { dim: true },
  3: { italic: true },
  4: { underline: sub[0] !== 0 },
  7: { inverse: true },
  8: { hidden: true },
  9: { strike: true },
  21: { underline: true },
  22: { bold: false, dim: false },
  23: { italic: false },
  24: { underline: false },
  27: { inverse: false },
  28: { hidden: false },
  29: { strike: false },
  39: { fg: undefined },
  49: { bg: undefined },
});

const plain = (style: Style, attribute: Attribute): Style => {
  if (attribute.code === 0) {
    return {};
  }

  return colored(style, attribute.code) ?? { ...style, ...switches(attribute.sub)[attribute.code] };
};

const EXTENDED_LENGTH: Record<number, number> = { 5: 2, 2: 4 };

const groupsOf = (params: string) =>
  params.split(";").map((group) => group.split(":").map(Number));

const followingSpec = (groups: number[][], at: number) => {
  const length = EXTENDED_LENGTH[groups[at + 1]?.[0] ?? 0] ?? 0;

  return { spec: groups.slice(at + 1, at + 1 + length).map((group) => group[0] ?? 0), skip: length };
};

const extended = (style: Style, attribute: Attribute): Style =>
  attribute.code === 38 ? { ...style, fg: extendedColor(attribute.sub) } : { ...style, bg: extendedColor(attribute.sub) };

interface Applying { groups: number[][]; at: number }

const applyAt = (style: Style, applying: Applying) => {
  const [code = 0, ...sub] = applying.groups[applying.at] ?? [];

  if (code !== 38 && code !== 48) {
    return { style: plain(style, { code, sub }), skip: 0 };
  }

  const taken = sub.length ? { spec: sub, skip: 0 } : followingSpec(applying.groups, applying.at);

  return { style: extended(style, { code, sub: taken.spec }), skip: taken.skip };
};

const applied = (style: Style, params: string): Style => {
  const groups = groupsOf(params);
  let skipping = 0;

  return Array.from(groups.keys()).reduce((next, at) => {
    if (skipping > 0) {
      skipping -= 1;

      return next;
    }

    const step = applyAt(next, { groups, at });

    skipping = step.skip;

    return step.style;
  }, style);
};

export interface Styling { lines: Run[][]; style: Style }

export const UNSTYLED: Styling = { lines: [], style: {} };

export const moreStyled = (from: Styling, text: string): Styling => {
  const lines: Run[][] = [...from.lines.slice(0, -1), [...(from.lines.at(-1) ?? [])]];
  let style = from.style;
  const add = (chunk: string) => {
    chunk.split("\n").forEach((part, n) => {
      if (n > 0) {
        lines.push([]);
      }

      if (part) {
        lines.at(-1)?.push({ text: part, style });
      }
    });
  };

  const afterEachCsi = text.split(CSI);
  let pending = String(afterEachCsi.shift());

  for (const after of afterEachCsi) {
    const sgr = SGR.exec(after);

    if (sgr) {
      add(pending);
      style = applied(style, String(sgr[1]));
      pending = after.slice(sgr[0].length);
    } else {
      pending += CSI + after;
    }
  }

  add(pending);

  return { lines, style };
};

export const styledLines = (text: string): Run[][] => {
  const lines = moreStyled(UNSTYLED, text).lines;

  if (lines.length > 1 && !lines.at(-1)?.length) {
    lines.pop();
  }

  return lines;
};

const colors = (style: Style): CSSProperties => ({
  color: style.inverse ? (style.bg ?? "var(--background)") : style.fg,
  backgroundColor: style.inverse ? (style.fg ?? "var(--foreground)") : style.bg,
});

const emphasis = (style: Style): CSSProperties => ({
  fontWeight: style.bold ? 700 : undefined,
  opacity: style.dim ? 0.6 : undefined,
  fontStyle: style.italic ? "italic" : undefined,
});

const decoration = (style: Style): CSSProperties => ({
  textDecorationLine: [style.underline && "underline", style.strike && "line-through"].filter(Boolean).join(" ") || undefined,
  visibility: style.hidden ? "hidden" : undefined,
});

export const runStyle = (style: Style): CSSProperties => ({ ...colors(style), ...emphasis(style), ...decoration(style) });
