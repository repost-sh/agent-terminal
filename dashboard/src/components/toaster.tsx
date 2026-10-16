import { CircleAlert, CircleCheck, Info, TriangleAlert, X } from "lucide-react";
import { useStore } from "../lib/store";
import { dismiss, toasts } from "../lib/toasts";

const ICONS = { success: CircleCheck, error: CircleAlert, warning: TriangleAlert, info: Info };

export const Toaster = () => {
  const shown = useStore(toasts);

  return (
    <section className="toaster" aria-label="Notifications" aria-live="polite" aria-relevant="additions">
      {shown.map((entry) => {
        const Icon = ICONS[entry.type];

        return (
          <div key={entry.id} className="toast" data-type={entry.type} role={entry.type === "error" ? "alert" : "status"}>
            <Icon aria-hidden="true" className="mt-0.5 size-3.5 shrink-0" />
            <div className="min-w-0 flex-1">
              <div className="font-medium">{entry.title}</div>
              {entry.description && <div className="mt-0.5 font-mono text-[11px] opacity-80">{entry.description}</div>}
            </div>
            <button
              type="button"
              aria-label="Close notification"
              onClick={() => {
                dismiss(entry.id);
              }}
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
