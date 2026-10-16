import type { Terminal } from "@xterm/xterm";
import { afterEach, beforeEach, expect, test, vi } from "vitest";

import { daemonWidths } from "./unicode";

interface Webgl { lost: (() => void) | null; disposed: boolean; activated: unknown }

const webgl = vi.hoisted(() => ({ made: [] as Webgl[], failing: false }));

const HARDWARE = "ANGLE (Apple, ANGLE Metal Renderer: Apple M4 Max, Unspecified Version)";

const UNMASKED_RENDERER_WEBGL = 0x9246;

const RENDERER = 0x1f01;

const canvas = { gpu: true, named: true, unmasked: HARDWARE as unknown, masked: "WebKit WebGL" as unknown, asked: [] as unknown[][], released: 0 };

const REAL_GET_CONTEXT = Object.getOwnPropertyDescriptor(HTMLCanvasElement.prototype, "getContext");

const getContext = (kind: string, options?: unknown) => {
  if (kind !== "webgl2") {
    return null;
  }

  canvas.asked.push([kind, options]);
  const loseContext = () => {
    canvas.released += 1;
  };

  const extensions = new Map<string, unknown>([["WEBGL_lose_context", { loseContext }]]);
  const parameters = new Map([
    [UNMASKED_RENDERER_WEBGL, canvas.unmasked],
    [RENDERER, canvas.masked],
  ]);

  if (canvas.named) {
    extensions.set("WEBGL_debug_renderer_info", { UNMASKED_RENDERER_WEBGL });
  }

  return canvas.gpu
    ? { RENDERER, getExtension: (name: string) => extensions.get(name) ?? null, getParameter: (parameter: number) => parameters.get(parameter) ?? null }
    : null;
};

vi.mock("@xterm/addon-webgl", () => ({
  WebglAddon: class {
    lost: (() => void) | null = null;
    disposed = false;
    activated: unknown = null;

    constructor() {
      if (webgl.failing) {
        throw new Error("no WebGL here");
      }

      webgl.made.push(this);
    }

    onContextLoss(lost: () => void) {
      this.lost = lost;
    }

    activate(terminal: unknown) {
      this.activated = terminal;
    }

    dispose() {
      this.disposed = true;
    }
  },
}));

const FACES = [
  '13px "JetBrains Mono NL"',
  'bold 13px "JetBrains Mono NL"',
  'italic 13px "JetBrains Mono NL"',
  'bold italic 13px "JetBrains Mono NL"',
  '13px "Symbols Nerd Font Mono"',
  '13px "Noto Emoji"',
];

const fonts = { load: vi.fn((_face: string) => Promise.resolve([])) };

const REAL_MATCH_MEDIA = window.matchMedia.bind(window);

const motion = (reduced: boolean) => {
  vi.stubGlobal("matchMedia", (query: string) => (query === "(prefers-reduced-motion: reduce)" ? { matches: reduced } : REAL_MATCH_MEDIA(query)));
};

const opened = async () => {
  motion(false);
  const { openTerminal } = await import("./xterm");
  const host = document.createElement("div");
  const release = vi.fn();
  const used: Terminal[] = [];
  const close = openTerminal(host, (terminal) => {
    used.push(terminal);

    return release;
  });

  return { host, release, used, close };
};

const settings = (terminal: Terminal) => [
  terminal.unicode.activeVersion,
  terminal.options.cursorBlink,
  terminal.options.disableStdin,
  terminal.options.fontFamily,
  terminal.options.fontSize,
  terminal.options.scrollback,
  terminal.options.cursorInactiveStyle,
  terminal.options.theme,
  webgl.made.map((made) => made.activated === terminal),
];

const PALETTE = {
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

const contextLost = (made: Webgl) => {
  made.lost?.();

  return made.disposed;
};

beforeEach(() => {
  vi.resetModules();
  webgl.made.length = 0;
  webgl.failing = false;
  canvas.gpu = true;
  canvas.named = true;
  canvas.unmasked = HARDWARE;
  canvas.masked = "WebKit WebGL";
  canvas.asked.length = 0;
  canvas.released = 0;
  Object.defineProperty(HTMLCanvasElement.prototype, "getContext", { value: getContext, configurable: true });
  fonts.load.mockClear();
  Object.defineProperty(document, "fonts", { value: fonts, configurable: true });
});

afterEach(() => {
  Reflect.deleteProperty(document, "fonts");

  if (REAL_GET_CONTEXT) {
    Object.defineProperty(HTMLCanvasElement.prototype, "getContext", REAL_GET_CONTEXT);
  }
});

test("a terminal opens once its fonts are loaded, with the daemon's widths, a screen mirror and WebGL", async () => {
  const shown = await opened();

  await vi.waitFor(() => {
    expect(shown.used).toHaveLength(1);
  });
  expect(fonts.load.mock.calls.map(([face]) => face)).toEqual(FACES);
  expect(shown.used.map(settings)).toEqual([
    [daemonWidths.version, true, true, '"JetBrains Mono NL", "Symbols Nerd Font Mono", "Noto Emoji", monospace', 13, 0, "block", PALETTE, [true]],
  ]);
  expect(shown.host.querySelector('[role="list"][aria-label="Screen"]')).not.toBeNull();
  expect(webgl.made.map(contextLost)).toEqual([true]);
  expect([canvas.asked, canvas.released]).toEqual([[["webgl2", { failIfMajorPerformanceCaveat: true }]], 1]);
  shown.close();
  expect([shown.release.mock.calls.length, shown.host.querySelector('[role="list"]')]).toEqual([1, null]);
});

test("a browser whose WebGL would run without a GPU draws the terminal with the DOM renderer, and says nothing", async () => {
  const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);

  canvas.gpu = false;
  const shown = await opened();

  await vi.waitFor(() => {
    expect(shown.used).toHaveLength(1);
  });
  expect([webgl.made, canvas.released, warn.mock.calls]).toEqual([[], 0, []]);
  shown.close();
});

