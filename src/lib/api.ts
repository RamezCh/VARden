// Typed wrappers around the Rust commands in src-tauri/src/commands.rs.
// Shapes mirror the serde types in crates/envcore (camelCase).
import { invoke } from "@tauri-apps/api/core";

export type Scope = "user" | "process";
export type Status = "activeSynced" | "persistentOnly" | "processOnly" | "missingStale";
export type ValueKind = "string" | "expandString";
export type Syntax = "posix" | "fish" | "nushell" | "powerShell";

export interface EnvVar {
  key: string;
  value: string;
  liveValue: string | null;
  scope: Scope;
  status: Status;
  kind: ValueKind;
  sensitive: boolean;
  /** Sync to terminals would change what open terminals have for this row. */
  pendingSync: boolean;
  /** The value as programs see it, when different (Windows %VAR%). */
  expanded: string | null;
  /** How many paths in the value were checked (0: not a path). */
  checkedPaths: number;
  /** Paths that don't exist; non-empty makes the status Missing / stale. */
  missingPaths: string[];
}

export type StoreInfo =
  | { type: "registry"; location: string }
  | { type: "shellFile"; path: string; syntax: Syntax };

export interface PlatformInfo {
  os: string;
  stores: StoreInfo[];
  backupDir: string;
  configurableTargets: boolean;
}

export interface DetectedShell {
  kind: ShellKind;
  name: string;
  executable: string | null;
  configPath: string | null;
  configExists: boolean;
  syntax: Syntax | null;
  inheritsUserEnv: boolean;
  isDefault: boolean;
}

export interface ShellTarget {
  path: string;
  syntax?: Syntax | null;
}

export type BackupTarget =
  | { type: "registry" }
  | { type: "shellFile"; path: string; existed: boolean };

export interface BackupEntry {
  id: string;
  createdAt: string;
  target: BackupTarget;
  reason: string;
  sizeBytes: number;
  /** The target's state before VARden first changed it. Never pruned. */
  original: boolean;
}

export interface BackupOutcome extends BackupEntry {
  /** No new backup was written: an existing one already holds this exact state. */
  reused: boolean;
}

export interface MutationResult {
  variables: EnvVar[];
  backups: BackupOutcome[];
}

export const api = {
  platformInfo: () => invoke<PlatformInfo>("get_platform_info"),
  listVariables: (tracked?: string[]) => invoke<EnvVar[]>("list_variables", { tracked }),
  setVariable: (key: string, value: string, scope: Scope) =>
    invoke<MutationResult>("set_variable", { key, value, scope }),
  deleteVariable: (key: string, scope: Scope) =>
    invoke<MutationResult>("delete_variable", { key, scope }),
  detectShells: () => invoke<DetectedShell[]>("detect_shells"),
  getShellTargets: () => invoke<ShellTarget[]>("get_shell_targets"),
  setShellTargets: (targets: ShellTarget[]) => invoke<ShellTarget[]>("set_shell_targets", { targets }),
  listBackups: () => invoke<BackupEntry[]>("list_backups"),
  restoreBackup: (id: string) => invoke<MutationResult>("restore_backup", { id }),
  openLogsFolder: () => invoke<string>("open_logs_folder"),
  deleteBackups: (ids: string[]) =>
    invoke<{ report: { deleted: number; keptOriginals: number }; backups: BackupEntry[] }>("delete_backups", { ids }),
};

// ----- Phase 3: rollback preview, terminal sync, .env bridge -----

export interface VarChange {
  key: string;
  /** null: doesn't exist now; restoring adds it. */
  current: string | null;
  /** null: restoring deletes it. */
  restored: string | null;
  sensitive: boolean;
}

export interface RestorePreview {
  changes: VarChange[];
  otherChanges: boolean;
  deletesFile: boolean;
}

export type ShellKind =
  | "bash"
  | "zsh"
  | "fish"
  | "nushell"
  | "powerShell"
  | "windowsPowerShell"
  | "cmd"
  | "gitBash";

export interface HookStatus {
  kind: ShellKind;
  name: string;
  profile: string | null;
  unsupported: string | null;
  installed: boolean;
  problem: string | null;
}

export interface TerminalSyncInfo {
  generation: number;
  lastSyncedAt: string | null;
  hooks: HookStatus[];
}

export interface SyncReport {
  generation: number;
  syncedAt: string;
  pushed: number;
  removed: number;
  skipped: string[];
  hooksInstalled: number;
}

export interface EnvEntry {
  key: string;
  value: string;
}

export interface SkippedLine {
  line: number;
  text: string;
  reason: string;
}

export interface ParsedEnv {
  entries: EnvEntry[];
  skipped: SkippedLine[];
}

export interface ExportItem {
  key: string;
  scope: Scope;
}

export interface ExportReport {
  path: string;
  written: number;
  /** [key, reason] */
  skipped: [string, string][];
}

export const syncApi = {
  previewRestore: (id: string) => invoke<RestorePreview>("preview_restore", { id }),
  terminalSync: () => invoke<TerminalSyncInfo>("get_terminal_sync"),
  installHook: (kind: ShellKind) =>
    invoke<{ sync: TerminalSyncInfo; backups: BackupOutcome[] }>("install_terminal_hook", { kind }),
  removeHook: (kind: ShellKind) =>
    invoke<{ sync: TerminalSyncInfo; backups: BackupOutcome[] }>("remove_terminal_hook", { kind }),
  syncTerminals: () => invoke<{ report: SyncReport; variables: EnvVar[] }>("sync_terminals"),
  readEnvFile: (path: string) => invoke<ParsedEnv>("read_env_file", { path }),
  importVariables: (entries: EnvEntry[], scope: Scope) =>
    invoke<MutationResult>("import_variables", { entries, scope }),
  exportEnvFile: (path: string, items: ExportItem[]) => invoke<ExportReport>("export_env_file", { path, items }),
};
