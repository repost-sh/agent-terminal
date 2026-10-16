import type { SessionInfo, TabInfo } from "./protocol";
import type { Cell } from "./route";
import { findTab, type TabAddress } from "./tabs";

export type Layout = 1 | 2 | 4;

export type ShownCell = { cell: NonNullable<Cell>; tab: TabInfo } | null;

const sameCell = (a: Cell, b: Cell) => a?.session === b?.session && a?.tab === b?.tab;

export const shownCells = (sessions: SessionInfo[], cells: Cell[]): ShownCell[] =>
  cells.map((cell) => {
    const tab = cell && findTab(sessions, cell);

    return cell && tab ? { cell, tab } : null;
  });

export const focusedAt = (shown: ShownCell[], focus: number) => (shown[focus] ? focus : Math.max(0, shown.findIndex(Boolean)));

export interface Filling { from: TabAddress; count: Layout }

export const fillCells = (sessions: SessionInfo[], filling: Filling): Cell[] => {
  const open = sessions.flatMap((info) => info.tabs.map((tab) => ({ session: info.session, tab: tab.tab })));
  const at = open.findIndex((cell) => sameCell(cell, filling.from));
  const order = at < 0 ? [filling.from, ...open] : [...open.slice(at), ...open.slice(0, at)];

  return Array.from({ length: filling.count }, (_, i) => order[i] ?? null);
};

export const resizeCells = (cells: Cell[], count: Layout): Cell[] => Array.from({ length: count }, (_, i) => cells[i] ?? null);

export interface Placing { at: number; cell: Cell }

export const withCell = (cells: Cell[], placing: Placing) => cells.map((entry, i) => (i === placing.at ? placing.cell : entry));

export const isShown = (cells: Cell[], cell: Cell) => cells.some((entry) => entry !== null && sameCell(entry, cell));
