// Single source of truth for the dashboard. Every mutation goes through a
// Tauri command, and the table is replaced with the list the backend returns,
// so the UI never drifts from what is actually stored.
import { useCallback, useEffect, useState } from "react";
import {
  api,
  type BackupEntry,
  type BackupOutcome,
  type DetectedShell,
  type EnvVar,
  type MutationResult,
  type PlatformInfo,
  type Scope,
} from "./api";
import type { ToastKind } from "./useToasts";

type Notify = (kind: ToastKind, text: string) => void;

export function formatWhen(iso: string): string {
  return new Date(iso).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
}

/** Tell the user when no new backup was written because one already holds this state. */
function reusedNotice(backups: BackupOutcome[]): string | null {
  const reused = backups.find((b) => b.reused);
  if (!reused) return null;
  const which = reused.original ? "the original backup" : "the backup";
  return `No new backup needed. This exact state is already saved in ${which} from ${formatWhen(reused.createdAt)}.`;
}

export function useEnvironment(notify: Notify) {
  const [platform, setPlatform] = useState<PlatformInfo | null>(null);
  const [vars, setVars] = useState<EnvVar[]>([]);
  const [shells, setShells] = useState<DetectedShell[]>([]);
  const [backups, setBackups] = useState<BackupEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const [p, v, s, b] = await Promise.all([
        api.platformInfo(),
        api.listVariables(),
        api.detectShells(),
        api.listBackups(),
      ]);
      setPlatform(p);
      setVars(v);
      setShells(s);
      setBackups(b);
    } catch (e) {
      notify("error", `Couldn't read your environment: ${String(e)}`);
    } finally {
      setLoading(false);
    }
  }, [notify]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  /** Runs a mutation; returns true on success so forms know whether to close. */
  const mutate = useCallback(
    async (call: () => Promise<MutationResult>, success: string): Promise<boolean> => {
      setBusy(true);
      try {
        const result = await call();
        setVars(result.variables);
        setBackups(await api.listBackups());
        notify("success", success);
        const notice = reusedNotice(result.backups);
        if (notice) notify("info", notice);
        return true;
      } catch (e) {
        notify("error", String(e));
        return false;
      } finally {
        setBusy(false);
      }
    },
    [notify],
  );

  const setVariable = useCallback(
    (key: string, value: string, scope: Scope, existed: boolean) =>
      mutate(
        () => api.setVariable(key, value, scope),
        scope === "user"
          ? `${existed ? "Updated" : "Saved"} ${key} in your user environment.`
          : `Set ${key} for this session.`,
      ),
    [mutate],
  );

  const deleteVariable = useCallback(
    (key: string, scope: Scope) =>
      mutate(
        () => api.deleteVariable(key, scope),
        scope === "user" ? `Deleted ${key} from your user environment.` : `Removed ${key} from this session.`,
      ),
    [mutate],
  );

  const restoreBackup = useCallback(
    (backup: BackupEntry) =>
      mutate(() => api.restoreBackup(backup.id), `Restored the backup from ${formatWhen(backup.createdAt)}.`),
    [mutate],
  );

  return { platform, vars, shells, backups, loading, busy, refresh, setVariable, deleteVariable, restoreBackup };
}
