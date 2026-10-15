import type { ComponentType } from "react";

import { ActivityPanel } from "./components/activity-panel";
import { CommandsPanel } from "./components/commands-panel";
import { InfoPanel } from "./components/info-panel";
import { OutputPanel } from "./components/output-panel";
import { ScreenshotsPanel } from "./components/screenshots-panel";
import type { TabInfo } from "./lib/protocol";

export interface PanelProps { session: string; tab: TabInfo }

export interface Panel { id: string; label: string; Body: ComponentType<PanelProps> }

export const PANELS: Panel[] = [
  { id: "activity", label: "Activity", Body: ActivityPanel },
  { id: "commands", label: "Commands", Body: CommandsPanel },
  { id: "screenshots", label: "Screenshots", Body: ScreenshotsPanel },
  { id: "output", label: "Output", Body: OutputPanel },
  { id: "info", label: "Info", Body: InfoPanel },
];
