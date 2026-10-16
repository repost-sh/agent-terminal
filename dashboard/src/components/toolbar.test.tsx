import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import { App } from "../app";
import { startDaemon } from "../fake-daemon/daemon";
import { sessionInfo, tabInfo } from "../fake-daemon/messages";
import { navigate } from "../lib/route";

const CWD = "/Users/me/orca/workspaces/cross-border/feat banking provider routefusion";

const COMMAND = "sh -c 'echo two words; read line'";

const shownTab = async (path: string) => {
  const daemon = startDaemon([sessionInfo("work", [tabInfo("app", { command: COMMAND, cwd: CWD }), tabInfo("api", { command: "make all", cwd: "/srv/api" })])]);

  navigate(path);
  render(<App />);
  await daemon.serve();
};

const commandLine = () => screen.getByRole("group", { name: "Command line" });

const parts = () => within(commandLine()).getAllByText(/./, { ignore: ".sr-only" });

const wrapping = (element: HTMLElement) => [...element.classList].filter((name) => /^(flex-wrap|break-)/.test(name));

test("the command line keeps the command and its working directory on one line each, in full on hover", async () => {
  await shownTab("/s/work/app");
  expect([
    wrapping(commandLine()),
    parts().map((part) => [part.textContent, part.getAttribute("data-tooltip"), part.classList.contains("truncate"), wrapping(part)]),
  ]).toEqual([
    [],
    [
      [COMMAND, COMMAND, true, []],
      [CWD, CWD, true, []],
    ],
  ]);
});

const copied = async () => {
  await act(async () => {
    fireEvent.click(within(commandLine()).getByRole("button", { name: "Copy the command with its working directory" }));
    await Promise.resolve();
  });
};

test("the command line copies as a cd into its working directory, quoted only when it has to be", async () => {
  const writeText = vi.fn(() => Promise.resolve());

  await shownTab("/s/work/app");
  vi.stubGlobal("navigator", { clipboard: { writeText } });
  await copied();
  act(() => {
    navigate("/s/work/api");
  });
  await copied();
  expect(writeText.mock.calls).toEqual([[`cd '${CWD}' && ${COMMAND}`], ["cd /srv/api && make all"]]);
});
