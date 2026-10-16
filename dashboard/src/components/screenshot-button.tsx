import { Camera } from "lucide-react";

import { takeScreenshot } from "../lib/api";
import type { ToolbarActionProps } from "./toolbar-actions";
import { toast } from "../lib/toasts";
import { IconButton } from "./ui";

export const ScreenshotButton = (props: ToolbarActionProps) => {
  const { shown } = props;
  const drawn = shown !== null && (shown.frame > 0 || shown.exited);
  const take = async () => {
    const path = await takeScreenshot(props.session, props.tab.tab);

    if (path) {
      toast("success", { title: `Saved ${path.split("/").pop() ?? path}`, description: path });
    }
  };

  return <IconButton look={{ label: "Take a screenshot", tooltip: "Screenshot", icon: Camera }} disabled={!drawn} onClick={() => void take()} />;
};
