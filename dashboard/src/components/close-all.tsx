import { closeAll, reason } from "../lib/api";
import type { SessionInfo } from "../lib/protocol";
import { navigate } from "../lib/route";
import { announce } from "./announcer";
import { closeDialog, Dialog } from "./dialog";
import { toast } from "./toaster";
import { Button } from "./ui";

const plural = (count: number, one: string, many: string) => `${count} ${count === 1 ? one : many}`;

export const CloseAllDialog = ({ sessions }: { sessions: SessionInfo[] }) => {
  const running = sessions.flatMap((entry) => entry.tabs).filter((tab) => !tab.exited).length;
  const closed = plural(sessions.length, "session", "sessions");
  const confirm = async () => {
    try {
      await closeAll();
      closeDialog(document.getElementById("main-content"));
      navigate("/");
      announce(`Closed ${closed}`);
    } catch (error) {
      toast("error", "Could not close all sessions", reason(error));
    }
  };
  return (
    <Dialog
      alert
      width={24}
      title="Close all sessions"
      description={`This will close ${closed} and kill ${plural(running, "running process", "running processes")}. This action cannot be undone.`}
      footer={
        <>
          <Button variant="ghost" data-autofocus onClick={() => closeDialog()}>
            Cancel
          </Button>
          <Button variant="destructive" onClick={confirm}>
            Close all
          </Button>
        </>
      }
    />
  );
};
