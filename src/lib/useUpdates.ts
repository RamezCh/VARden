// Update state shared by the startup check (a toast) and the Settings page.
import { useCallback, useEffect, useRef, useState } from "react";
import { settingsApi } from "./api";
import { checkForUpdate, describeUpdateError, installUpdate, restartApp, type UpdateState } from "./updates";
import type { ToastAction, ToastKind } from "./useToasts";

type Notify = (kind: ToastKind, text: string, action?: ToastAction) => void;

export function useUpdates(notify: Notify, openSettings: () => void) {
  const [state, setState] = useState<UpdateState>({ kind: "idle" });
  const busy = useRef(false);

  const check = useCallback(
    async (quiet = false) => {
      if (busy.current) return;
      busy.current = true;
      if (!quiet) setState({ kind: "checking" });
      try {
        const update = await checkForUpdate();
        if (update) {
          setState({ kind: "available", update });
          if (quiet) {
            notify("info", `VARden ${update.version} is available.`, { label: "See what's new", run: openSettings });
          }
        } else {
          setState({ kind: "current", checkedAt: new Date() });
        }
      } catch (e) {
        // A quiet startup check never interrupts with an error.
        if (quiet) console.warn("Update check failed", e);
        else setState({ kind: "error", message: describeUpdateError(e) });
      } finally {
        busy.current = false;
      }
    },
    [notify, openSettings],
  );

  const install = useCallback(async () => {
    if (state.kind !== "available") return;
    const update = state.update;
    setState({ kind: "downloading", update, done: 0, total: null });
    try {
      await installUpdate(update, (done, total) => setState({ kind: "downloading", update, done, total }));
      setState({ kind: "ready", update });
    } catch (e) {
      setState({ kind: "error", message: `The update couldn't be installed: ${String(e)}` });
    }
  }, [state]);

  // Check once at startup, when the user hasn't turned it off.
  useEffect(() => {
    if (!("__TAURI_INTERNALS__" in window)) return;
    settingsApi
      .prefs()
      .then((p) => {
        if (p.checkForUpdates) void check(true);
      })
      .catch(() => {});
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return { state, check: () => check(false), install, restart: restartApp };
}

export type Updates = ReturnType<typeof useUpdates>;
