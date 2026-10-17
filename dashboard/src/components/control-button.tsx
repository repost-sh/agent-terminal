import { Hand, MousePointerClick } from "lucide-react";

import { controlButtonId, releaseControl, takeControl, useControl } from "../lib/control";
import { focusWhenShown } from "../lib/focus";
import type { ToolbarTab } from "./toolbar-actions";
import { Button, cn } from "./ui";

const isLive = (props: ToolbarTab) =>
  props.stream === "open" && props.shown !== null && props.shown.frame > 0 && !props.shown.exited;

const CENTRED = "pointer-events-none absolute inset-0 z-10 flex items-center justify-center transition-opacity focus-within:opacity-100";

const BOTTOM = "pointer-events-none absolute inset-x-0 z-10 flex justify-center";

const TakeControl = (props: ToolbarTab & { moving: boolean }) => {
  const id = controlButtonId(props.session, props.tab.tab);

  if (props.shown?.exited) {
    return null;
  }

  return (
    <div data-testid="take-control" className={cn(CENTRED, props.moving ? "opacity-100 duration-0" : "opacity-0 duration-500")}>
      <Button look={{ variant: "inverted", icon: MousePointerClick, size: "pill" }} className="pointer-events-auto" id={id} ref={isLive(props) ? focusWhenShown(id) : undefined} disabled={!isLive(props)} onClick={() => {
          takeControl({ session: props.session, tab: props.tab.tab }, props.tab.program);
        }}>
        Take control
      </Button>
    </div>
  );
};

const ReleaseControl = (props: ToolbarTab & { taking: boolean }) => {
  const id = controlButtonId(props.session, props.tab.tab);

  return (
    <div data-testid="release-control" className={cn(BOTTOM, props.shown?.exited ? "bottom-16" : "bottom-4")}>
      <Button look={{ variant: "inverted", icon: Hand, size: "pill" }} className="pointer-events-auto" id={id} ref={props.taking ? undefined : focusWhenShown(id)} disabled={props.taking} onClick={() => {
          releaseControl(props.session, props.tab.tab);
        }}>
        Release control
      </Button>
    </div>
  );
};

export const ControlOverlay = (props: ToolbarTab & { moving: boolean }) => {
  const control = useControl(props.session, props.tab.tab);

  return control === "off" ? <TakeControl {...props} /> : <ReleaseControl {...props} taking={control === "taking"} />;
};
