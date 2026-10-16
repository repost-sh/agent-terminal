import type { Terminal } from "@xterm/xterm";

const rowItem = () => {
  const item = document.createElement("div");

  item.setAttribute("role", "listitem");

  return item;
};

export const mirrorRows = (terminal: Terminal, host: HTMLElement) => {
  const list = document.createElement("div");

  list.setAttribute("role", "list");
  list.setAttribute("aria-label", "Screen");
  list.className = "sr-only";
  host.appendChild(list);
  const update = () => {
    const buffer = terminal.buffer.active;

    list.append(...Array.from({ length: terminal.rows - list.childElementCount }, rowItem));
    Array.from(list.children)
      .slice(terminal.rows)
      .forEach((item) => {
        item.remove();
      });
    Array.from(list.children).forEach((item, y) => {
      const text = String(buffer.getLine(buffer.viewportY + y)?.translateToString(true));

      if (item.textContent !== text) {
        item.textContent = text;
      }
    });
  };

  const parsed = terminal.onWriteParsed(update);
  const resized = terminal.onResize(update);

  return () => {
    parsed.dispose();
    resized.dispose();
    list.remove();
  };
};
