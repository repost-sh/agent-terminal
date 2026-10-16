import { act, fireEvent, render, screen } from "@testing-library/react";
import { Copy } from "lucide-react";
import { expect, test, vi } from "vitest";

import { toasts } from "../lib/toasts";
import { Badge, Button, Chip, CliBlock, cn, CopyButton, Dot, IconButton, IconLink } from "./ui";

test("class names join what is set and skip what is not", () => {
  expect(cn("a", false, null, undefined, "", "b")).toBe("a b");
});

test("buttons carry the look of their variant and size, with an optional icon", () => {
  const { container } = render(
    <>
      <Button>Plain</Button>
      <Button look={{ variant: "ghost", size: "icon-sm", icon: Copy }} className="extra">
        Ghost
      </Button>
      <Button look={{ variant: "outline" }}>Outline</Button>
      <Button look={{ variant: "destructive" }} type="submit">
        Destructive
      </Button>
    </>,
  );

  expect(container.innerHTML).toMatchSnapshot();
});

test("icon buttons and links name their action for screen readers and tooltips", () => {
  const { container } = render(
    <>
      <IconButton look={{ label: "Copy line", icon: Copy }} />
      <IconButton look={{ label: "Copy", tooltip: "Copy the line", icon: Copy, kind: "head" }} className="extra" />
      <IconLink href="/s/work/app" look={{ label: "Open work app", icon: Copy }} />
      <IconLink href="/x" look={{ label: "X", tooltip: "Go to x", icon: Copy }} className="extra" />
    </>,
  );

  expect(container.innerHTML).toMatchSnapshot();
});

test("badges, chips and dots carry the look of their tone", () => {
  const { container } = render(
    <>
      <Badge>2</Badge>
      <Badge variant="outline" className="extra">
        3
      </Badge>
      <Chip tone="muted">idle</Chip>
      <Chip tone="destructive">exit 1</Chip>
      <Chip tone="warning">refused</Chip>
      {(["success", "warning", "destructive"] as const).map((tone) => (
        <Dot key={tone} tone={tone} />
      ))}
      <Dot tone="muted" className="extra" />
    </>,
  );

  expect(container.innerHTML).toMatchSnapshot();
});

test("a CLI line copies to the clipboard and says whether it could", async () => {
  const writeText = vi.fn(() => Promise.resolve());

  vi.stubGlobal("navigator", { clipboard: { writeText } });
  const { container } = render(<CliBlock line="agent-terminal open htop" />);

  expect(container.innerHTML).toMatchSnapshot();
  await act(async () => {
    fireEvent.click(screen.getByRole("button", { name: "Copy agent-terminal open htop" }));
    await Promise.resolve();
  });
  expect([writeText.mock.calls, toasts.get()[0]]).toEqual([
    [["agent-terminal open htop"]],
    expect.objectContaining({ type: "success", title: "Copied", description: "agent-terminal open htop" }),
  ]);
  writeText.mockImplementation(() => Promise.reject(new Error("denied")));
  await act(async () => {
    fireEvent.click(screen.getByRole("button", { name: "Copy agent-terminal open htop" }));
    await Promise.resolve();
  });
  expect(toasts.get()[0]).toEqual(expect.objectContaining({ type: "error", title: "Could not copy", description: "denied" }));
});

const copyLook = () => {
  const button = screen.getByRole("button", { name: "Copy the line" });

  return [button.getAttribute("data-tooltip"), button.innerHTML.includes("lucide-check"), button.className.includes("text-success")];
};

const clickCopy = async () => {
  await act(async () => {
    fireEvent.click(screen.getByRole("button", { name: "Copy the line" }));
    await Promise.resolve();
  });
};

test("a copy button copies its value and shows a green check for 1.5 s", async () => {
  const writeText = vi.fn(() => Promise.resolve());

  vi.stubGlobal("navigator", { clipboard: { writeText } });
  vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
  render(<CopyButton value="cd /srv && htop" label="Copy the line" />);
  expect(copyLook()).toEqual(["Copy", false, false]);
  await clickCopy();
  expect([writeText.mock.calls, copyLook()]).toEqual([[["cd /srv && htop"]], ["Copied", true, true]]);
  act(() => {
    vi.advanceTimersByTime(1499);
  });
  expect(copyLook()).toEqual(["Copied", true, true]);
  act(() => {
    vi.advanceTimersByTime(1);
  });
  expect(copyLook()).toEqual(["Copy", false, false]);
  vi.useRealTimers();
});

test("a copy clicked again shows the check for 1.5 s from the last click", async () => {
  vi.stubGlobal("navigator", { clipboard: { writeText: () => Promise.resolve() } });
  vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
  render(<CopyButton value="htop" label="Copy the line" />);
  await clickCopy();
  act(() => {
    vi.advanceTimersByTime(1000);
  });
  await clickCopy();
  act(() => {
    vi.advanceTimersByTime(1499);
  });
  expect(copyLook()).toEqual(["Copied", true, true]);
  act(() => {
    vi.advanceTimersByTime(1);
  });
  expect(copyLook()).toEqual(["Copy", false, false]);
  vi.useRealTimers();
});

test("a refused copy keeps the copy icon", async () => {
  vi.stubGlobal("navigator", { clipboard: { writeText: () => Promise.reject(new Error("denied")) } });
  render(<CopyButton value="htop" label="Copy the line" />);
  await clickCopy();
  expect([copyLook(), toasts.get()[0]]).toEqual([["Copy", false, false], expect.objectContaining({ type: "error", title: "Could not copy" })]);
});
