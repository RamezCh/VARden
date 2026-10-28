import { useCallback, useEffect, useRef, useState } from "react";

export type ToastKind = "success" | "info" | "error";
export interface Toast {
  id: number;
  kind: ToastKind;
  text: string;
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
    (kind: ToastKind, text: string) => {
      const id = nextId.current++;
      setToasts((ts) => [...ts.slice(-3), { id, kind, text }]);
      if (kind !== "error") {
        timers.current.set(id, window.setTimeout(() => dismiss(id), kind === "info" ? 9000 : 4500));
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
