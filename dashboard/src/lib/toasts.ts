export type ToastType = "success" | "error" | "warning" | "info";

export interface Toast { id: number; type: ToastType; title: string; description: string }

const SHOWN_AT_ONCE = 3;

let toasts: Toast[] = [];
let nextId = 0;
const listeners = new Set<() => void>();

const publish = (next: Toast[]) => {
  toasts = next;
  listeners.forEach((listener) => {
    listener();
  });
};

export const dismiss = (id: number) => {
  publish(toasts.filter((toast) => toast.id !== id));
};

interface Message { title: string; description?: string }

export const toast = (type: ToastType, message: Message) => {
  const id = nextId++;

  publish([{ id, type, title: message.title, description: message.description ?? "" }, ...toasts].slice(0, SHOWN_AT_ONCE));
  window.setTimeout(() => {
    dismiss(id);
  }, type === "error" || type === "warning" ? 9000 : 5000);
};

export const subscribeToToasts = (listener: () => void) => {
  listeners.add(listener);

  return () => listeners.delete(listener);
};

export const shownToasts = () => toasts;
