import { Hand } from "lucide-react";

import { controlButtonId, releaseControl, takeControl, useControl } from "../lib/control";
import { focusWhenShown } from "../lib/focus";
import type { ToolbarTab } from "./toolbar-actions";
import { Button } from "./ui";

const isLive = (props: ToolbarTab) =>
  props.stream === "open" && props.shown !== null && props.shown.frame > 0 && !props.shown.exited;

const toggle = (props: ToolbarTab, held: boolean) => {
  if (held) {
    releaseControl(props.session, props.tab.tab);
  } else {
    takeControl({ session: props.session, tab: props.tab.tab }, props.tab.program);
  }
};

const PILL =
  "pointer-events-none absolute bottom-4 left-1/2 z-10 -translate-x-1/2 rounded-[1.25rem] border border-border bg-background/85 p-2 opacity-0 shadow-xl backdrop-blur-md transition-opacity duration-200 group-hover/viewport:pointer-events-auto group-hover/viewport:opacity-100 focus-within:pointer-events-auto focus-within:opacity-100 motion-reduce:transition-none";

export const ControlButton = (props: ToolbarTab) => {
  const control = useControl(props.session, props.tab.tab);
  const held = control !== "off";
  const disabled = held ? control === "taking" : !isLive(props);
  const id = controlButtonId(props.session, props.tab.tab);

  if (props.shown?.exited) {
    return null;
  }

  return (
    <div className={PILL}>
      <Button look={{ icon: Hand, size: "lg" }} id={id} ref={disabled ? undefined : focusWhenShown(id)} disabled={disabled} onClick={() => {
          toggle(props, held);
        }}>
        {held ? "Release control" : "Take control"}
      </Button>
    </div>
  );
};
