import { fireEvent, render, screen, within } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import { App } from "../app";
import { startDaemon } from "../fake-daemon/daemon";
import { sessionInfo, tabInfo } from "../fake-daemon/messages";

const shownApp = async () => {
  const daemon = startDaemon([sessionInfo("work", [tabInfo("t1")])]);

  render(<App />);
  await daemon.serve();

  return daemon;
};

const selectedPanel = () => screen.getByRole("tab", { selected: true }).textContent;

test("the details panels switch by click and by arrow, Home and End keys", async () => {
  await shownApp();
  const details = screen.getByRole("region", { name: "Details of work t1" });
  const tabs = within(details).getByRole("tablist", { name: "Details" });

  fireEvent.click(within(tabs).getByRole("tab", { name: "Activity" }));
  expect(selectedPanel()).toBe("Activity");
  fireEvent.keyDown(tabs, { key: "ArrowRight" });
  expect([selectedPanel(), document.activeElement?.id]).toEqual(["Commands", "ptab-commands"]);
  fireEvent.keyDown(tabs, { key: "End" });
  expect(selectedPanel()).toBe("Info");
  fireEvent.keyDown(tabs, { key: "ArrowRight" });
  expect(selectedPanel()).toBe("Activity");
  fireEvent.keyDown(tabs, { key: "ArrowLeft" });
  expect(selectedPanel()).toBe("Info");
  fireEvent.keyDown(tabs, { key: "Home" });
  expect(selectedPanel()).toBe("Activity");
  expect(fireEvent.keyDown(tabs, { key: "x" })).toBe(true);
  expect(selectedPanel()).toBe("Activity");
});

test("the sidebar and the side panel resize by arrow keys within their limits", async () => {
  await shownApp();
  const sidebar = screen.getByRole("separator", { name: "Resize the sessions sidebar" });
  const panel = screen.getByRole("separator", { name: "Resize the side panel" });

  fireEvent.keyDown(sidebar, { key: "ArrowRight" });
  fireEvent.keyDown(panel, { key: "ArrowRight" });
  expect([sidebar.getAttribute("aria-valuenow"), panel.getAttribute("aria-valuenow")]).toEqual(["17", "28"]);
  fireEvent.keyDown(sidebar, { key: "ArrowLeft" });
  fireEvent.keyDown(panel, { key: "ArrowLeft" });
  expect([sidebar.getAttribute("aria-valuenow"), panel.getAttribute("aria-valuenow")]).toEqual(["15", "30"]);
  for (let press = 0; press < 20; press += 1) {
    fireEvent.keyDown(sidebar, { key: "ArrowLeft" });
  }

  expect(fireEvent.keyDown(sidebar, { key: "Enter" })).toBe(true);
  expect(sidebar.getAttribute("aria-valuenow")).toBe("10");
});

test("dragging a handle sizes its column as a share of the row", async () => {
  await shownApp();
  const sidebar = screen.getByRole("separator", { name: "Resize the sessions sidebar" });
  const panel = screen.getByRole("separator", { name: "Resize the side panel" });
  const row = new DOMRect(0, 0, 1000, 600);

  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue(row);
  fireEvent.pointerDown(sidebar, { pointerId: 1 });
  fireEvent.pointerMove(sidebar, { clientX: 200 });
  fireEvent.pointerUp(sidebar);
  fireEvent.pointerMove(sidebar, { clientX: 290 });
  fireEvent.pointerDown(panel, { pointerId: 2 });
  fireEvent.pointerMove(panel, { clientX: 600 });
  expect([sidebar.getAttribute("aria-valuenow"), panel.getAttribute("aria-valuenow")]).toEqual(["20", "40"]);
  fireEvent.pointerMove(panel, { clientX: 0 });
  expect(panel.getAttribute("aria-valuenow")).toBe("50");
});
