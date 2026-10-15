import type { Terminal } from "@xterm/xterm";

const fitRows = (list: HTMLElement, rows: number) => {
  while (list.childElementCount < rows) {
    const item = document.createElement("div");

    item.setAttribute("role", "listitem");
    list.appendChild(item);
  }

  while (list.childElementCount > rows) {
    list.lastElementChild?.remove();
  }
};

interface Mirror { terminal: Terminal; list: HTMLElement; shown: string[] }

const mirrorRow = (mirror: Mirror, y: number) => {
  const buffer = mirror.terminal.buffer.active;
  const text = buffer.getLine(buffer.viewportY + y)?.translateToString(true) ?? "";

  if (mirror.shown[y] === text) {
    return;
  }

  const item = mirror.list.children.item(y);

  if (item) {
    item.textContent = text;
  }

  mirror.shown[y] = text;
};

export const mirrorRows = (terminal: Terminal, host: HTMLElement) => {
  const list = document.createElement("div");

  list.setAttribute("role", "list");
  list.setAttribute("aria-label", "Screen");
  list.className = "sr-only";
  host.appendChild(list);
  const mirror: Mirror = { terminal, list, shown: [] };
  const update = () => {
    const rows = terminal.rows;

    fitRows(list, rows);
    for (let y = 0; y < rows; y += 1) {
      mirrorRow(mirror, y);
    }

    mirror.shown.length = rows;
  };

  const parsed = terminal.onWriteParsed(update);
  const resized = terminal.onResize(update);

  return () => {
    parsed.dispose();
    resized.dispose();
    list.remove();
  };
};
