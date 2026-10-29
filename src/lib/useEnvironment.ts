// Single source of truth for the dashboard. Every mutation goes through a
// Tauri command, and the table is replaced with the list the backend returns,
// so the UI never drifts from what is actually stored.
import { useCallback, useEffect, useRef, useState } from "react";
import {
  api,
  editApi,
  profileApi,
  syncApi,
  type BackupEntry,
  type BackupOutcome,
  type DetectedShell,
  type EnvEntry,
  type EnvVar,
  type ExportItem,
  type MutationResult,
  type PlatformInfo,
  type Profile,
  type ProfileResult,
  type ProfilesView,
  type ShellTarget,
  type Scope,
  type ShellKind,
  type TerminalSyncInfo,
} from "./api";
import { fileName } from "./files";
import type { ToastAction, ToastKind } from "./useToasts";

type Notify = (kind: ToastKind, text: string, action?: ToastAction) => void;

export function formatWhen(iso: string): string {
  return new Date(iso).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
}

/** "9:50 AM" for today, otherwise "Oct 27". */
export function formatShort(iso: string): string {
  const d = new Date(iso);
  return d.toDateString() === new Date().toDateString()
    ? d.toLocaleTimeString(undefined, { timeStyle: "short" })
    : d.toLocaleDateString(undefined, { month: "short", day: "numeric" });
}

export function plural(n: number, word: string): string {
  return `${n} ${word}${n === 1 ? "" : "s"}`;
}

/** Tell the user when no new backup was written because one already holds this state. */
function reusedNotice(backups: BackupOutcome[]): string | null {
  const reused = backups.find((b) => b.reused);
  if (!reused) return null;
  const which = reused.original ? "the original backup" : "the backup";
  return `No new backup needed. This exact state is already saved in ${which} from ${formatWhen(reused.createdAt)}.`;
}

