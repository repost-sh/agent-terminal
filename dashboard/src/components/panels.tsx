import type { ComponentType } from "react";

import { ActivityPanel } from "./activity-panel";
import { CommandsPanel } from "./commands-panel";
import { InfoPanel } from "./info-panel";
import { OutputPanel } from "./output-panel";
import { ScreenshotsPanel } from "./screenshots-panel";
import type { TabInfo } from "../lib/protocol";
import type { PanelId } from "../lib/side-panel";

export interface PanelProps { session: string; tab: TabInfo }

export interface Panel { id: PanelId; label: string; Body: ComponentType<PanelProps> }

export const PANELS: Panel[] = [
  { id: "activity", label: "Activity", Body: ActivityPanel },
  { id: "commands", label: "Commands", Body: CommandsPanel },
  { id: "screenshots", label: "Screenshots", Body: ScreenshotsPanel },
  { id: "output", label: "Output", Body: OutputPanel },
  { id: "info", label: "Info", Body: InfoPanel },
];
