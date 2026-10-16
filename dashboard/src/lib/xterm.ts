import "@xterm/xterm/css/xterm.css";

import { WebglAddon } from "@xterm/addon-webgl";
import { Terminal, type ITheme } from "@xterm/xterm";

import { mirrorRows } from "./screen-rows";
import { settle } from "./settle";
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

const LARGEST_FONT = `${String(LARGEST_FONT_PX)}px`;

const FACES = [
  ...["", "bold ", "italic ", "bold italic "].map((style) => `${style}${LARGEST_FONT} "${TEXT_FAMILY}"`),
  ...FALLBACK_FAMILIES.map((family) => `${LARGEST_FONT} "${family}"`),
];

let fontsLoaded: Promise<unknown> | null = null;

const fontsReady = () =>
  (fontsLoaded ??= settle(() => Promise.all(FACES.map((face) => document.fonts.load(face)))).then((loaded) => {
    if (!loaded.ok) {
      console.warn("agent-terminal: the terminal font did not load; using a fallback", loaded.error);
    }
  }));

let gpu: boolean | null = null;

const probedGpu = () => {
  const probe = document.createElement("canvas").getContext("webgl2", { failIfMajorPerformanceCaveat: true });

  probe?.getExtension("WEBGL_lose_context")?.loseContext();

  return probe !== null;
};

const onGpu = () => (gpu ??= probedGpu());

const drawWithWebgl = (terminal: Terminal) => {
  const drawing = settle(() => {
    const webgl = new WebglAddon();

    webgl.onContextLoss(() => {
      webgl.dispose();
    });
    terminal.loadAddon(webgl);
  });

  void drawing.then((drawn) => {
    if (!drawn.ok) {
      console.warn("agent-terminal: WebGL is not available; drawing the terminal with the DOM renderer", drawn.error);
    }
  });
};

const newTerminal = () =>
  new Terminal({
    fontFamily: FONT,
    fontSize: LARGEST_FONT_PX,
    scrollback: 0,
    disableStdin: true,
    cursorBlink: !matchMedia("(prefers-reduced-motion: reduce)").matches,
    cursorInactiveStyle: "block",
    theme: PALETTE,
    allowProposedApi: true,
  });

export const openTerminal = (host: HTMLElement, use: (terminal: Terminal) => () => void) => {
  let terminal: Terminal | null = null;
  let unmirror: (() => void) | null = null;
  let release: (() => void) | null = null;
  let disposed = false;

  void fontsReady().then(() => {
    if (disposed) {
      return;
    }

    terminal = newTerminal();
    terminal.unicode.register(daemonWidths);
    terminal.unicode.activeVersion = daemonWidths.version;
    terminal.open(host);
    unmirror = mirrorRows(terminal, host);

    if (onGpu()) {
      drawWithWebgl(terminal);
    }

    release = use(terminal);
  });

  return () => {
    disposed = true;
    release?.();
    unmirror?.();
    terminal?.dispose();
  };
};
