import { act, render } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import { App } from "../app";
import { startDaemon } from "../fake-daemon/daemon";
import { frameOf, sessionInfo, tabInfo } from "../fake-daemon/messages";

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

const resize = (box: DOMRect) => {
  const area = document.getElementById("viewport-region");
  const watching = observers.filter((observer) => area && observer.targets.includes(area));

  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue(box);
  expect(watching).toHaveLength(1);
  act(() => {
    for (const observer of watching) {
      observer.callback([], observer as unknown as ResizeObserver);
    }
  });
};

const fontSize = () => {
  const styles = Array.from(document.querySelectorAll("style"), (style) => style.textContent).join("\n");

  return /\.xterm-rows \{[^}]*font-size: ([\d.]+px)/.exec(styles)?.[1];
};

test("the terminal's font shrinks to fit its area in quarter-pixel steps, and never grows past 13px", async () => {
  vi.stubGlobal("ResizeObserver", CapturingObserver);
  const daemon = startDaemon([sessionInfo("work", [tabInfo("t1")])]);

  render(<App />);
  await daemon.serve();
  await daemon.connected(STREAM);
  await daemon.push(STREAM, frameOf(1, { paint: "x" }));
  const area = document.getElementById("viewport-region");

  Object.defineProperties(area, { clientWidth: { value: 432 }, clientHeight: { value: 332 } });
  expect(fontSize()).toBe("13px");
  resize(new DOMRect(0, 0, 0, 0));
  expect(fontSize()).toBe("13px");
  resize(new DOMRect(0, 0, 800, 600));
  expect(fontSize()).toBe("6.5px");
  resize(new DOMRect(0, 0, 400, 300));
  expect(fontSize()).toBe("6.5px");
  resize(new DOMRect(0, 0, 100, 75));
  expect(fontSize()).toBe("13px");
});
