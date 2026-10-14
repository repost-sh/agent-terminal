import type { ComponentType } from "react";

import { CommandsPanel } from "./components/commands-panel";
import type { TabInfo } from "./lib/protocol";

export type PanelProps = { session: string; tab: TabInfo };

export type Panel = { id: string; label: string; Body: ComponentType<PanelProps> };

export const PANELS: Panel[] = [{ id: "commands", label: "Commands", Body: CommandsPanel }];
