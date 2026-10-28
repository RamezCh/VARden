import type { Toast } from "../lib/useToasts";
import { XIcon } from "./icons";
import { IconButton } from "./IconButton";

const BAR: Record<Toast["kind"], string> = {
  success: "bg-synced",
  info: "bg-process",
  error: "bg-missing",
};

export function Toasts({ toasts, onDismiss }: { toasts: Toast[]; onDismiss: (id: number) => void }) {
  return (
    <div aria-live="polite" className="pointer-events-none fixed bottom-4 right-4 z-50 flex w-[min(420px,calc(100vw-2rem))] flex-col gap-2">
      {toasts.map((t) => (
        <div
          key={t.id}
          role={t.kind === "error" ? "alert" : "status"}
          className="toast-in pointer-events-auto flex items-start gap-3 overflow-hidden rounded-lg border border-rule bg-raised py-2.5 pl-0 pr-2 text-sm shadow-lg shadow-black/30"
        >
          <span className={`w-1 self-stretch ${BAR[t.kind]}`} />
          <p className="flex-1 py-0.5">{t.text}</p>
          {t.action && (
            <button
              type="button"
              onClick={() => {
                onDismiss(t.id);
                t.action?.run();
              }}
              className="shrink-0 rounded-md px-2 py-0.5 text-sm font-semibold text-process hover:bg-process/10"
            >
              {t.action.label}
            </button>
          )}
          <IconButton label="Dismiss" onClick={() => onDismiss(t.id)}>
            <XIcon />
          </IconButton>
        </div>
      ))}
    </div>
  );
}
