export type Color = { index: number } | { rgb: string };

export type Style = {
  fg: Color | null;
  bg: Color | null;
  bold: boolean;
  dim: boolean;
  italic: boolean;
  underline: boolean;
  inverse: boolean;
  strike: boolean;
};

export type Run = { text: string; style: Style };

export type Line = { text: string; runs: Run[] };

export type Item = { kind: "line"; line: Line } | { kind: "mark"; name: string };

export const PLAIN: Style = { fg: null, bg: null, bold: false, dim: false, italic: false, underline: false, inverse: false, strike: false };

const CUBE = [0, 95, 135, 175, 215, 255];

const hex = (n: number) => n.toString(16).padStart(2, "0");

const indexed = (n: number): Color | null => {
  if (!Number.isInteger(n) || n < 0 || n > 255) return null;
  if (n < 16) return { index: n };
  if (n >= 232) {
    const level = 8 + 10 * (n - 232);
    return { rgb: `#${hex(level)}${hex(level)}${hex(level)}` };
  }
  const cube = n - 16;
  return { rgb: `#${hex(CUBE[Math.floor(cube / 36)] ?? 0)}${hex(CUBE[Math.floor(cube / 6) % 6] ?? 0)}${hex(CUBE[cube % 6] ?? 0)}` };
};

const extended = (params: number[][], at: number): [Color | null, number] => {
  const param = params[at] ?? [];
  if (param.length > 1) {
    const [, kind, ...rest] = param;
    if (kind === 5) return [indexed(rest[0] ?? -1), at + 1];
    if (kind === 2) {
      const rgb = rest.length > 3 ? rest.slice(1) : rest;
      return [rgbOf(rgb), at + 1];
    }
    return [null, at + 1];
  }
  const kind = params[at + 1]?.[0];
  if (kind === 5) return [indexed(params[at + 2]?.[0] ?? -1), at + 3];
  if (kind === 2) return [rgbOf([params[at + 2]?.[0] ?? -1, params[at + 3]?.[0] ?? -1, params[at + 4]?.[0] ?? -1]), at + 5];
  return [null, at + 1];
};

const rgbOf = ([r, g, b]: number[]): Color | null =>
  [r, g, b].every((part) => part !== undefined && part >= 0 && part <= 255) ? { rgb: `#${hex(r ?? 0)}${hex(g ?? 0)}${hex(b ?? 0)}` } : null;

const sgr = (style: Style, text: string): Style => {
  const params = (text === "" ? "0" : text).split(";").map((param) => param.split(":").map((part) => (part === "" ? 0 : Number(part))));
  let next = { ...style };
  for (let at = 0; at < params.length; ) {
    const code = params[at]?.[0] ?? 0;
    if (code === 38 || code === 48) {
      const [color, after] = extended(params, at);
      if (code === 38) next.fg = color;
      else next.bg = color;
      at = after;
      continue;
    }
    if (code === 0) next = { ...PLAIN };
    else if (code === 1) next.bold = true;
    else if (code === 2) next.dim = true;
    else if (code === 3) next.italic = true;
    else if (code === 4) next.underline = true;
    else if (code === 7) next.inverse = true;
    else if (code === 9) next.strike = true;
    else if (code === 21 || code === 22) next = { ...next, bold: false, dim: false };
    else if (code === 23) next.italic = false;
    else if (code === 24) next.underline = false;
    else if (code === 27) next.inverse = false;
    else if (code === 29) next.strike = false;
    else if (code >= 30 && code <= 37) next.fg = { index: code - 30 };
    else if (code === 39) next.fg = null;
    else if (code >= 40 && code <= 47) next.bg = { index: code - 40 };
    else if (code === 49) next.bg = null;
    else if (code >= 90 && code <= 97) next.fg = { index: code - 90 + 8 };
    else if (code >= 100 && code <= 107) next.bg = { index: code - 100 + 8 };
    at += 1;
  }
  return next;
};

const picture = (char: string) => {
  const code = char.charCodeAt(0);
  if (code < 0x20) return String.fromCharCode(0x2400 + code);
  if (code === 0x7f) return "␡";
  return char;
};

type Parse = "ground" | "escape" | "csi" | "string";

type Lines = { items: Item[]; open: Line };

const emptyLine = (): Line => ({ text: "", runs: [] });

const closed = (lines: Lines) => {
  lines.items.push({ kind: "line", line: lines.open });
  lines.open = emptyLine();
};

const marked = (lines: Lines, name: string) => {
  if (lines.open.text !== "") closed(lines);
  lines.items.push({ kind: "mark", name });
};

export type OutputStream = { plain: Lines; raw: Lines; parse: Parse; params: string; style: Style };

export const newStream = (): OutputStream => ({
  plain: { items: [], open: emptyLine() },
  raw: { items: [], open: emptyLine() },
  parse: "ground",
  params: "",
  style: PLAIN,
});

const printed = (line: Line, char: string, style: Style) => {
  line.text += char;
  const last = line.runs[line.runs.length - 1];
  if (last && last.style === style) last.text += char;
  else line.runs.push({ text: char, style });
};

const withOpenCopied = (lines: Lines): Lines => ({
  items: [...lines.items],
  open: { text: lines.open.text, runs: lines.open.runs.map((run) => ({ ...run })) },
});

const feedChar = (stream: OutputStream, char: string) => {
  const raw = stream.raw.open;
  if (char === "\n") {
    printed(raw, "␊", PLAIN);
    closed(stream.raw);
    closed(stream.plain);
    return;
  }
  printed(raw, picture(char), PLAIN);
  if (char === "\x1b") {
    stream.parse = "escape";
    return;
  }
  switch (stream.parse) {
    case "ground":
      if (char === "\t" || char >= " ") {
        if (char !== "\x7f") printed(stream.plain.open, char, stream.style);
      }
      return;
    case "escape":
      if (char === "[") {
        stream.parse = "csi";
        stream.params = "";
      } else if (char === "]" || char === "P" || char === "X" || char === "^" || char === "_") stream.parse = "string";
      else if (char >= "0" && char <= "~") stream.parse = "ground";
      return;
    case "csi":
      if (char >= "@" && char <= "~") {
        if (char === "m" && /^[\d;:]*$/.test(stream.params)) stream.style = sgr(stream.style, stream.params);
        stream.parse = "ground";
      } else if (char >= " " && char <= "?") stream.params += char;
      return;
    case "string":
      if (char === "\x07") stream.parse = "ground";
      return;
  }
};

export type Segment = { text: string } | { mark: string };

export const fed = (stream: OutputStream, segments: Segment[]): OutputStream => {
  const next: OutputStream = { ...stream, plain: withOpenCopied(stream.plain), raw: withOpenCopied(stream.raw) };
  for (const segment of segments) {
    if ("mark" in segment) {
      marked(next.plain, segment.mark);
      marked(next.raw, segment.mark);
      continue;
    }
    for (const char of segment.text) feedChar(next, char);
  }
  return next;
};

export const itemsOf = (lines: Lines): Item[] => (lines.open.text === "" ? lines.items : [...lines.items, { kind: "line", line: lines.open }]);
