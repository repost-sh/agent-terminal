import { Hand } from "lucide-react";

import { controlButtonId, releaseControl, takeControl, useControl } from "../lib/control";
import { focusWhenShown } from "../lib/focus";
import type { ToolbarActionProps } from "../toolbar-actions";
import { Button } from "./ui";

export const ControlButton = ({ session, tab, stream, shown }: ToolbarActionProps) => {
  const control = useControl(session, tab.tab);
  const held = control !== "off";
  const live = stream === "open" && shown !== null && shown.frame > 0 && !shown.exited;
  const disabled = held ? control === "taking" : !live;
  const id = controlButtonId(session, tab.tab);
  return (
    <Button
      id={id}
      ref={disabled ? undefined : focusWhenShown(id)}
      variant={held ? "default" : "outline"}
      icon={Hand}
      disabled={disabled}
      onClick={() => (held ? releaseControl(session, tab.tab) : takeControl(session, tab.tab, tab.program))}
    >
      {held ? "Release control" : "Take control"}
    </Button>
  );
};
