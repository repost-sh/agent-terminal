import { Plus, RotateCw } from "lucide-react";

import { navigate } from "../lib/route";
import { showDialog } from "./dialog";
import { NewSessionDialog } from "./new-session";
import { Button, CliBlock, Dot } from "./ui";

const DEFAULT_PORT = "4849";

const START_COMMANDS = ["agent-terminal open <program>", "agent-terminal shell", "agent-terminal skills get core"];

export const NoSessions = () => (
  <div className="flex h-full items-center justify-center p-6">
    <div className="w-full max-w-sm space-y-4 text-center">
      <div className="space-y-2">
        <p className="text-sm text-muted-foreground">No active sessions</p>
        <p className="text-xs text-muted-foreground/60">Start a terminal from the CLI, or create a session here</p>
      </div>
      <ul className="space-y-1.5" aria-label="Commands to get started">
        {START_COMMANDS.map((line) => (
          <li key={line}>
            <CliBlock line={line} />
          </li>
        ))}
      </ul>
      <Button icon={Plus} onClick={() => showDialog((live) => <NewSessionDialog sessions={live} existing={null} />)}>
        New session
      </Button>
    </div>
  </div>
);

export const Disconnected = ({ onRetry }: { onRetry: () => void }) => {
  const port = location.port || DEFAULT_PORT;
  const start = port === DEFAULT_PORT ? "agent-terminal dashboard start" : `agent-terminal dashboard start --port ${port}`;
  return (
    <div className="flex h-full items-center justify-center p-6">
      <div className="w-full max-w-sm space-y-4 text-center">
        <div className="space-y-2">
          <p className="flex items-center justify-center gap-2 text-sm text-muted-foreground">
            <Dot tone="destructive" />
            Disconnected
          </p>
          <p className="text-xs text-muted-foreground/60">The agent-terminal daemon is not running on :{port}</p>
        </div>
        <CliBlock line={start} />
        <Button variant="outline" icon={RotateCw} onClick={onRetry}>
          Retry
        </Button>
      </div>
    </div>
  );
};

export const NotOpen = ({ name }: { name: string }) => (
  <div className="flex h-full items-center justify-center p-6 text-center">
    <div className="space-y-2">
      <p className="text-sm text-muted-foreground">{name} is not open</p>
      <p className="text-xs">
        <a
          href="/"
          onClick={(event) => {
            event.preventDefault();
            navigate("/");
          }}
          className="text-muted-foreground underline underline-offset-4 hover:text-foreground"
        >
          Show open sessions
        </a>
      </p>
    </div>
  </div>
);

export const NotFound = () => (
  <div className="flex h-full items-center justify-center text-center">
    <div>
      <p className="mr-5 inline-block border-r border-foreground/30 pr-[23px] align-top text-2xl leading-[49px] font-medium">404</p>
      <div className="inline-block">
        <p className="text-sm leading-[49px] font-normal">This page could not be found.</p>
      </div>
    </div>
  </div>
);
