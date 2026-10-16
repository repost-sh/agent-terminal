import { act, render, screen, within } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import { App } from "../app";
import { startDaemon } from "../fake-daemon/daemon";
import { frameOf, sessionInfo, tabInfo } from "../fake-daemon/messages";
import { openAt } from "../fake-daemon/sockets";
import { navigate } from "../lib/route";

const STREAM = "/api/session/work/t1/stream";

const observers: CapturingObserver[] = [];

class CapturingObserver {
  readonly targets: Element[] = [];
  readonly callback: ResizeObserverCallback;

  constructor(callback: ResizeObserverCallback) {
    this.callback = callback;
    observers.push(this);
  }

  observe(target: Element) {
    this.targets.push(target);
  }

  disconnect() {
    this.targets.length = 0;
  }
}

const viewport = () => screen.getByTestId("viewport");

const resize = (box: DOMRect, watched = (observer: CapturingObserver) => observer.targets.includes(viewport())) => {
  const watching = observers.filter(watched);

  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue(box);
  expect(watching).toHaveLength(1);
  act(() => {
    for (const observer of watching) {
      observer.callback([], observer as unknown as ResizeObserver);
    }
  });
};

const fontSize = () => /\.xterm-rows \{[^}]*font-size: ([\d.]+px)/.exec(screen.getByRole("group", { name: /live frame/ }).textContent)?.[1];

test("the terminal's font shrinks to fit its area in quarter-pixel steps, and never grows past 13px", async () => {
  vi.stubGlobal("ResizeObserver", CapturingObserver);
  const daemon = startDaemon([sessionInfo("work", [tabInfo("t1")])]);

  render(<App />);
  await daemon.serve();
  await daemon.connected(STREAM);
  await daemon.push(STREAM, frameOf(1, { paint: "x" }));
  Object.defineProperties(viewport(), { clientWidth: { value: 432 }, clientHeight: { value: 332 } });
  expect(fontSize()).toBe("13px");
  const fitted = [new DOMRect(0, 0, 0, 0), new DOMRect(0, 0, 0, 600), new DOMRect(0, 0, 800, 600), new DOMRect(0, 0, 400, 300), new DOMRect(0, 0, 100, 75), new DOMRect(0, 0, 800, 300), new DOMRect(0, 0, 100, 75), new DOMRect(0, 0, 400, 600), new DOMRect(0, 0, 8000, 6000)].map((box) => {
    resize(box);

    return fontSize();
  });

  expect(fitted).toEqual(["13px", "13px", "6.5px", "6.5px", "13px", "6.5px", "13px", "6.5px", "4px"]);
  const frameOfTab = screen.getByRole("group", { name: /live frame/ });

  resize(new DOMRect(0, 0, 100, 75), (observer) => observer.targets.some((target) => target !== frameOfTab && frameOfTab.contains(target)));
  expect(fontSize()).toBe("13px");
});

test("leaving a tab stops watching its size and closes its stream", async () => {
  vi.stubGlobal("ResizeObserver", CapturingObserver);
  const daemon = startDaemon([sessionInfo("work", [tabInfo("t1"), tabInfo("t2")])]);

  render(<App />);
  await daemon.serve();
  await daemon.connected(STREAM);
  const area = viewport();
  const watching = observers.filter((observer) => observer.targets.includes(area));

  await act(async () => {
    navigate("/s/work/t2");
    await Promise.resolve();
  });
  expect([watching.length, watching.flatMap((observer) => observer.targets), openAt(STREAM)]).toEqual([1, [], []]);
});

const frame = () => screen.getByRole("group", { name: /live frame/ });

test("a terminal hides until its first frame, dims once its program exits or its stream reconnects, and says how it exited", async () => {
  const daemon = startDaemon([sessionInfo("work", [tabInfo("t1", { program: "make" })])]);

  render(<App />);
  await daemon.serve();
  await daemon.connected(STREAM);
  expect(frame().className).toBe("term-frame opacity-0");
  await daemon.push(STREAM, frameOf(0, { exited: true, exit_code: 2 }));
  const exitBar = await within(viewport()).findByRole("status");

  expect([frame().className, screen.queryByText("Waiting for first frame...")]).toEqual(["term-frame is-dim", null]);
  expect(exitBar.outerHTML).toMatchSnapshot();
  await daemon.push(STREAM, frameOf(1, { exited: true, exit_code: 0 }));
  await screen.findByRole("group", { name: /live frame 1/ });
  expect(within(exitBar).getByText(/exited with code/).innerHTML).toBe('<span class="font-mono">make</span> exited with code <span class="font-mono">0</span>');
});

test("a frame that has not drawn yet waits, and a reconnecting stream dims the frame", async () => {
  const daemon = startDaemon([sessionInfo("work", [tabInfo("t1")])]);

  render(<App />);
  await daemon.serve();
  await daemon.connected(STREAM);
  await daemon.push(STREAM, frameOf(0));
  await vi.waitFor(() => {
    expect(daemon.sent(STREAM)).toEqual([{ type: "shown" }]);
  });
  expect([frame().className, screen.getByText("Waiting for first frame...").className]).toEqual(["term-frame opacity-0", "absolute flex items-center gap-2 text-xs text-muted-foreground"]);
  await daemon.push(STREAM, frameOf(1));
  expect(await screen.findByRole("group", { name: /live frame 1/ })).toBeTruthy();
  expect(frame().className).toBe("term-frame");
  await daemon.hangUp(STREAM);
  expect(frame().className).toBe("term-frame is-dim");
});

