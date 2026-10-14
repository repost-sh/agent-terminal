import { RotateCw } from "lucide-react";

import { tabName } from "../lib/tabs";
import type { ToolbarActionProps } from "../toolbar-actions";
import { closeDialog, Dialog, showDialog } from "./dialog";
import { Button, IconButton } from "./ui";

const RestartDialog = ({ name, program, onRestart }: { name: string; program: string; onRestart: () => Promise<boolean> }) => (
  <Dialog
    alert
    width={24}
    title={`Restart ${program}?`}
    description={`${name} is killed and started again with the same command, size and working directory.`}
    footer={
      <>
        <Button variant="ghost" data-autofocus onClick={() => closeDialog()}>
          Cancel
        </Button>
        <Button
          onClick={() => {
            closeDialog();
            void onRestart();
          }}
        >
          Restart
        </Button>
      </>
    }
  />
);

export const RestartButton = ({ session, tab, stream, shown, onRestart }: ToolbarActionProps) => {
  const drawn = shown !== null && (shown.frame > 0 || shown.exited);
  const restart = () => {
    if (shown?.exited) void onRestart();
    else showDialog(() => <RestartDialog name={tabName(session, tab.tab)} program={tab.program} onRestart={onRestart} />);
  };
  return <IconButton label={`Restart ${tab.program}`} tooltip="Restart" icon={RotateCw} disabled={stream !== "open" || !drawn} onClick={restart} />;
};
