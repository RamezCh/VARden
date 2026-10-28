import { useCallback, useEffect, useRef, useState } from "react";

export type ToastKind = "success" | "info" | "error";
export interface ToastAction {
  label: string;
  run: () => void;
}
export interface Toast {
  id: number;
  kind: ToastKind;
  text: string;
  action?: ToastAction;
}

/** Success/info toasts dismiss themselves; errors stay until dismissed. */
export function useToasts() {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const nextId = useRef(1);
  const timers = useRef(new Map<number, number>());

  const dismiss = useCallback((id: number) => {
    setToasts((ts) => ts.filter((t) => t.id !== id));
    const timer = timers.current.get(id);
    if (timer) window.clearTimeout(timer);
    timers.current.delete(id);
  }, []);

  const push = useCallback(
    (kind: ToastKind, text: string, action?: ToastAction) => {
      const id = nextId.current++;
      setToasts((ts) => [...ts.slice(-3), { id, kind, text, action }]);
      if (kind !== "error") {
        // Toasts with an action (Undo) stay long enough to use it.
        const ms = action ? 10000 : kind === "info" ? 9000 : 4500;
        timers.current.set(id, window.setTimeout(() => dismiss(id), ms));
      }
    },
    [dismiss],
  );

  useEffect(() => {
    const all = timers.current;
    return () => all.forEach((t) => window.clearTimeout(t));
  }, []);

  return { toasts, push, dismiss };
}
