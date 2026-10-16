import { createStore } from "./store";

export type ToastType = "success" | "error" | "warning" | "info";

export interface Toast { id: string; type: ToastType; title: string; description: string }

const SHOWN_AT_ONCE = 3;

export const toasts = createStore<Toast[]>([]);

export const dismiss = (id: string) => {
  toasts.set(toasts.get().filter((toast) => toast.id !== id));
};

interface Message { title: string; description?: string }

export const toast = (type: ToastType, message: Message) => {
  const id = crypto.randomUUID();

  toasts.set([{ id, type, title: message.title, description: message.description ?? "" }, ...toasts.get()].slice(0, SHOWN_AT_ONCE));
  window.setTimeout(() => {
    dismiss(id);
  }, type === "error" || type === "warning" ? 9000 : 5000);
};
