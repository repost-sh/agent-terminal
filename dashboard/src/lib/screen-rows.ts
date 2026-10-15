import type { Terminal } from "@xterm/xterm";

export type ScreenRows = { update: () => void; dispose: () => void };

export const mirrorRows = (terminal: Terminal, host: HTMLElement): ScreenRows => {
  const list = document.createElement("div");
  list.setAttribute("role", "list");
  list.setAttribute("aria-label", "Screen");
  list.className = "sr-only";
  host.appendChild(list);
  let shown: string[] = [];
  const update = () => {
    const buffer = terminal.buffer.active;
    const rows = terminal.rows;
    while (list.childElementCount < rows) {
      const item = document.createElement("div");
      item.setAttribute("role", "listitem");
      list.appendChild(item);
    }
    while (list.childElementCount > rows) list.lastElementChild?.remove();
    for (let y = 0; y < rows; y += 1) {
      const text = buffer.getLine(buffer.viewportY + y)?.translateToString(true) ?? "";
      if (shown[y] !== text) {
        const item = list.children.item(y);
        if (item) item.textContent = text;
        shown[y] = text;
      }
    }
    shown.length = rows;
  };
  return { update, dispose: () => list.remove() };
};
