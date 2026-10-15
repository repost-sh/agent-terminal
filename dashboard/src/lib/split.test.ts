import fc from "fast-check";
import { expect, test } from "vitest";

import { sessionInfo, tabInfo } from "../fake-daemon/messages";
import type { Cell } from "./route";
import { fillCells, focusedAt, isShown, resizeCells, shownCells, withCell, type Layout } from "./split";

const sessions = [sessionInfo("a", [tabInfo("t1"), tabInfo("t2")]), sessionInfo("b", [tabInfo("t1")])];

const a1 = { session: "a", tab: "t1" };
const a2 = { session: "a", tab: "t2" };
const b1 = { session: "b", tab: "t1" };

test("cells show the open tab they name and nothing for a closed one", () => {
  const shown = shownCells(sessions, [a2, null, { session: "a", tab: "t9" }]);

  expect(shown.map((entry) => entry && [entry.cell, entry.tab.tab])).toEqual([[a2, "t2"], null, null]);
});

test("focus stays on a shown cell or moves to the first shown one", () => {
  const shown = shownCells(sessions, [null, b1, a1, null]);

  expect([focusedAt(shown, 2), focusedAt(shown, 0), focusedAt(shown, 9)]).toEqual([2, 1, 1]);
  expect(focusedAt(shownCells(sessions, [null, null]), 1)).toBe(0);
});

test("filling a layout starts at the tab and wraps around the open tabs", () => {
  expect(fillCells(sessions, { from: a2, count: 4 })).toEqual([a2, b1, a1, null]);
  expect(fillCells(sessions, { from: b1, count: 2 })).toEqual([b1, a1]);
  expect(fillCells(sessions, { from: { session: "c", tab: "t1" }, count: 2 })).toEqual([{ session: "c", tab: "t1" }, a1]);
  expect(fillCells(sessions, { from: a1, count: 1 })).toEqual([a1]);
});

const cells = fc.array(fc.option(fc.constantFrom(a1, a2, b1), { nil: null }), { maxLength: 4 });

const layout = fc.constantFrom<Layout>(1, 2, 4);

test("resizing keeps the cells that fit and pads with empty ones", () => {
  fc.assert(
    fc.property(cells, layout, (shown, count) => {
      const resized = resizeCells(shown, count);

      expect(resized).toEqual(Array.from({ length: count }, (_, i) => shown[i] ?? null));
    }),
  );
});

test("placing a cell changes only that cell and the cell then shows", () => {
  fc.assert(
    fc.property(cells.filter((shown) => shown.length > 0), fc.nat(), (shown, index) => {
      const at = index % shown.length;
      const placed = withCell(shown, { at, cell: b1 });

      expect(placed.filter((_, i) => i !== at)).toEqual(shown.filter((_, i) => i !== at));
      expect(isShown(placed, b1)).toBe(true);
    }),
  );
  expect(isShown([null, a1] satisfies Cell[], a2)).toBe(false);
});
