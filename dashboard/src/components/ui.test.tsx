import { act, fireEvent, render, screen } from "@testing-library/react";
import { Copy } from "lucide-react";
import { expect, test, vi } from "vitest";

import { toasts } from "../lib/toasts";
import { Badge, Button, Chip, CliBlock, cn, Dot, IconButton, IconLink } from "./ui";

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
