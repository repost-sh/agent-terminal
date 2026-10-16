import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import { TooltipLayer } from "./tooltip";

const page = () => {
  render(
    <>
      <button type="button" data-tooltip="Restart">
        <span>icon</span>
      </button>
      <button type="button" data-tooltip="Screenshot">
        shot
      </button>
      <p>plain</p>
      <TooltipLayer />
    </>,
  );

  return screen.getByRole("tooltip", { hidden: true });
};

test("hovering a control shows its tooltip under it, kept on screen", () => {
  const tip = page();

  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue(new DOMRect(100, 50, 40, 20));
  fireEvent.mouseOver(screen.getByText("icon"));
  expect([tip.hidden, tip.textContent, tip.style.left, tip.style.top]).toEqual([false, "Restart", "120px", "74px"]);
  fireEvent.mouseOver(screen.getByText("icon"));
  fireEvent.mouseOver(screen.getByText("shot"));
  expect(tip.textContent).toBe("Screenshot");
  fireEvent.mouseOver(screen.getByText("plain"));
  expect(tip.hidden).toBe(true);
  fireEvent.mouseOver(screen.getByText("plain"));
  expect(tip.hidden).toBe(true);
});

test("a focused control keeps its tooltip until Escape or a click, and the Escape that hides it goes no further", () => {
  const tip = page();
  const restart = screen.getByRole("button", { name: "icon" });
  const heard = vi.fn();

  restart.addEventListener("keydown", heard);
  fireEvent.mouseOver(restart);
  restart.focus();
  fireEvent.mouseOver(screen.getByText("plain"));
  expect(tip.hidden).toBe(false);
  expect([fireEvent.keyDown(restart, { key: "x" }), tip.hidden, heard.mock.calls.length]).toEqual([true, false, 1]);
  fireEvent.keyDown(restart, { key: "Escape" });
  expect([tip.hidden, heard.mock.calls.length]).toEqual([true, 1]);
  fireEvent.keyDown(restart, { key: "Escape" });
  expect(heard.mock.calls.length).toBe(2);
  fireEvent.mouseOver(restart);
  fireEvent.click(document.body);
  expect(tip.hidden).toBe(true);
});

test("focus shows the tooltip of a control it visibly reaches, and hides it when it moves elsewhere", () => {
  const tip = page();
  const shot = screen.getByRole("button", { name: "shot" });

  fireEvent.focusIn(shot);
  expect(tip.hidden).toBe(true);
  shot.focus();
  expect([tip.hidden, tip.textContent]).toEqual([false, "Screenshot"]);
  fireEvent.focusIn(screen.getByText("plain"));
  expect(tip.hidden).toBe(true);
});

test("a tooltip near the edge of the window stays inside it", () => {
  const tip = page();

  vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockReturnValue(30);
  vi.spyOn(HTMLElement.prototype, "offsetHeight", "get").mockReturnValue(16);
  const placed = (box: DOMRect) => {
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue(box);
    fireEvent.mouseOver(screen.getByText("shot"));

    return [tip.style.left, tip.style.top];
  };

  expect([placed(new DOMRect(100, 50, 40, 20)), placed(new DOMRect(window.innerWidth - 10, window.innerHeight - 10, 10, 10)), placed(new DOMRect(0, 0, 4, 4))]).toEqual([
    ["105px", "74px"],
    [`${String(window.innerWidth - 34)}px`, `${String(window.innerHeight - 20)}px`],
    ["4px", "8px"],
  ]);
});

test("a layer that is gone stops following the page", () => {
  const tip = page();
  const heard = vi.fn();

  fireEvent.mouseOver(screen.getByText("icon"));
  cleanup();
  document.body.addEventListener("keydown", heard);
  fireEvent.keyDown(document.body, { key: "Escape" });
  fireEvent.click(document.body);
  fireEvent.mouseOver(document.body);
  fireEvent.focusIn(document.body);
  document.body.removeEventListener("keydown", heard);
  expect([heard.mock.calls.length, tip.hidden]).toEqual([1, false]);
});
