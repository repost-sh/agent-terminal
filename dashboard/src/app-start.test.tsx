import { act, render, screen } from "@testing-library/react";
import { expect, test } from "vitest";

import { App } from "./app";
import { startDaemon } from "./fake-daemon/daemon";
import { navigate } from "./lib/route";

const named = () => [document.title, screen.getByRole("heading", { level: 1 }).textContent, screen.getByRole("alert", { name: "Screen" }).textContent];

test("the page names each screen in its title, its heading and what it tells a screen reader", async () => {
  const daemon = startDaemon([]);

  render(<App />);
  expect(named()).toEqual(["agent-terminal", "agent-terminal", ""]);
  await daemon.serve();
  expect(named()).toEqual(["No active sessions · agent-terminal", "No active sessions", "No active sessions"]);
  await act(async () => {
    navigate("/s/work/t9");
    await Promise.resolve();
  });
  expect(document.title).toBe("Not open · agent-terminal");
});
