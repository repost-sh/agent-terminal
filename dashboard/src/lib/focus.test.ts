import { expect, test, vi } from "vitest";

import { focusNext, focusNowOrWhenShown, focusWhenShown } from "./focus";

const button = (id: string, disabled = false) => {
  const element = document.createElement("button");

  element.id = id;
  element.disabled = disabled;
  document.body.append(element);

  return { element, focus: vi.spyOn(element, "focus") };
};

test("an element that is shown and enabled takes focus at once, without scrolling", () => {
  const shown = button("now");

  focusNowOrWhenShown("now");
  expect(shown.focus).toHaveBeenCalledWith({ preventScroll: true });
  shown.element.remove();
});

test("a disabled or missing element takes focus when it is next shown, once", () => {
  const disabled = button("later", true);

  focusNowOrWhenShown("later");
  expect(disabled.focus).not.toHaveBeenCalled();
  focusWhenShown("other")(disabled.element);
  expect(disabled.focus).not.toHaveBeenCalled();
  focusWhenShown("later")(null);
  focusWhenShown("later")(disabled.element);
  expect(disabled.focus).toHaveBeenCalledWith({ preventScroll: true });
  focusWhenShown("later")(disabled.element);
  expect(disabled.focus).toHaveBeenCalledOnce();
  disabled.element.remove();
  focusNowOrWhenShown("missing");
  const missing = button("missing");

  focusWhenShown("missing")(missing.element);
  expect(missing.focus).toHaveBeenCalledOnce();
  missing.element.remove();
  focusNext("named");
  const named = button("named");

  focusWhenShown("named")(named.element);
  expect(named.focus).toHaveBeenCalledOnce();
  named.element.remove();
});
