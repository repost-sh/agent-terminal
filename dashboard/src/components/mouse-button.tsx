import { Mouse, MouseOff } from "lucide-react";

import { toggleMouse, useMouseWanted } from "../lib/mouse-preference";
import { Button } from "./ui";

const TOOLTIP = {
  on: "While you hold control, the mouse goes to the app when it asks for it; shift+click selects text",
  off: "The mouse selects text and never reaches the app",
};

export const MouseButton = () => {
  const wanted = useMouseWanted();

  return (
    <Button look={{ variant: wanted ? "default" : "outline", icon: wanted ? Mouse : MouseOff }} aria-pressed={wanted} data-tooltip={wanted ? TOOLTIP.on : TOOLTIP.off} onClick={toggleMouse}>
      {wanted ? "Mouse on" : "Mouse off"}
    </Button>
  );
};