export function useEnvironment(notify: Notify, openBackups: () => void) {
  const [platform, setPlatform] = useState<PlatformInfo | null>(null);
  const [vars, setVars] = useState<EnvVar[]>([]);
  const [shells, setShells] = useState<DetectedShell[]>([]);
  const [backups, setBackups] = useState<BackupEntry[]>([]);
  const [sync, setSync] = useState<TerminalSyncInfo | null>(null);
  const [profiles, setProfiles] = useState<ProfilesView>({ profiles: [], active: null });
  const [startupFiles, setStartupFiles] = useState<ShellTarget[]>([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);

  const fail = useCallback(
    (e: unknown) => {
      const text = String(e);
      // A damaged managed block can only be fixed by hand or from a backup.
      const action = /malformed|damaged/i.test(text) ? { label: "Open backups", run: openBackups } : undefined;
      notify("error", text, action);
    },
    [notify, openBackups],
  );

  /**
   * Re-read everything. `quiet` is for automatic refreshes (outside changes,
   * window focus): no loading state, no error toasts, and it reports whether
   * the variables actually changed.
   */
  const varsRef = useRef<EnvVar[]>([]);
  const refresh = useCallback(async ({ quiet = false } = {}): Promise<boolean> => {
    if (!quiet) setLoading(true);
    try {
      const [p, v, s, b, t, pr, sf] = await Promise.all([
        api.platformInfo(),
        api.listVariables(),
        api.detectShells(),
        api.listBackups(),
        syncApi.terminalSync(),
        profileApi.list(),
        api.getShellTargets(),
      ]);
      setProfiles(pr);
      setStartupFiles(sf);
      const changed = JSON.stringify(v) !== JSON.stringify(varsRef.current);
      setPlatform(p);
      setVars(v);
      setShells(s);
      setBackups(b);
      setSync(t);
      return changed;
    } catch (e) {
      if (quiet) console.warn("Background refresh failed", e);
      else notify("error", `Couldn't read your environment: ${String(e)}`);
      return false;
    } finally {
      if (!quiet) setLoading(false);
    }
  }, [notify]);
  varsRef.current = vars;

  // Changes made outside the app (setx, the Windows dialog, an editor on
  // .zshrc) are reported by the backend; the window regaining focus is a
  // cheap catch-all on top.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    let lastFocusRefresh = 0;
    (async () => {
      try {
        const { listen } = await import("@tauri-apps/api/event");
        const { getCurrentWindow } = await import("@tauri-apps/api/window");
        const offChange = await listen("environment-changed", async () => {
          if (await refresh({ quiet: true })) {
            notify("info", "Your variables were changed outside the app. The list is up to date.");
          }
        });
        // Sync started from the tray icon: refresh and confirm it.
        const offTray = await listen<{ pushed: number; hooksInstalled: number }>("tray-synced", async ({ payload }) => {
          await refresh({ quiet: true });
          notify(
            "success",
            payload.hooksInstalled > 0
              ? `Synced from the tray. Open terminals pick up ${plural(payload.pushed, "variable")} at their next prompt.`
              : "Synced from the tray, but no shell has terminal sync installed yet.",
          );
        });
        const offFocus = await getCurrentWindow().onFocusChanged(({ payload: focused }) => {
          if (focused && Date.now() - lastFocusRefresh > 3000) {
            lastFocusRefresh = Date.now();
            void refresh({ quiet: true });
          }
        });
        unlisten = () => {
          offChange();
          offTray();
          offFocus();
        };
        if (cancelled) unlisten();
      } catch {
        /* not running inside Tauri */
      }
    })();
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [refresh, notify]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  // `undo` restores backups, which is itself a mutation; a ref breaks the cycle.
  const undoRef = useRef<(taken: BackupOutcome[]) => void>(() => {});

  /** Runs a mutation; returns true on success so forms know whether to close. */
  const mutate = useCallback(
    async (call: () => Promise<MutationResult>, success: string, undoable = true): Promise<boolean> => {
      setBusy(true);
      try {
        const result = await call();
        setVars(result.variables);
        setBackups(await api.listBackups());
        const taken = result.backups;
        notify(
          "success",
          success,
          undoable && taken.length > 0 ? { label: "Undo", run: () => undoRef.current(taken) } : undefined,
        );
        const notice = reusedNotice(taken);
        if (notice) notify("info", notice);
        return true;
      } catch (e) {
        fail(e);
        return false;
      } finally {
        setBusy(false);
      }
    },
    [notify, fail],
  );

  undoRef.current = (taken) =>
    void mutate(
      async () => {
        let last: MutationResult | null = null;
        for (const b of taken) last = await api.restoreBackup(b.id);
        return last ?? { variables: vars, backups: [] };
      },
      "Undone. The previous state is restored.",
      false,
    );

  const setVariable = useCallback(
    (key: string, value: string, scope: Scope, existed: boolean) =>
      mutate(
        () => api.setVariable(key, value, scope),
        scope === "user"
          ? `${existed ? "Updated" : "Saved"} ${key} in your user environment.`
          : `Set ${key} as temporary. Sync to push it into open terminals.`,
      ),
    [mutate],
  );

  const deleteVariable = useCallback(
    (key: string, scope: Scope) =>
      mutate(
        () => api.deleteVariable(key, scope),
        scope === "user" ? `Deleted ${key} from your user environment.` : `Removed temporary ${key}. Open terminals drop it at the next sync.`,
      ),
    [mutate],
  );

  const deleteVariables = useCallback(
    (items: ExportItem[]) =>
      mutate(() => editApi.deleteVariables(items), `Deleted ${plural(items.length, "variable")}.`),
    [mutate],
  );

  const renameVariable = useCallback(
    (from: string, to: string, scope: Scope) =>
      mutate(() => editApi.renameVariable(from, to, scope), `Renamed ${from} to ${to.trim()}.`),
    [mutate],
  );

  const saveUserPath = useCallback(
    (entries: string[], baseAt: number) =>
      mutate(
        () => editApi.setUserPath(entries, baseAt),
        "Saved your PATH. New terminals and apps pick it up; sync to update open terminals.",
      ),
    [mutate],
  );

  const restoreBackup = useCallback(
    (backup: BackupEntry) =>
      mutate(() => api.restoreBackup(backup.id), `Restored the backup from ${formatWhen(backup.createdAt)}.`),
    [mutate],
  );

  const importVariables = useCallback(
    (entries: EnvEntry[], scope: Scope, source: string) =>
      mutate(
        () => syncApi.importVariables(entries, scope),
        `Imported ${plural(entries.length, "variable")} from ${source} ${
          scope === "user" ? "into your user environment" : "as temporary variables"
        }.`,
      ),
    [mutate],
  );

  const exportVariables = useCallback(
    async (path: string, items: ExportItem[]): Promise<boolean> => {
      setBusy(true);
      try {
        const report = await syncApi.exportEnvFile(path, items);
        notify("success", `Exported ${plural(report.written, "variable")} to ${fileName(report.path)}.`);
        if (report.skipped.length > 0) {
          notify("info", `Left out ${report.skipped.map(([k, why]) => `${k} (${why})`).join(", ")}.`);
        }
        return true;
      } catch (e) {
        fail(e);
        return false;
      } finally {
        setBusy(false);
      }
    },
    [notify, fail],
  );

  /** Profiles: saving and deleting don't touch variables; applying does. */
  const saveProfile = useCallback(
    async (profile: Profile): Promise<boolean> => {
      setBusy(true);
      try {
        setProfiles(await profileApi.save(profile));
        notify("success", `Saved profile ${profile.name.trim()}.`);
        return true;
      } catch (e) {
        fail(e);
        return false;
      } finally {
        setBusy(false);
      }
    },
    [notify, fail],
  );

  const deleteProfile = useCallback(
    async (profile: Profile): Promise<boolean> => {
      setBusy(true);
      try {
        setProfiles(await profileApi.remove(profile.id));
        notify("success", `Deleted profile ${profile.name}.`);
        return true;
      } catch (e) {
        fail(e);
        return false;
      } finally {
        setBusy(false);
      }
    },
    [notify, fail],
  );

  const runProfile = useCallback(
    (call: () => Promise<ProfileResult>, success: string) =>
      mutate(async () => {
        const r = await call();
        setProfiles(r.profiles);
        return { variables: r.variables, backups: r.backups };
      }, success),
    [mutate],
  );

  const applyProfile = useCallback(
    (profile: Profile, scope: Scope) =>
      runProfile(
        () => profileApi.apply(profile.id, scope),
        scope === "user"
          ? `Applied ${profile.name} to your user environment.`
          : `Applied ${profile.name} as temporary variables. Sync to push them into open terminals.`,
      ),
    [runProfile],
  );

  const deactivateProfile = useCallback(
    (name: string) => runProfile(() => profileApi.deactivate(), `Turned off ${name}. Previous values are back.`),
    [runProfile],
  );

  const saveStartupFiles = useCallback(
    async (targets: ShellTarget[]): Promise<boolean> => {
      setBusy(true);
      try {
        setStartupFiles(await api.setShellTargets(targets));
        setVars(await api.listVariables());
        setPlatform(await api.platformInfo());
        notify("success", "Saved where variables are written.");
        return true;
      } catch (e) {
        fail(e);
        return false;
      } finally {
        setBusy(false);
      }
    },
    [notify, fail],
  );

  /** Delete backups; originals are always kept by the backend. */
  const deleteBackups = useCallback(
    async (ids: string[]): Promise<boolean> => {
      setBusy(true);
      try {
        const { report, backups: left } = await api.deleteBackups(ids);
        setBackups(left);
        const kept = report.keptOriginals > 0 ? ` The original${report.keptOriginals === 1 ? " was" : "s were"} kept.` : "";
        notify("success", `Deleted ${plural(report.deleted, "backup")}.${kept}`);
        return true;
      } catch (e) {
        fail(e);
        return false;
      } finally {
        setBusy(false);
      }
    },
    [notify, fail],
  );

  const syncTerminals = useCallback(async () => {
    setBusy(true);
    try {
      const { report, variables } = await syncApi.syncTerminals();
      setVars(variables);
      setSync(await syncApi.terminalSync());
      notify(
        "success",
        report.hooksInstalled > 0
          ? `Synced. Open terminals pick up ${plural(report.pushed, "variable")} at their next prompt.`
          : "Synced, but no shell has terminal sync installed yet, so open terminals won't see it.",
      );
      if (report.skipped.length > 0) {
        notify(
          "info",
          `${report.skipped.join(", ")} can't be pushed into open terminals (a shell expression, or a name shells can't set), so only new terminals get ${
            report.skipped.length === 1 ? "it" : "them"
          }.`,
        );
      }
    } catch (e) {
      fail(e);
    } finally {
      setBusy(false);
    }
  }, [notify, fail]);

  const setHook = useCallback(
    async (kind: ShellKind, name: string, install: boolean) => {
      setBusy(true);
      try {
        const result = install ? await syncApi.installHook(kind) : await syncApi.removeHook(kind);
        setSync(result.sync);
        setBackups(await api.listBackups());
        notify(
          "success",
          install
            ? `Terminal sync is set up for ${name}. Terminals opened from now on will receive syncs.`
            : `Removed terminal sync from ${name}.`,
        );
        const notice = reusedNotice(result.backups);
        if (notice) notify("info", notice);
      } catch (e) {
        fail(e);
      } finally {
        setBusy(false);
      }
    },
    [notify, fail],
  );

  return {
    platform,
    vars,
    shells,
    backups,
    sync,
    loading,
    busy,
    refresh,
    setVariable,
    deleteVariable,
    deleteVariables,
    renameVariable,
    saveUserPath,
    restoreBackup,
    deleteBackups,
    profiles,
    saveProfile,
    deleteProfile,
    applyProfile,
    deactivateProfile,
    startupFiles,
    saveStartupFiles,
    importVariables,
    exportVariables,
    syncTerminals,
    setHook,
  };
}
