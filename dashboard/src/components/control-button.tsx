import { Hand } from "lucide-react";

import { controlButtonId, releaseControl, takeControl, useControl } from "../lib/control";
import type { TabInfo } from "../lib/protocol";
import type { Shown, StreamState } from "../lib/stream";
import { Button } from "./ui";

type ControlButtonProps = { session: string; tab: TabInfo; stream: StreamState; shown: Shown | null };

export const ControlButton = ({ session, tab, stream, shown }: ControlButtonProps) => {
  const control = useControl(session, tab.tab);
  const held = control !== "off";
  const live = stream === "open" && shown !== null && shown.frame > 0 && !shown.exited;
  return (
    <Button
      id={controlButtonId(session, tab.tab)}
      variant={held ? "default" : "outline"}
      icon={Hand}
      disabled={held ? control === "taking" : !live}
      onClick={() => (held ? releaseControl(session, tab.tab) : takeControl(session, tab.tab, tab.program))}
    >
      {held ? "Release control" : "Take control"}
    </Button>
  );
};
