import { Camera } from "lucide-react";

import { reason, takeScreenshot } from "../lib/api";
import type { ToolbarActionProps } from "../toolbar-actions";
import { toast } from "./toaster";
import { IconButton } from "./ui";

export const ScreenshotButton = ({ session, tab, shown }: ToolbarActionProps) => {
  const drawn = shown !== null && (shown.frame > 0 || shown.exited);
  const take = async () => {
    try {
      const path = await takeScreenshot(session, tab.tab);
      toast("success", `Saved ${path.split("/").pop() ?? path}`, path);
    } catch (error) {
      toast("error", "Could not take a screenshot", reason(error));
    }
  };
  return <IconButton label="Take a screenshot" tooltip="Screenshot" icon={Camera} disabled={!drawn} onClick={take} />;
};
