import { fireEvent, render, screen, within } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import { App } from "../app";
import { startDaemon, type Request } from "../fake-daemon/daemon";
import { frameOf, sessionInfo, tabInfo } from "../fake-daemon/messages";
import type { SessionInfo } from "../lib/protocol";

const shownApp = async (sessions: SessionInfo[]) => {
  const daemon = startDaemon(sessions);

  render(<App />);
  await daemon.serve();

  return daemon;
};

const dialog = () => screen.getByRole("dialog");

const typeInto = (label: string, value: string) => {
  fireEvent.change(within(dialog()).getByLabelText(label), { target: { value } });
};

const submit = () => {
  fireEvent.submit(within(dialog()).getByRole("button", { name: /Create|Open tab/ }));
};

const focused = (element: HTMLElement) => element.matches(":focus");

const opened = (request: Request) => ({ status: 200, body: { data: { session: "session-1", tab: "t1" } }, sessions: [sessionInfo((request.body as { session: string }).session, [tabInfo("t1")])] });

test("a first session is named for you, needs a command, and opens on its tab", async () => {
  const daemon = await shownApp([]);

  daemon.answer("POST /api/sessions", opened);
  fireEvent.click(within(screen.getByRole("main")).getByRole("button", { name: "New session" }));
  expect(within(dialog()).queryByRole("group", { name: "Open in" })).toBeNull();
  const name = within(dialog()).getByLabelText<HTMLInputElement>("Session name");

  expect([focused(name), name.value, within(dialog()).getByRole("heading").textContent]).toEqual([true, "session-1", "New session"]);
  submit();
  expect(within(dialog()).getByRole("alert").textContent).toBe("Enter a command.");
  expect(focused(within(dialog()).getByLabelText("Command"))).toBe(true);
  typeInto("Command", "lazygit");
  typeInto("Working directory", "/srv");
  expect(within(dialog()).getByLabelText("Same as").textContent).toBe("agent-terminal session new session-1 && agent-terminal --session session-1 open lazygit --cwd /srv --size 120x40");
  submit();
  expect(await screen.findByRole("heading", { name: /session-1 t1/ })).toBeTruthy();
  expect(daemon.requests.find((request) => request.path === "/api/sessions")?.body).toEqual({
    session: "session-1",
    new_session: true,
    command: "lazygit",
    cwd: "/srv",
    cols: 120,
    rows: 40,
    shell_integration: false,
  });
  expect([location.pathname, screen.queryByRole("dialog")]).toEqual(["/s/session-1/t1", null]);
  await daemon.connected("/api/session/session-1/t1/stream");
  await daemon.push("/api/session/session-1/t1/stream", frameOf(1));
  expect(await screen.findByText("session-1 t1 is live")).toBeTruthy();
});

test("a new tab in an existing session sends the shell, its integration and a custom size", async () => {
  const daemon = await shownApp([sessionInfo("work", [tabInfo("t1")]), sessionInfo("play", [tabInfo("t1")])]);

  daemon.answer("POST /api/sessions", () => ({ status: 200, body: { data: { session: "play", tab: "t2" } } }));
  fireEvent.click(screen.getByRole("button", { name: "New tab in play" }));
  expect(within(dialog()).getByRole("heading").textContent).toBe("New tab");
  expect(within(dialog()).getByLabelText<HTMLSelectElement>("Session").value).toBe("play");
  fireEvent.change(within(dialog()).getByLabelText("Session"), { target: { value: "work" } });
  typeInto("Command", "/bin/zsh -l");
  expect(within(dialog()).getByRole("switch", { name: "Shell integration" }).getAttribute("aria-checked")).toBe("true");
  fireEvent.click(within(dialog()).getByLabelText("Custom"));
  typeInto("Columns", "19");
  submit();
  expect(within(dialog()).getByRole("group", { name: "Size", description: "Columns 20–500, rows 5–200." }).getAttribute("aria-invalid")).toBeNull();
  expect(focused(within(dialog()).getByLabelText("Custom"))).toBe(true);
  typeInto("Columns", "100");
  typeInto("Rows", "30");
  expect(within(dialog()).getByLabelText("Same as").textContent).toBe("agent-terminal --session work shell --shell zsh --size 100x30");
  fireEvent.click(within(dialog()).getByRole("switch", { name: "Shell integration" }));
  expect(within(dialog()).getByLabelText("Same as").textContent).toBe("agent-terminal --session work open /bin/zsh -l --size 100x30");
  submit();
  await vi.waitFor(() => {
    expect(location.pathname).toBe("/s/play/t2");
  });
  expect(daemon.requests.find((request) => request.path === "/api/sessions")?.body).toEqual({
    session: "work",
    new_session: false,
    command: "/bin/zsh -l",
    cwd: "~",
    cols: 100,
    rows: 30,
    shell_integration: false,
  });
});

