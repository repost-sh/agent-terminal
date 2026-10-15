import { Hand } from "lucide-react";

import { controlButtonId, releaseControl, takeControl, useControl } from "../lib/control";
import { focusWhenShown } from "../lib/focus";
import type { ToolbarActionProps } from "../toolbar-actions";
import { Button } from "./ui";

const isLive = (props: ToolbarActionProps) =>
  props.stream === "open" && props.shown !== null && props.shown.frame > 0 && !props.shown.exited;

const toggle = (props: ToolbarActionProps, held: boolean) => {
  if (held) {
    releaseControl(props.session, props.tab.tab);
  } else {
    takeControl({ session: props.session, tab: props.tab.tab }, props.tab.program);
  }
};

export const ControlButton = (props: ToolbarActionProps) => {
  const control = useControl(props.session, props.tab.tab);
  const held = control !== "off";
  const disabled = held ? control === "taking" : !isLive(props);
  const id = controlButtonId(props.session, props.tab.tab);

  return (
    <Button look={{ variant: held ? "default" : "outline", icon: Hand }} id={id} ref={disabled ? undefined : focusWhenShown(id)} disabled={disabled} onClick={() => {
        toggle(props, held);
      }}>
      {held ? "Release control" : "Take control"}
    </Button>
  );
};
