import { fireEvent, render, screen } from "@testing-library/react";
import type { Window as HappyWindow } from "happy-dom";
import { afterEach, expect, test, vi } from "vitest";

import { Disconnected, NothingOpen } from "./empty-screens";

const servedAt = (url: string) => {
  (window as unknown as HappyWindow).happyDOM.setURL(url);
};

afterEach(() => {
  servedAt("http://localhost:3000/");
});

test("no sessions offers the commands that start one and a button for a new session", () => {
  const { container } = render(<NothingOpen sessions={[]} />);

  expect(container.innerHTML).toMatchSnapshot();
});

test("a lost daemon says how to start it on the dashboard's port, and Retry asks again", () => {
  const retry = vi.fn();
  const started = (url: string) => {
    servedAt(url);
    const { unmount } = render(<Disconnected onRetry={retry} />);
    const said = [screen.getByRole("code").textContent, screen.getByText(/is not running/).textContent];

    unmount();

    return said;
  };

  expect([started("http://localhost:4849/"), started("http://localhost/"), started("http://localhost:3000/")]).toEqual([
    ["agent-terminal dashboard start", "The agent-terminal daemon is not running on :4849"],
    ["agent-terminal dashboard start", "The agent-terminal daemon is not running on :4849"],
    ["agent-terminal dashboard start --port 3000", "The agent-terminal daemon is not running on :3000"],
  ]);
  const { container } = render(<Disconnected onRetry={retry} />);

  expect(container.innerHTML).toMatchSnapshot();
  fireEvent.click(screen.getByRole("button", { name: "Retry" }));
  expect(retry).toHaveBeenCalledOnce();
});
