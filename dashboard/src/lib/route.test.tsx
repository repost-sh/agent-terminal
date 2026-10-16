import { act, fireEvent, render, renderHook, screen } from "@testing-library/react";
import fc from "fast-check";
import { expect, test } from "vitest";

import { followLink, leadHomeTo, navigate, parseRoute, splitRoute, tabRoute, usePath, type Cell } from "./route";

const name = fc.string({ minLength: 1, maxLength: 12, unit: "grapheme" });

const cell: fc.Arbitrary<Cell> = fc.option(fc.record({ session: name, tab: name }), { nil: null });

test("any split of two or four cells reads back as the same cells", () => {
  fc.assert(
    fc.property(fc.oneof(fc.array(cell, { minLength: 2, maxLength: 2 }), fc.array(cell, { minLength: 4, maxLength: 4 })), (cells) => {
      expect(parseRoute(splitRoute(cells))).toEqual({ kind: "split", cells });
    }),
  );
});

test("any tab reads back from its route", () => {
  fc.assert(
    fc.property(name, name, (session, tab) => {
      expect(parseRoute(tabRoute(session, tab))).toEqual({ kind: "tab", session, tab });
    }),
  );
});

test("routes the dashboard does not serve are missing", () => {
  expect(parseRoute("/")).toEqual({ kind: "home" });
  expect(
    ["/s/a", "/s/a/b/c", "/split/a:b", "/split/a:b,_,_", "/split/a:b,c", "/split/a:b:c,_", "/split/:b,_", "/x", "/x/split/a:b,_", "/split/a:b,_/x", "/x/s/a/t1", "/s/a/t1/x"].map(
      parseRoute,
    ),
  ).toEqual(
    Array.from({ length: 12 }, () => ({ kind: "missing" })),
  );
});

test("navigating pushes the path, replacing it when asked, and the root leads home", () => {
  const before = history.length;

  navigate("/s/a/t1");
  navigate("/s/a/t1");
  expect([location.pathname, history.length]).toEqual(["/s/a/t1", before + 1]);
  navigate("/s/a/t2", { replace: true });
  expect([location.pathname, history.length]).toEqual(["/s/a/t2", before + 1]);
  leadHomeTo("/s/b/t1");
  navigate("/");
  expect(location.pathname).toBe("/s/b/t1");
  leadHomeTo(null);
  navigate("/");
  expect(location.pathname).toBe("/");
});

const Link = () => (
  <a
    href="/s/x/t1"
    onClick={(event) => {
      followLink(event, "/s/x/t1");
    }}
  >
    x
  </a>
);

test("a plain left click follows a link in place and any other click is left to the browser", () => {
  navigate("/");
  render(<Link />);
  const link = screen.getByText("x");

  for (const init of [{ button: 1 }, { metaKey: true }, { ctrlKey: true }, { shiftKey: true }, { altKey: true }]) {
    expect(fireEvent.click(link, init)).toBe(true);
  }

  navigate("/");
  expect(fireEvent.click(link)).toBe(false);
  expect(location.pathname).toBe("/s/x/t1");
});

test("going back or forward in the browser moves the path", () => {
  const hook = renderHook(() => usePath());

  history.pushState(null, "", "/s/a/t2");
  expect(hook.result.current).toBe("/");
  act(() => {
    window.dispatchEvent(new PopStateEvent("popstate"));
  });
  expect(hook.result.current).toBe("/s/a/t2");
  hook.unmount();
});
