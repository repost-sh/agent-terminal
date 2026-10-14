import type { CSSProperties } from "react";

export type Style = {
  fg?: string;
  bg?: string;
  bold?: boolean;
  dim?: boolean;
  italic?: boolean;
  underline?: boolean;
  inverse?: boolean;
  hidden?: boolean;
  strike?: boolean;
};

export type Run = { text: string; style: Style };

const CSI = "\x1b[";

function* sgrSequences(text: string) {
  const params = /([\d;:]*)m/y;
  for (let index = text.indexOf(CSI); index !== -1; index = text.indexOf(CSI, index + 1)) {
    params.lastIndex = index + CSI.length;
    const match = params.exec(text);
    if (match) yield { index, length: CSI.length + match[0].length, params: match[1] ?? "" };
  }
}

const CUBE = [0, 95, 135, 175, 215, 255];

const indexed = (n: number | undefined) => {
  if (n === undefined || n < 0 || n > 255) return undefined;
  if (n < 16) return `var(--ansi-${n})`;
  if (n < 232) {
    const at = n - 16;
    return `rgb(${CUBE[Math.floor(at / 36)]} ${CUBE[Math.floor(at / 6) % 6]} ${CUBE[at % 6]})`;
  }
  const gray = 8 + 10 * (n - 232);
  return `rgb(${gray} ${gray} ${gray})`;
};

const extendedColor = (spec: number[]) => {
  const [kind, ...values] = spec;
  if (kind === 5) return indexed(values[0]);
  const [r, g, b] = values.length > 3 ? values.slice(-3) : values;
  return kind === 2 && b !== undefined ? `rgb(${r} ${g} ${b})` : undefined;
};

const plain = (style: Style, code: number, sub: number[]): Style => {
  if (code === 0) return {};
  if (code >= 30 && code <= 37) return { ...style, fg: indexed(code - 30) };
  if (code >= 40 && code <= 47) return { ...style, bg: indexed(code - 40) };
  if (code >= 90 && code <= 97) return { ...style, fg: indexed(code - 82) };
  if (code >= 100 && code <= 107) return { ...style, bg: indexed(code - 92) };
  const changes: Record<number, Style> = {
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
  };
  return { ...style, ...changes[code] };
};

const applied = (style: Style, params: string): Style => {
  const groups = params.split(";").map((group) => group.split(":").map((value) => (value === "" ? 0 : Number(value))));
  let next = style;
  for (let i = 0; i < groups.length; i += 1) {
    const [code = 0, ...sub] = groups[i] ?? [];
    if (code !== 38 && code !== 48) {
      next = plain(next, code, sub);
      continue;
    }
    let spec = sub;
    if (!sub.length) {
      const length = groups[i + 1]?.[0] === 5 ? 2 : groups[i + 1]?.[0] === 2 ? 4 : 0;
      spec = groups.slice(i + 1, i + 1 + length).map((group) => group[0] ?? 0);
      i += length;
    }
    next = code === 38 ? { ...next, fg: extendedColor(spec) } : { ...next, bg: extendedColor(spec) };
  }
  return next;
};

export const styledLines = (text: string): Run[][] => {
  const lines: Run[][] = [[]];
  let style: Style = {};
  let at = 0;
  const add = (chunk: string) =>
    chunk.split("\n").forEach((part, n) => {
      if (n > 0) lines.push([]);
      if (part) lines.at(-1)?.push({ text: part, style });
    });
  for (const { index, length, params } of sgrSequences(text)) {
    add(text.slice(at, index));
    style = applied(style, params);
    at = index + length;
  }
  add(text.slice(at));
  if (lines.length > 1 && !lines.at(-1)?.length) lines.pop();
  return lines;
};

export const runStyle = ({ fg, bg, bold, dim, italic, underline, inverse, hidden, strike }: Style): CSSProperties => ({
  color: inverse ? (bg ?? "var(--background)") : fg,
  backgroundColor: inverse ? (fg ?? "var(--foreground)") : bg,
  fontWeight: bold ? 700 : undefined,
  opacity: dim ? 0.6 : undefined,
  fontStyle: italic ? "italic" : undefined,
  textDecorationLine: [underline && "underline", strike && "line-through"].filter(Boolean).join(" ") || undefined,
  visibility: hidden ? "hidden" : undefined,
});
