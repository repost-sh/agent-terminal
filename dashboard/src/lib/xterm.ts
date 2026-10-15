import "@xterm/xterm/css/xterm.css";

import { WebglAddon } from "@xterm/addon-webgl";
import { Terminal, type ITheme } from "@xterm/xterm";

import { mirrorRows } from "./screen-rows";
import { daemonWidths } from "./unicode";

export const LARGEST_FONT_PX = 13;

const TEXT_FAMILY = "JetBrains Mono NL";

const FALLBACK_FAMILIES = ["Symbols Nerd Font Mono", "Noto Emoji"];

const FONT = [TEXT_FAMILY, ...FALLBACK_FAMILIES].map((family) => `"${family}"`).concat("monospace").join(", ");

const PALETTE: ITheme = {
  background: "#111113",
  foreground: "#d4d4d8",
  cursor: "#d4d4d8",
  cursorAccent: "#111113",
  selectionBackground: "#1e3a8a",
  black: "#27272a",
  red: "#f87171",
  green: "#4ade80",
  yellow: "#facc15",
  blue: "#60a5fa",
  magenta: "#c084fc",
  cyan: "#22d3ee",
  white: "#d4d4d8",
  brightBlack: "#71717a",
  brightRed: "#fca5a5",
  brightGreen: "#86efac",
  brightYellow: "#fde047",
  brightBlue: "#93c5fd",
  brightMagenta: "#d8b4fe",
  brightCyan: "#67e8f9",
  brightWhite: "#fafafa",
};

const FACES = [
  ...["", "bold ", "italic ", "bold italic "].map((style) => `${style}${LARGEST_FONT_PX}px "${TEXT_FAMILY}"`),
  ...FALLBACK_FAMILIES.map((family) => `${LARGEST_FONT_PX}px "${family}"`),
];

let fontsLoaded: Promise<unknown> | null = null;

const fontsReady = () =>
  (fontsLoaded ??= Promise.all(FACES.map((face) => document.fonts.load(face))).catch((error: unknown) =>
    console.warn("agent-terminal: the terminal font did not load; using a fallback", error),
  ));

const drawWithWebgl = (terminal: Terminal) => {
  try {
    const webgl = new WebglAddon();
    webgl.onContextLoss(() => webgl.dispose());
    terminal.loadAddon(webgl);
  } catch (error) {
    console.warn("agent-terminal: WebGL is not available; drawing the terminal with the DOM renderer", error);
  }
};

export const openTerminal = (host: HTMLElement, use: (terminal: Terminal) => () => void) => {
  let terminal: Terminal | null = null;
  let unmirror = () => {};
  let release = () => {};
  let disposed = false;
  fontsReady().then(() => {
    if (disposed) return;
    terminal = new Terminal({
      fontFamily: FONT,
      fontSize: LARGEST_FONT_PX,
      scrollback: 0,
      disableStdin: true,
      cursorBlink: !matchMedia("(prefers-reduced-motion: reduce)").matches,
      cursorInactiveStyle: "block",
      theme: PALETTE,
      allowProposedApi: true,
    });
    terminal.unicode.register(daemonWidths);
    terminal.unicode.activeVersion = daemonWidths.version;
    terminal.open(host);
    unmirror = mirrorRows(terminal, host);
    drawWithWebgl(terminal);
    release = use(terminal);
  });
  return () => {
    disposed = true;
    release();
    unmirror();
    terminal?.dispose();
  };
};
