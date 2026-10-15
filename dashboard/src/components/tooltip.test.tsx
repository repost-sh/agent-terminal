import { fireEvent, render, screen } from "@testing-library/react";
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

  const tip = document.querySelector<HTMLDivElement>("[aria-hidden='true'].fixed");

  if (!tip) {
    throw new Error("the tooltip layer is missing");
  }

  return tip;
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

test("a focused control keeps its tooltip until Escape or a click, and Escape alone passes", () => {
  const tip = page();
  const restart = screen.getByRole("button", { name: "icon" });

  fireEvent.mouseOver(restart);
  restart.focus();
  fireEvent.mouseOver(screen.getByText("plain"));
  expect(tip.hidden).toBe(false);
  expect(fireEvent.keyDown(document, { key: "x" })).toBe(true);
  fireEvent.keyDown(document, { key: "Escape" });
  expect(tip.hidden).toBe(true);
  fireEvent.keyDown(document, { key: "Escape" });
  fireEvent.mouseOver(restart);
  fireEvent.click(document.body);
  expect(tip.hidden).toBe(true);
  fireEvent.focusIn(screen.getByText("plain"));
  expect(tip.hidden).toBe(true);
});
