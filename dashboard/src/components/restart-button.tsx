import { RotateCw } from "lucide-react";

import { reason, restartTab } from "../lib/api";
import { controlButtonId } from "../lib/control";
import { focusNowOrWhenShown } from "../lib/focus";
import type { TabInfo } from "../lib/protocol";
import { settle } from "../lib/settle";
import { tabName } from "../lib/tabs";
import type { ToolbarActionProps } from "../toolbar-actions";
import { announce } from "./announcer";
import { closeDialog, Dialog, showDialog } from "./dialog";
import { toast } from "./toaster";
import { Button, IconButton } from "./ui";

export const restartProgram = async (session: string, tab: TabInfo) => {
  const restarted = await settle(async () => {
    await restartTab(session, tab.tab);
    announce(`${tab.program} restarted`);
  });

  if (!restarted.ok) {
    toast("error", { title: `Could not restart ${tab.program}`, description: reason(restarted.error) });
  }

  return restarted.ok;
};

export const restartExited = async (session: string, tab: TabInfo) => {
  if (await restartProgram(session, tab)) {
    focusNowOrWhenShown(controlButtonId(session, tab.tab));
  }
};

interface RestartProps { session: string; tab: TabInfo }

const RestartDialog = ({ session, tab }: RestartProps) => (
  <Dialog
    alert
    width={24}
    title={`Restart ${tab.program}?`}
    description={`${tabName(session, tab.tab)} is killed and started again with the same command, size and working directory.`}
    footer={
      <>
        <Button look={{ variant: "ghost" }} data-autofocus onClick={() => {
          closeDialog();
        }}>
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

export const RestartButton = (props: ToolbarActionProps) => {
  const { shown } = props;
  const drawn = shown !== null && (shown.frame > 0 || shown.exited);
  const restart = () => {
    if (shown?.exited) {
      void restartExited(props.session, props.tab);
    } else {
      showDialog(() => <RestartDialog session={props.session} tab={props.tab} />);
    }
  };

  return (
    <IconButton look={{ label: `Restart ${props.tab.program}`, tooltip: "Restart", icon: RotateCw }} disabled={props.stream !== "open" || !drawn} onClick={restart} />
  );
};
