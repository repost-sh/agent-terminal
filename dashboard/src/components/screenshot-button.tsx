import { Camera } from "lucide-react";

import { takeScreenshot } from "../lib/api";
import type { ToolbarActionProps } from "../toolbar-actions";
import { toast } from "./toaster";
import { IconButton } from "./ui";

export const ScreenshotButton = ({ session, tab, shown }: ToolbarActionProps) => {
  const drawn = shown !== null && (shown.frame > 0 || shown.exited);
  const take = async () => {
    const path = await takeScreenshot(session, tab.tab);
    if (path) toast("success", `Saved ${path.split("/").pop() ?? path}`, path);
  };
  return <IconButton label="Take a screenshot" tooltip="Screenshot" icon={Camera} disabled={!drawn} onClick={() => void take()} />;
};
