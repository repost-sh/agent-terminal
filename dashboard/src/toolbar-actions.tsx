import type { ComponentType } from "react";

import { RecordButton } from "./components/record-button";
import { RestartButton } from "./components/restart-button";
import type { TabInfo } from "./lib/protocol";
import type { Shown, StreamState } from "./lib/stream";

export type ToolbarActionProps = { session: string; tab: TabInfo; stream: StreamState; shown: Shown | null; onRestart: () => Promise<boolean> };

export type ToolbarAction = { id: string; Action: ComponentType<ToolbarActionProps> };

export const TOOLBAR_ACTIONS: ToolbarAction[] = [
  { id: "restart", Action: RestartButton },
  { id: "record", Action: RecordButton },
];
