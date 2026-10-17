import type { ComponentType } from "react";

import { LayoutSwitch } from "./layout-switch";
import { MouseButton } from "./mouse-button";
import { RestartButton } from "./restart-button";
import { ScreenshotButton } from "./screenshot-button";
import type { TabInfo } from "../lib/protocol";
import type { Layout } from "../lib/split";
import type { Shown, StreamState } from "../lib/stream";

export interface ToolbarTab { session: string; tab: TabInfo; stream: StreamState; shown: Shown | null }

export type ToolbarActionProps = ToolbarTab & { layout: Layout; onLayout: (count: Layout) => void };

export const TOOLBAR_ACTIONS: ComponentType<ToolbarActionProps>[] = [RestartButton, ScreenshotButton, LayoutSwitch, MouseButton];
