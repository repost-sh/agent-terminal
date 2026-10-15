import "@xterm/xterm/css/xterm.css";

import { WebglAddon } from "@xterm/addon-webgl";
import { Terminal, type ITheme } from "@xterm/xterm";

export const LARGEST_FONT_PX = 13;

const FONT = '"JetBrains Mono", ui-monospace, monospace';

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

const fontsReady = () =>
  Promise.all([document.fonts.load(`${LARGEST_FONT_PX}px ${FONT}`), document.fonts.load(`bold ${LARGEST_FONT_PX}px ${FONT}`)]).catch((error: unknown) =>
    console.warn("agent-terminal: the terminal font did not load; using a fallback", error),
  );

const drawWithWebgl = (terminal: Terminal) => {
  try {
    const webgl = new WebglAddon();
    webgl.onContextLoss(() => webgl.dispose());
    terminal.loadAddon(webgl);
  } catch (error) {
    console.warn("agent-terminal: WebGL is not available; drawing the terminal with the DOM renderer", error);
  }
};

const dropAnnouncementsOnceDrawn = (terminal: Terminal) => {
  const liveRegion = terminal.element?.querySelector(".live-region");
  if (!liveRegion) return;
  terminal.onRender(() => {
    if (liveRegion.textContent) liveRegion.textContent = "";
  });
};

export const openTerminal = (host: HTMLElement, use: (terminal: Terminal) => () => void) => {
  let terminal: Terminal | null = null;
  let release = () => {};
  let disposed = false;
  fontsReady().then(() => {
    if (disposed) return;
    terminal = new Terminal({
      fontFamily: FONT,
      fontSize: LARGEST_FONT_PX,
      scrollback: 0,
      disableStdin: true,
      screenReaderMode: true,
      cursorBlink: !matchMedia("(prefers-reduced-motion: reduce)").matches,
      cursorInactiveStyle: "block",
      theme: PALETTE,
    });
    terminal.open(host);
    dropAnnouncementsOnceDrawn(terminal);
    drawWithWebgl(terminal);
    release = use(terminal);
  });
  return () => {
    disposed = true;
    release();
    terminal?.dispose();
  };
};
