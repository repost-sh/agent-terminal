import { Hand, MousePointerClick, type LucideIcon } from "lucide-react";

import { controlButtonId, releaseControl, takeControl, useControl } from "../lib/control";
import { focusNext, focusWhenShown } from "../lib/focus";
import type { ToolbarTab } from "./toolbar-actions";
import { Button } from "./ui";

const isLive = (props: ToolbarTab) =>
  props.stream === "open" && props.shown !== null && props.shown.frame > 0 && !props.shown.exited;

interface PillProps { id: string; icon: LucideIcon; ready: boolean; onClick: () => void; children: string }

const Pill = (props: PillProps) => (
  <Button look={{ variant: "inverted", icon: props.icon, size: "pill" }} className="pointer-events-auto" id={props.id} ref={props.ready ? focusWhenShown(props.id) : undefined} disabled={!props.ready} onClick={props.onClick}>
    {props.children}
  </Button>
);

const TakeControl = (props: ToolbarTab) =>
  props.shown?.exited ? null : (
    <div data-testid="take-control" className="opacity-0 transition-opacity duration-500 focus-within:opacity-100 group-data-[moving=true]/viewport:opacity-100 group-data-[moving=true]/viewport:duration-0">
      <Pill id={controlButtonId(props.session, props.tab.tab)} icon={MousePointerClick} ready={isLive(props)} onClick={() => {
          takeControl({ session: props.session, tab: props.tab.tab }, props.tab.program);
        }}>
        Take control
      </Pill>
    </div>
  );

export const ControlOverlay = (props: ToolbarTab) => {
  const control = useControl(props.session, props.tab.tab);
  const id = controlButtonId(props.session, props.tab.tab);

  return control === "off" ? (
    <TakeControl {...props} />
  ) : (
    <Pill id={id} icon={Hand} ready={control === "held"} onClick={() => {
        focusNext(id);
        releaseControl(props.session, props.tab.tab);
      }}>
      Release control
    </Pill>
  );
};
