import { RotateCw } from "lucide-react";

import { reason, restartTab } from "../lib/api";
import type { TabInfo } from "../lib/protocol";
import { tabName } from "../lib/tabs";
import type { ToolbarActionProps } from "../toolbar-actions";
import { announce } from "./announcer";
import { closeDialog, Dialog, showDialog } from "./dialog";
import { toast } from "./toaster";
import { Button, IconButton } from "./ui";

export const restartProgram = async (session: string, tab: TabInfo) => {
  try {
    await restartTab(session, tab.tab);
    announce(`${tab.program} restarted`);
    return true;
  } catch (error) {
    toast("error", `Could not restart ${tab.program}`, reason(error));
    return false;
  }
};

const RestartDialog = ({ session, tab }: { session: string; tab: TabInfo }) => (
  <Dialog
    alert
    width={24}
    title={`Restart ${tab.program}?`}
    description={`${tabName(session, tab.tab)} is killed and started again with the same command, size and working directory.`}
    footer={
      <>
        <Button variant="ghost" data-autofocus onClick={() => closeDialog()}>
          Cancel
        </Button>
        <Button
          onClick={() => {
            closeDialog();
            void restartProgram(session, tab);
          }}
        >
          Restart
        </Button>
      </>
    }
  />
);

export const RestartButton = ({ session, tab, stream, shown }: ToolbarActionProps) => {
  const drawn = shown !== null && (shown.frame > 0 || shown.exited);
  const restart = () => {
    if (shown?.exited) void restartProgram(session, tab);
    else showDialog(() => <RestartDialog session={session} tab={tab} />);
  };
  return <IconButton label={`Restart ${tab.program}`} tooltip="Restart" icon={RotateCw} disabled={stream !== "open" || !drawn} onClick={restart} />;
};
