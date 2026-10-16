import { fireEvent, render, screen } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import { App } from "../app";
import { startDaemon } from "../fake-daemon/daemon";
import { frameOf, sessionInfo, tabInfo } from "../fake-daemon/messages";

const STREAM = "/api/session/work/app/stream";

const shownApp = async () => {
  const daemon = startDaemon([sessionInfo("work", [tabInfo("app", { program: "vim" })])]);

  render(<App />);
  await daemon.serve();
  await daemon.connected(STREAM);

  return daemon;
};

const restart = () => screen.getByRole<HTMLButtonElement>("button", { name: "Restart vim" });

test("restart waits for a drawn frame on an open stream", async () => {
  const daemon = await shownApp();

  expect([restart().disabled, restart().dataset["tooltip"]]).toEqual([true, "Restart"]);
  await daemon.push(STREAM, frameOf(0));
  await vi.waitFor(() => {
    expect(daemon.sent(STREAM)).toHaveLength(1);
  });
  expect(restart().disabled).toBe(true);
  await daemon.push(STREAM, frameOf(1));
  await screen.findByRole("group", { name: /live frame 1/ });
  expect(restart().disabled).toBe(false);
  await daemon.hangUp(STREAM);
  expect(restart().disabled).toBe(true);
});

test("restarting an exited program skips the question and hands focus to taking control once the program runs", async () => {
  const daemon = await shownApp();

  daemon.answer("POST /api/session/work/app/restart", () => ({ status: 200, body: { data: null } }));
  await daemon.push(STREAM, frameOf(1, { exited: true, exit_code: 0 }));
  await screen.findByRole("button", { name: "Close tab" });
  fireEvent.click(restart());
  expect(screen.queryByRole("alertdialog")).toBeNull();
  expect(await screen.findByText("vim restarted")).toBeTruthy();
  await daemon.push(STREAM, frameOf(2));
  await vi.waitFor(() => {
    expect(screen.getByRole("button", { name: "Take control" })).toHaveFocus();
  });
});
