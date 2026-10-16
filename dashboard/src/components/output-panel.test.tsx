import { fireEvent, render, screen, within } from "@testing-library/react";
import { expect, test } from "vitest";

import { App } from "../app";
import { startDaemon, type FakeDaemon } from "../fake-daemon/daemon";
import { sessionInfo, tabInfo } from "../fake-daemon/messages";

const OUTPUT = "/api/session/work/t1/output";

type Segment = { text: string } | { mark: string };

interface Update { segments: Segment[]; raw?: Segment[]; reset?: boolean }

const shownOutput = async () => {
  const daemon = startDaemon([sessionInfo("work", [tabInfo("t1")])]);

  render(<App />);
  await daemon.serve();
  fireEvent.click(screen.getByRole("tab", { name: "Output" }));
  await daemon.connected(OUTPUT);

  return daemon;
};

const pushed = async (daemon: FakeDaemon, update: Update) => {
  await daemon.push(OUTPUT, { type: "output", reset: update.reset ?? false, styled: update.segments, raw: update.raw ?? update.segments });
};

const output = () => within(screen.getByRole("list", { name: "Output of work t1" }));

const lines = () => output().getAllByRole("listitem").map((line) => within(line).queryByRole("separator")?.getAttribute("aria-label") ?? line.textContent);

const panel = () => within(screen.getByRole("tabpanel"));

const run = (text: string) => output().getByText((_, element) => element?.tagName === "SPAN" && element.textContent === text);

test("output reads as styled lines with marks between them, and raw shows the bytes", async () => {
  const daemon = await shownOutput();

  expect(screen.getByText("No output yet")).toBeTruthy();
  await pushed(daemon, { segments: [{ text: "one\nt" }, { mark: "build" }, { text: "\x1b[31mtwo\tend\x7f" }] });
  expect([lines(), output().getAllByRole("listitem")[0]?.className]).toEqual([["one", "t", "mark build", "two\tend\x7f"], "min-h-[1lh] whitespace-pre-wrap break-all"]);
  expect(["one", "t", "two\tend\x7f"].map((text) => [text, run(text).style.color])).toEqual([
    ["one", ""],
    ["t", ""],
    ["two\tend\x7f", "var(--ansi-1)"],
  ]);
  expect(screen.getByRole("button", { name: "Raw" }).className).toMatchSnapshot();
  fireEvent.click(screen.getByRole("button", { name: "Raw" }));
  expect(lines()).toEqual(["one␊", "t", "mark build", "␛[31mtwo␉end␡"]);
  await pushed(daemon, { segments: [{ text: " more\n" }, { mark: "end" }] });
  expect(lines()).toEqual(["one␊", "t", "mark build", "␛[31mtwo␉end␡ more␊", "mark end"]);
  fireEvent.click(screen.getByRole("button", { name: "Raw" }));
  expect(lines()).toEqual(["one", "t", "mark build", "two\tend\x7f more", "mark end"]);
});

test("a reset replaces the output", async () => {
  const daemon = await shownOutput();

  await pushed(daemon, { segments: [{ text: "old\n" }] });
  await pushed(daemon, { segments: [{ text: "new" }], reset: true });
  expect(lines()).toEqual(["new"]);
});

test("search counts and marks every occurrence, ignoring case", async () => {
  const daemon = await shownOutput();

  await pushed(daemon, { segments: [{ text: "Alpha beta\nALPHA alpha\n" }, { mark: "x" }] });
  const search = screen.getByRole("textbox", { name: "Search the output" });

  fireEvent.change(search, { target: { value: " alpha " } });
  expect([panel().getByRole("status").textContent, lines(), search.getAttribute("spellcheck")]).toEqual(["3 matches", ["Alpha beta", "ALPHA alpha", "mark x"], "false"]);
  expect(output().getAllByRole("mark").map((mark) => mark.textContent)).toEqual(["Alpha", "ALPHA", "alpha"]);
  fireEvent.change(search, { target: { value: "beta" } });
  expect(panel().getByRole("status").textContent).toBe("1 match");
  fireEvent.change(search, { target: { value: "gamma" } });
  expect(panel().getByRole("status").textContent).toBe("0 matches");
  fireEvent.change(search, { target: { value: "  " } });
  expect(panel().queryByRole("status")).toBeNull();
});

test("an open line continues across updates, in styled and in raw output", async () => {
  const daemon = await shownOutput();

  await pushed(daemon, { segments: [{ text: "a\nb" }] });
  await pushed(daemon, { segments: [{ text: "c\nd" }] });
  await pushed(daemon, { segments: [{ text: "x\ny\n" }] });
  expect(lines()).toEqual(["a", "bc", "dx", "y"]);
  fireEvent.click(screen.getByRole("button", { name: "Raw" }));
  expect(lines()).toEqual(["a␊", "bc␊", "dx␊", "y␊"]);
});

test("a line drawn in several styles is searched as one text", async () => {
  const daemon = await shownOutput();

  await pushed(daemon, { segments: [{ text: "\x1b[31mal\x1b[0mpha\n" }] });
  fireEvent.change(screen.getByRole("textbox", { name: "Search the output" }), { target: { value: "alpha" } });
  expect(panel().getByRole("status").textContent).toBe("1 match");
});
