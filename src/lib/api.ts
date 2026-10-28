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
  kind: string;
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
  /** The target's state before Environment Manager first changed it. Never pruned. */
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
};
