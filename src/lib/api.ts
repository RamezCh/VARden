// Typed wrappers around the Rust commands in src-tauri/src/commands.rs.
// Shapes mirror the serde types in crates/envcore (camelCase).
import { invoke } from "@tauri-apps/api/core";

export type Scope = "user" | "process";
export type Status = "activeSynced" | "persistentOnly" | "processOnly" | "missingStale";
export type ValueKind = "string" | "expandString" | "expression";
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

// ----- Stage 3: profiles, startup files -----

export interface Profile {
  /** Empty when creating. */
  id: string;
  name: string;
  /** An empty value means "expected": not written, shown as Missing until set. */
  vars: EnvEntry[];
}

export interface ActiveProfile {
  id: string;
  name: string;
  scope: Scope;
  appliedAt: string;
  /** Edited since it was applied. */
  outdated: boolean;
}

export interface ProfilesView {
  profiles: Profile[];
  active: ActiveProfile | null;
}

export interface ProfileResult {
  variables: EnvVar[];
  profiles: ProfilesView;
  backups: BackupOutcome[];
}

export const profileApi = {
  list: () => invoke<ProfilesView>("list_profiles"),
  save: (profile: Profile) => invoke<ProfilesView>("save_profile", { profile }),
  remove: (id: string) => invoke<ProfilesView>("delete_profile", { id }),
  apply: (id: string, scope: Scope) => invoke<ProfileResult>("apply_profile", { id, scope }),
  deactivate: () => invoke<ProfileResult>("deactivate_profile"),
};

// ----- Stage 4: editing tools -----

export interface PathEntry {
  /** As written: %USERPROFILE%\\bin, $HOME/bin. */
  value: string;
  /** The folder it points to, when different from value. */
  expanded: string | null;
  /** The folder could be checked (absolute, no unknown variables). */
  checked: boolean;
  missing: boolean;
  /** An earlier PATH entry already points to the same folder. */
  duplicate: boolean;
}

export interface PathInfo {
  separator: string;
  /** Your entries: the only part that can change. */
  entries: PathEntry[];
  /** Where the system PATH sits among your entries (0: before them). */
  baseAt: number;
  /** Can your entries go before the system PATH? (Not on Windows.) */
  baseMovable: boolean;
  /** The read-only system part. */
  base: PathEntry[];
  note: string | null;
}

export const editApi = {
  deleteVariables: (items: ExportItem[]) => invoke<MutationResult>("delete_variables", { items }),
  renameVariable: (from: string, to: string, scope: Scope) =>
    invoke<MutationResult>("rename_variable", { from, to, scope }),
  pathInfo: () => invoke<PathInfo>("get_path_info"),
  checkPathEntries: (entries: string[], baseAt: number) =>
    invoke<PathInfo>("check_path_entries", { entries, baseAt }),
  setUserPath: (entries: string[], baseAt: number) => invoke<MutationResult>("set_user_path", { entries, baseAt }),
  recentExportFolders: () => invoke<string[]>("recent_export_folders"),
};

// ----- Stage 5: desktop settings -----

export interface AppPrefs {
  /** Closing the window keeps VARden in the tray. */
  closeToTray: boolean;
  /** Look for a new version when VARden starts. */
  checkForUpdates: boolean;
  /** The first close asked what closing should do. */
  closeAsked: boolean;
}

export interface AutostartInfo {
  enabled: boolean;
  /** "Start with Windows", "Open at login", ... */
  label: string;
  /** False on desktops without a tray. */
  tray: boolean;
}

export const settingsApi = {
  prefs: () => invoke<AppPrefs>("get_app_prefs"),
  setPrefs: (prefs: AppPrefs) => invoke<AppPrefs>("set_app_prefs", { prefs }),
  autostart: () => invoke<AutostartInfo>("get_autostart"),
  setAutostart: (enabled: boolean) => invoke<AutostartInfo>("set_autostart", { enabled }),
};

export interface Report {
  text: string;
  namesHidden: number;
  userHidden: number;
  secretsHidden: number;
  lines: number;
}

export const reportApi = {
  prepare: (keepNames: boolean) => invoke<Report>("prepare_report", { keepNames }),
  save: (path: string, text: string) => invoke<void>("save_report", { path, text }),
};
