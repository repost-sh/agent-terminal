import type { ComponentType } from "react";

import { ActivityPanel } from "./components/activity-panel";
import { CommandsPanel } from "./components/commands-panel";
import { InfoPanel } from "./components/info-panel";
import { OutputPanel } from "./components/output-panel";
import type { TabInfo } from "./lib/protocol";

export type PanelProps = { session: string; tab: TabInfo };

export type Panel = { id: string; label: string; Body: ComponentType<PanelProps> };

export const PANELS: Panel[] = [
  { id: "activity", label: "Activity", Body: ActivityPanel },
  { id: "commands", label: "Commands", Body: CommandsPanel },
  { id: "output", label: "Output", Body: OutputPanel },
  { id: "info", label: "Info", Body: InfoPanel },
];
