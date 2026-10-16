import type { ComponentType } from "react";

import { ControlButton } from "./control-button";
import { LayoutSwitch } from "./layout-switch";
import { RestartButton } from "./restart-button";
import { ScreenshotButton } from "./screenshot-button";
import type { TabInfo } from "../lib/protocol";
import type { Layout } from "../lib/split";
import type { Shown, StreamState } from "../lib/stream";

export interface ToolbarTab { session: string; tab: TabInfo; stream: StreamState; shown: Shown | null }

export type ToolbarActionProps = ToolbarTab & { layout: Layout; onLayout: (count: Layout) => void };

export interface ToolbarAction { id: string; Action: ComponentType<ToolbarActionProps> }

export const TOOLBAR_ACTIONS: ToolbarAction[] = [
  { id: "restart", Action: RestartButton },
  { id: "screenshot", Action: ScreenshotButton },
  { id: "layout", Action: LayoutSwitch },
  { id: "control", Action: ControlButton },
];
