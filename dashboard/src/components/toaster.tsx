import { CircleAlert, CircleCheck, Info, TriangleAlert, X } from "lucide-react";
import { useSyncExternalStore } from "react";

type ToastType = "success" | "error" | "warning" | "info";

type Toast = { id: number; type: ToastType; title: string; description: string };

const SHOWN_AT_ONCE = 3;
const ICONS = { success: CircleCheck, error: CircleAlert, warning: TriangleAlert, info: Info };

let toasts: Toast[] = [];
let nextId = 0;
const listeners = new Set<() => void>();

const publish = (next: Toast[]) => {
  toasts = next;
  listeners.forEach((listener) => listener());
};

const dismiss = (id: number) => publish(toasts.filter((toast) => toast.id !== id));

export const toast = (type: ToastType, title: string, description = "") => {
  const id = nextId++;
  publish([{ id, type, title, description }, ...toasts].slice(0, SHOWN_AT_ONCE));
  window.setTimeout(() => dismiss(id), type === "error" || type === "warning" ? 9000 : 5000);
};

const subscribe = (listener: () => void) => {
  listeners.add(listener);
  return () => listeners.delete(listener);
};

export const Toaster = () => {
  const shown = useSyncExternalStore(subscribe, () => toasts);
  return (
    <section className="toaster" aria-label="Notifications" aria-live="polite" aria-relevant="additions">
      {shown.map(({ id, type, title, description }) => {
        const Icon = ICONS[type];
        return (
          <div key={id} className="toast" data-type={type} role={type === "error" ? "alert" : "status"}>
            <Icon aria-hidden="true" className="mt-0.5 size-3.5 shrink-0" />
            <div className="min-w-0 flex-1">
              <div className="font-medium">{title}</div>
              {description && <div className="mt-0.5 font-mono text-[11px] opacity-80">{description}</div>}
            </div>
            <button
              type="button"
              aria-label="Close notification"
              onClick={() => dismiss(id)}
              className="-mt-1 -mr-1 inline-flex size-6 items-center justify-center rounded opacity-60 hover:opacity-100 focus-visible:outline-2 focus-visible:outline-current"
            >
              <X aria-hidden="true" className="size-[13px]" />
            </button>
          </div>
        );
      })}
    </section>
  );
};
