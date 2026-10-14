import type { SessionInfo, TabInfo } from "./protocol";
import type { Cell } from "./route";
import { findTab } from "./tabs";

export type Layout = 1 | 2 | 4;

export type ShownCell = { cell: NonNullable<Cell>; tab: TabInfo } | null;

const sameCell = (a: Cell, b: Cell) => a?.session === b?.session && a?.tab === b?.tab;

export const shownCells = (sessions: SessionInfo[], cells: Cell[]): ShownCell[] =>
  cells.map((cell) => {
    const tab = cell && findTab(sessions, cell.session, cell.tab);
    return cell && tab ? { cell, tab } : null;
  });

export const focusedAt = (shown: ShownCell[], focus: number) => (shown[focus] ? focus : Math.max(0, shown.findIndex(Boolean)));

export const fillCells = (sessions: SessionInfo[], from: NonNullable<Cell>, count: Layout): Cell[] => {
  const open = sessions.flatMap((info) => info.tabs.map((tab) => ({ session: info.session, tab: tab.tab })));
  const at = open.findIndex((cell) => sameCell(cell, from));
  const order = at < 0 ? [from, ...open] : [...open.slice(at), ...open.slice(0, at)];
  return Array.from({ length: count }, (_, i) => order[i] ?? null);
};

export const resizeCells = (cells: Cell[], count: Layout): Cell[] => Array.from({ length: count }, (_, i) => cells[i] ?? null);

export const withCell = (cells: Cell[], at: number, cell: Cell) => cells.map((entry, i) => (i === at ? cell : entry));

export const isShown = (cells: Cell[], cell: Cell) => cells.some((entry) => entry !== null && sameCell(entry, cell));