test("names are checked before anything is sent, and a refused open keeps the dialog", async () => {
  const daemon = await shownApp([sessionInfo("work", [tabInfo("t1")])]);

  daemon.answer("POST /api/sessions", () => ({ status: 400, body: { error: { code: 1, message: "lazygit: not found" } } }));
  fireEvent.click(screen.getByRole("button", { name: "New session" }));
  typeInto("Command", "lazygit");
  for (const [name, refusal] of [["", "Enter a session name."], ["Repost", "Use up to 24 of a-z, 0-9 and -, starting with a letter or digit."], ["work", "A session named work already exists."]]) {
    typeInto("Session name", String(name));
    submit();
    const field = within(dialog()).getByRole("textbox", { name: "Session name", description: String(refusal) });

    expect([field.getAttribute("aria-invalid"), focused(field)]).toEqual(["true", true]);
  }

  typeInto("Session name", "fresh");
  fireEvent.click(within(dialog()).getByLabelText("80×24"));
  submit();
  expect(await screen.findByText("lazygit: not found")).toBeTruthy();
  expect([focused(within(dialog()).getByLabelText("Command")), within(dialog()).getByRole<HTMLButtonElement>("button", { name: "Create" }).disabled]).toEqual([true, false]);
  expect(daemon.requests.filter((request) => request.path === "/api/sessions")).toHaveLength(1);
  fireEvent.click(within(dialog()).getByRole("button", { name: "Cancel" }));
  expect(screen.queryByRole("dialog")).toBeNull();
});

const radio = (label: string) => {
  fireEvent.click(within(dialog()).getByLabelText(label));
};

test("the dialog shows the fields its mode, size and shell call for", async () => {
  await shownApp([sessionInfo("work", [tabInfo("t1")])]);
  fireEvent.click(screen.getByRole("button", { name: "New session" }));
  expect(dialog().outerHTML).toMatchSnapshot();
  radio("Existing session");
  expect(dialog().outerHTML).toMatchSnapshot();
  typeInto("Command", "bash");
  radio("Custom");
  expect(dialog().outerHTML).toMatchSnapshot();
  radio("New session");
  expect(within(dialog()).getByRole("heading").textContent).toBe("New session");
  fireEvent.click(within(dialog()).getByRole("button", { name: "Cancel" }));
});

test("the command line copies, and a refused field takes focus without scrolling", async () => {
  const writeText = vi.fn(() => Promise.resolve());

  await shownApp([]);
  vi.stubGlobal("navigator", { clipboard: { writeText } });
  fireEvent.click(within(screen.getByRole("main")).getByRole("button", { name: "New session" }));
  fireEvent.click(within(dialog()).getByRole("button", { name: "Copy the command line" }));
  expect(writeText.mock.calls).toEqual([["agent-terminal session new session-1 && agent-terminal --session session-1 open <program> --size 120x40"]]);
  const focus = vi.spyOn(HTMLElement.prototype, "focus");

  submit();
  expect(focus.mock.calls).toEqual([[{ preventScroll: true }]]);
  fireEvent.click(within(dialog()).getByRole("button", { name: "Cancel" }));
});

test("the dialog cannot be sent twice while it opens, opens on the Activity panel and returns focus to the page", async () => {
  const daemon = await shownApp([sessionInfo("work", [tabInfo("t1")])]);
  const create = () => within(dialog()).getByRole<HTMLButtonElement>("button", { name: "Create" });

  daemon.answer("POST /api/sessions", () => ({ status: 200, body: { data: { session: "session-2", tab: "t1" } }, sessions: [sessionInfo("session-2", [tabInfo("t1")])] }));
  fireEvent.click(screen.getByRole("tab", { name: "Output" }));
  fireEvent.click(screen.getByRole("button", { name: "New session" }));
  typeInto("Command", "lazygit");
  let release: (value: null) => void = () => undefined;
  const held = new Promise((resolve) => {
    release = resolve;
  });
  const fetching = globalThis.fetch.bind(globalThis);

  vi.stubGlobal("fetch", async (...args: Parameters<typeof fetch>) => {
    await held;

    return fetching(...args);
  });
  submit();
  expect(create().disabled).toBe(true);
  release(null);
  await vi.waitFor(() => {
    expect(screen.queryByRole("dialog")).toBeNull();
  });
  const activity = await screen.findByRole("tab", { name: "Activity" });

  expect([focused(screen.getByRole("main")), activity.getAttribute("aria-selected")]).toEqual([true, "true"]);
});
