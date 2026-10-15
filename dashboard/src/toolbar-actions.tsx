import type { ComponentType } from "react";

import { ControlButton } from "./components/control-button";
import { LayoutSwitch } from "./components/layout-switch";
import { RecordButton } from "./components/record-button";
import { RestartButton } from "./components/restart-button";
import { ScreenshotButton } from "./components/screenshot-button";
import type { TabInfo } from "./lib/protocol";
import type { Layout } from "./lib/split";
import type { Shown, StreamState } from "./lib/stream";

export type ToolbarTab = { session: string; tab: TabInfo; stream: StreamState; shown: Shown | null };

export type ToolbarActionProps = ToolbarTab & { layout: Layout; onLayout: (count: Layout) => void };

export type ToolbarAction = { id: string; Action: ComponentType<ToolbarActionProps> };

export const TOOLBAR_ACTIONS: ToolbarAction[] = [
  { id: "restart", Action: RestartButton },
  { id: "screenshot", Action: ScreenshotButton },
  { id: "record", Action: RecordButton },
  { id: "layout", Action: LayoutSwitch },
  { id: "control", Action: ControlButton },
];