const drawnWithWebgl = async () => {
  const shown = await opened();

  await vi.waitFor(() => {
    expect(shown.used).toHaveLength(1);
  });
  shown.close();

  return [webgl.made.length === 1, canvas.released];
};

test.each([
  "ANGLE (Google, Vulkan 1.3.0 (SwiftShader Device (Subzero) (0x0000C0DE)), SwiftShader driver)",
  "llvmpipe (LLVM 15.0.7, 256 bits)",
  "softpipe",
  "Apple Software Renderer",
  "Microsoft Basic Render Driver",
])("WebGL that renders in software, as %s does, leaves the terminal to the DOM renderer", async (unmasked) => {
  canvas.unmasked = unmasked;
  canvas.masked = HARDWARE;

  expect(await drawnWithWebgl()).toEqual([false, 1]);
});

test("without the renderer's real name, the name WebGL shows decides, and a name that is not text means a GPU", async () => {
  canvas.named = false;
  canvas.unmasked = "SwiftShader";
  const masked = await drawnWithWebgl();

  vi.resetModules();
  webgl.made.length = 0;
  canvas.masked = "SwiftShader";
  const software = await drawnWithWebgl();

  vi.resetModules();
  webgl.made.length = 0;
  canvas.named = true;
  canvas.unmasked = null;

  expect([masked, software, await drawnWithWebgl()]).toEqual([
    [true, 1],
    [false, 2],
    [true, 3],
  ]);
});

test("the GPU is probed once for every terminal on the page", async () => {
  motion(false);
  const { openTerminal } = await import("./xterm");
  const used: Terminal[] = [];
  const closes = [0, 1].map(() =>
    openTerminal(document.createElement("div"), (terminal) => {
      used.push(terminal);

      return () => undefined;
    }),
  );

  await vi.waitFor(() => {
    expect(used).toHaveLength(2);
  });
  expect([canvas.asked.length, webgl.made.map((made) => used.includes(made.activated as Terminal))]).toEqual([1, [true, true]]);
  closes.forEach((close) => {
    close();
  });
});

test("the fonts load once for every terminal, and a blinking cursor waits for motion to be welcome", async () => {
  motion(false);
  const { openTerminal } = await import("./xterm");
  const first = openTerminal(document.createElement("div"), () => () => undefined);

  motion(true);
  const used: Terminal[] = [];
  const second = openTerminal(document.createElement("div"), (terminal) => {
    used.push(terminal);

    return () => undefined;
  });

  await vi.waitFor(() => {
    expect(used).toHaveLength(1);
  });
  expect([fonts.load.mock.calls.length, used.map((terminal) => terminal.options.cursorBlink)]).toEqual([FACES.length, [false]]);
  first();
  second();
});

test("a terminal closed before its fonts load never opens", async () => {
  const loaded: (() => void)[] = [];

  fonts.load.mockImplementation(
    () =>
      new Promise((resolve) => {
        loaded.push(() => {
          resolve([]);
        });
      }),
  );
  const shown = await opened();

  shown.close();
  await vi.waitFor(() => {
    expect(loaded).toHaveLength(FACES.length);
  });
  loaded.forEach((load) => {
    load();
  });
  await new Promise((resolve) => setTimeout(resolve, 10));
  expect([shown.used, shown.host.childElementCount]).toEqual([[], 0]);
});

test("a font that fails to load, or no WebGL, is warned about and the terminal still opens", async () => {
  const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
  const missing = new Error("no such font");

  fonts.load.mockImplementation(() => Promise.reject(missing));
  webgl.failing = true;
  const shown = await opened();

  await vi.waitFor(() => {
    expect(shown.used).toHaveLength(1);
  });
  await vi.waitFor(() => {
    expect(warn).toHaveBeenCalledTimes(2);
  });
  expect(warn.mock.calls.map(([message]) => String(message))).toEqual([
    "agent-terminal: the terminal font did not load; using a fallback",
    "agent-terminal: WebGL is not available; drawing the terminal with the DOM renderer",
  ]);
  expect(warn.mock.calls[0]?.[1]).toBe(missing);
  shown.close();
});
