import { closeAll, reason } from "../lib/api";
import type { SessionInfo } from "../lib/protocol";
import { navigate } from "../lib/route";
import { settle } from "../lib/settle";
import { announce } from "../lib/announcements";
import { closeDialog, Dialog } from "./dialog";
import { toast } from "../lib/toasts";
import { Button } from "./ui";

interface Noun { one: string; many: string }

const plural = (count: number, noun: Noun) => `${String(count)} ${count === 1 ? noun.one : noun.many}`;

interface CloseAllProps { sessions: SessionInfo[] }

export const CloseAllDialog = ({ sessions }: CloseAllProps) => {
  const running = sessions.flatMap((entry) => entry.tabs).filter((tab) => !tab.exited).length;
  const closed = plural(sessions.length, { one: "session", many: "sessions" });
  const confirm = async () => {
    const result = await settle(async () => {
      await closeAll();
      closeDialog(document.getElementById("main-content"));
      navigate("/");
      announce(`Closed ${closed}`);
    });

    if (!result.ok) {
      toast("error", { title: "Could not close all sessions", description: reason(result.error) });
    }
  };

  return (
    <Dialog
      alert
      width={24}
      title="Close all sessions"
      description={`This will close ${closed} and kill ${plural(running, { one: "running process", many: "running processes" })}. This action cannot be undone.`}
      footer={
        <>
          <Button look={{ variant: "ghost" }} data-autofocus onClick={() => {
            closeDialog();
          }}>
            Cancel
          </Button>
          <Button look={{ variant: "destructive" }} onClick={() => {
            void confirm();
          }}>
            Close all
          </Button>
        </>
      }
    />
  );
};
