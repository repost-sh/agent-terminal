import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import { App } from "./app";
import { SESSIONS_STREAM, startDaemon } from "./fake-daemon/daemon";
import { sessionInfo, tabInfo } from "./fake-daemon/messages";
import { navigate } from "./lib/route";

test("the dashboard opens the first tab of the first session", async () => {
  const daemon = startDaemon([sessionInfo("empty", []), sessionInfo("work", [tabInfo("app", { program: "htop" })])]);

  render(<App />);
  expect(document.title).toBe("agent-terminal");
  await daemon.serve();
  expect(location.pathname).toBe("/s/work/app");
  expect(screen.getByRole("heading", { level: 1 }).textContent).toBe("work app · htop");
  expect(document.title).toBe("work app · htop · agent-terminal");
});

test("sessions without a tab name themselves and offer the commands and a New tab button for each", async () => {
  const daemon = startDaemon([sessionInfo("idle", []), sessionInfo("spare", [])]);

  render(<App />);
  await daemon.serve();
  const main = within(screen.getByRole("main"));

  expect([location.pathname, document.title, screen.getByRole("heading", { level: 1 }).textContent]).toEqual(["/", "No open tabs · agent-terminal", "No open tabs"]);
  expect(main.getByText("Open a tab in idle and spare from the CLI, or here")).toBeTruthy();
  expect(main.getAllByRole("code").map((line) => line.textContent)).toEqual([
    "agent-terminal --session <name> open --name <tab> <program>",
    "agent-terminal --session <name> shell --name <tab>",
    "agent-terminal skills get core",
  ]);
  expect(main.getAllByRole("button", { name: /^New/ }).map((button) => button.textContent)).toEqual(["New tab in idle", "New tab in spare"]);
  fireEvent.click(main.getByRole("button", { name: "New tab in spare" }));
  expect(within(screen.getByRole("dialog")).getByRole("heading").textContent).toBe("New tab");
  expect(within(screen.getByRole("dialog")).getByLabelText<HTMLSelectElement>("Session").value).toBe("spare");
  fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
});

test("a route the dashboard does not serve is a 404, and a closed tab says it is not open", async () => {
  const daemon = startDaemon([sessionInfo("work", [tabInfo("app")])]);

  navigate("/nowhere");
  render(<App />);
  await daemon.serve();
  expect(document.title).toBe("404: This page could not be found · agent-terminal");
  expect(screen.getByText("This page could not be found.")).toBeTruthy();
  await act(async () => {
    navigate("/s/work/t7");
    await Promise.resolve();
  });
  expect(screen.getByText("work t7 is not open")).toBeTruthy();
  fireEvent.click(screen.getByRole("link", { name: "Show open sessions" }));
  expect(location.pathname).toBe("/s/work/app");
});

test("a lost daemon is retried three times, then shown as down until Retry reconnects", async () => {
  vi.useFakeTimers();
  const daemon = startDaemon([sessionInfo("work", [tabInfo("app")])]);

  render(<App />);
  await daemon.serve();
  await daemon.hangUp(SESSIONS_STREAM);
  for (const wait of [500, 1000, 2000]) {
    await act(async () => {
      await vi.advanceTimersByTimeAsync(wait);
    });
    await daemon.refuse();
  }

  vi.useRealTimers();
  expect(document.title).toBe("Disconnected · agent-terminal");
  expect(screen.getByText("The agent-terminal daemon is not running on :3000")).toBeTruthy();
  expect(screen.getByText("agent-terminal dashboard start --port 3000")).toBeTruthy();
  expect(within(screen.getByRole("navigation", { name: "Sessions and tabs" })).getByText("Not connected")).toBeTruthy();
  expect(["Close all sessions", "New session"].map((name) => screen.getByRole<HTMLButtonElement>("button", { name }).disabled)).toEqual([true, true]);
  fireEvent.click(screen.getByRole("button", { name: "Retry" }));
  await daemon.serve();
  expect(screen.getByRole("status", { name: "Announcements" }).textContent).toBe("Connected");
  expect(location.pathname).toBe("/s/work/app");
});

test("the sidebar waits for the daemon before listing anything", async () => {
  const daemon = startDaemon([]);

  render(<App />);
  expect(within(screen.getByRole("navigation", { name: "Sessions and tabs" })).queryByText("No sessions")).toBeNull();
  await daemon.serve();
  expect(within(screen.getByRole("navigation", { name: "Sessions and tabs" })).getByText("No sessions")).toBeTruthy();
});

test("a skip link leads to the main content, which takes focus only by script", () => {
  startDaemon([]);
  render(<App />);
  const skip = screen.getByRole("link", { name: "Skip to content" });

  expect([skip.getAttribute("href"), screen.getByRole("main").id, screen.getByRole("main").tabIndex]).toEqual(["#main-content", "main-content", -1]);
});

test("the page behind an open dialog is inert", async () => {
  const daemon = startDaemon([sessionInfo("work", [tabInfo("app")])]);

  render(<App />);
  await daemon.serve();
  const behind = () => screen.getByRole("complementary", { name: "Sessions", hidden: true }).matches("[inert] *");

  expect(behind()).toBe(false);
  fireEvent.click(screen.getByRole("button", { name: "New session" }));
  expect(behind()).toBe(true);
  fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
  expect(behind()).toBe(false);
});

test("Retry goes home first, so a reconnected dashboard opens the first tab", async () => {
  vi.useFakeTimers();
  const daemon = startDaemon([sessionInfo("work", [tabInfo("app")])]);

  navigate("/nowhere");
  render(<App />);
  await daemon.serve();
  await daemon.hangUp(SESSIONS_STREAM);
  for (const wait of [500, 1000, 2000]) {
    await act(async () => {
      await vi.advanceTimersByTimeAsync(wait);
    });
    await daemon.refuse();
  }

  vi.useRealTimers();
  fireEvent.click(screen.getByRole("button", { name: "Retry" }));
  await daemon.serve();
  expect(location.pathname).toBe("/s/work/app");
});
