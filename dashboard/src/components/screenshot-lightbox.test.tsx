import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import { App } from "../app";
import { startDaemon, type FakeDaemon } from "../fake-daemon/daemon";
import { sessionInfo, shotOf, tabInfo } from "../fake-daemon/messages";
import type { Compared, Screenshot } from "../lib/protocol";

const SHOTS = "/api/session/work/screenshots/stream";

const COMPARE = "POST /api/session/work/screenshots/compare";

const three = (prefix: string): Screenshot[] => [
  shotOf(`${prefix}1.png`, { by: "snapshot --png" }),
  shotOf(`${prefix}2.png`, { frame: 4 }),
  shotOf(`${prefix}3.png`, { frame: 5 }),
];

const opened = async (list: Screenshot[], at: number) => {
  const daemon = startDaemon([sessionInfo("work", [tabInfo("t1")])]);

  render(<App />);
  await daemon.serve();
  fireEvent.click(screen.getByRole("tab", { name: "Screenshots" }));
  await daemon.connected(SHOTS);
  await daemon.push(SHOTS, { type: "screenshots", screenshots: list });
  fireEvent.click(screen.getAllByRole("button", { name: /\.png/ })[at] ?? document.body);

  return daemon;
};

const lightbox = () => screen.getByRole("dialog");

const button = (name: string) => within(lightbox()).getByRole<HTMLButtonElement>("button", { name });

const closed = () => {
  fireEvent.click(button("Close"));
};

const answering = (daemon: FakeDaemon, compared: Compared) => {
  daemon.answer(COMPARE, () => ({ status: 200, body: compared }));
};

const picked = async (other: Screenshot) => {
  await act(async () => {
    fireEvent.change(within(lightbox()).getByLabelText("Compare with"), { target: { value: other.path } });
    await Promise.resolve();
  });
  await within(lightbox()).findByText(/changed|sizes|refused/);
};

test("a screenshot shows framed at its size with its tab, frame, time and maker", async () => {
  const shots = three("look");

  await opened(shots, 0);
  expect(lightbox().outerHTML).toMatchSnapshot();
  fireEvent.click(button("Next screenshot"));
  expect(within(lightbox()).getByRole("figure").textContent).toBe("look2.png · t1 · frame 4 · 03:04:05 · dashboard");
  closed();
});

const stepped = (name: string) => {
  button(name).focus();
  fireEvent.click(button(name));

  return button("Previous screenshot").matches(":focus") ? "Previous" : "Next";
};

test("stepping to either end moves focus to the button that still steps", async () => {
  await opened(three("step"), 0);
  expect([stepped("Next screenshot"), stepped("Next screenshot"), stepped("Previous screenshot"), stepped("Previous screenshot")]).toEqual(["Next", "Previous", "Previous", "Next"]);
  closed();
});

test("the compare picker offers the other screenshots, and focus follows into and out of comparing", async () => {
  const shots = three("pick");
  const [, middle, last] = shots;
  const daemon = await opened(shots, 1);

  answering(daemon, { changed: 1, boxes: [{ x: 0, y: 0, width: 96, height: 64 }] });
  fireEvent.click(button("Compare with…"));
  expect(Array.from(within(lightbox()).getByLabelText<HTMLSelectElement>("Compare with").options, (option) => option.text)).toEqual(["Compare with…", "pick1.png", "pick3.png"]);
  await picked(last ?? shots[0] ?? middle ?? shotOf("none.png"));
  expect(button("Back to screenshot")).toHaveFocus();
  expect(lightbox().outerHTML).toMatchSnapshot();
  fireEvent.click(button("Back to screenshot"));
  expect(button("Compare with…")).toHaveFocus();
  closed();
});

test("a comparison always puts the older screenshot first, whichever one is open", async () => {
  const shots = three("order");
  const daemon = await opened(shots, 0);

  answering(daemon, { changed: 3, boxes: [] });
  fireEvent.click(button("Compare with…"));
  await picked(shots[1] ?? shotOf("none.png"));
  expect(within(lightbox()).getAllByRole("img").map((image) => image.getAttribute("alt"))).toEqual(["order1.png, frame 3", "order2.png, frame 4"]);
  closed();
});

test("a failed comparison reads as an error", async () => {
  const shots = three("fail");
  const daemon = await opened(shots, 0);

  daemon.answer(COMPARE, () => ({ status: 422, body: { error: { code: 1, message: "refused: not a screenshot" } } }));
  fireEvent.click(button("Compare with…"));
  await picked(shots[2] ?? shotOf("none.png"));
  expect(within(lightbox()).getByRole("status").outerHTML).toMatchSnapshot();
  closed();
});

test("screenshots of different sizes say so", async () => {
  const shots = three("dims");
  const daemon = await opened(shots, 0);

  answering(daemon, { changed: null, boxes: [] });
  fireEvent.click(button("Compare with…"));
  await picked(shots[2] ?? shotOf("none.png"));
  expect(within(lightbox()).getByRole("status").outerHTML).toMatchSnapshot();
  closed();
});

test("a comparison still on its way shows the pair with nothing counted yet", async () => {
  const shots = three("wait");

  await opened(shots, 0);
  vi.stubGlobal("fetch", () => new Promise(() => undefined));
  fireEvent.click(button("Compare with…"));
  await act(async () => {
    fireEvent.change(within(lightbox()).getByLabelText("Compare with"), { target: { value: shots[1]?.path } });
    await Promise.resolve();
  });
  expect([within(lightbox()).getByRole("status").textContent, within(lightbox()).getAllByRole("img").length]).toEqual(["", 2]);
  closed();
});

test("a screenshot that left the list shows no position and cannot step", async () => {
  const shots = three("gone");
  const daemon = await opened(shots, 0);

  await daemon.push(SHOTS, { type: "screenshots", screenshots: shots.slice(1) });
  expect([within(lightbox()).getByRole("img").getAttribute("alt"), button("Previous screenshot").disabled, button("Next screenshot").disabled]).toEqual(["gone1.png, frame 3", true, true]);
  expect(within(lightbox()).getByRole("group", { name: "Step through the screenshots" }).textContent).toBe("Previous screenshotNext screenshot");
  closed();
});
