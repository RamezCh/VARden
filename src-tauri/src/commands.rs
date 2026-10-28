//! Tauri commands — thin wrappers over `envcore::manager::EnvManager`.
//!
//! All commands are `async` so Tauri runs them off the main thread: registry
//! and file I/O must never freeze the window.

use std::path::PathBuf;

use envcore::backup::{BackupEntry, BackupOutcome};
use envcore::dotenv::{self, EnvEntry, ParsedEnv};
use envcore::manager::{EnvManager, PlatformInfo, RestorePreview, ShellTarget};
use envcore::model::{EnvVarView, Scope};
use envcore::shells::{self, DetectedShell, ShellKind};
use envcore::terminal::{SyncReport, TerminalSyncInfo};
use envcore::transfer::{ExportItem, ExportReport};
use envcore::Error;
use serde::Serialize;
use tauri::State;

type CmdResult<T> = Result<T, Error>;

/// Result of a mutation: the refreshed table plus any backups taken, so the
/// UI can offer an immediate "Undo".
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MutationResult {
    pub variables: Vec<EnvVarView>,
    pub backups: Vec<BackupOutcome>,
}

#[tauri::command]
pub async fn get_platform_info(mgr: State<'_, EnvManager>) -> CmdResult<PlatformInfo> {
    Ok(mgr.platform())
}

#[tauri::command]
pub async fn list_variables(
    mgr: State<'_, EnvManager>,
    tracked: Option<Vec<String>>,
) -> CmdResult<Vec<EnvVarView>> {
    mgr.list(&tracked.unwrap_or_default())
}

#[tauri::command]
pub async fn set_variable(
    mgr: State<'_, EnvManager>,
    key: String,
    value: String,
    scope: Scope,
) -> CmdResult<MutationResult> {
    let backups = mgr.set(key.trim(), &value, scope)?;
    Ok(MutationResult {
        variables: mgr.list(&[])?,
        backups,
    })
}

#[tauri::command]
pub async fn delete_variable(
    mgr: State<'_, EnvManager>,
    key: String,
    scope: Scope,
) -> CmdResult<MutationResult> {
    let backups = mgr.remove(&key, scope)?;
    Ok(MutationResult {
        variables: mgr.list(&[])?,
        backups,
    })
}

#[tauri::command]
pub async fn detect_shells() -> CmdResult<Vec<DetectedShell>> {
    Ok(shells::detect())
}

#[tauri::command]
pub async fn get_shell_targets(mgr: State<'_, EnvManager>) -> CmdResult<Vec<ShellTarget>> {
    Ok(mgr.shell_targets())
}

#[tauri::command]
pub async fn set_shell_targets(
    mgr: State<'_, EnvManager>,
    targets: Vec<ShellTarget>,
) -> CmdResult<Vec<ShellTarget>> {
    mgr.set_shell_targets(targets)?;
    Ok(mgr.shell_targets())
}

#[tauri::command]
pub async fn list_backups(mgr: State<'_, EnvManager>) -> CmdResult<Vec<BackupEntry>> {
    mgr.backups()
}

#[tauri::command]
pub async fn restore_backup(mgr: State<'_, EnvManager>, id: String) -> CmdResult<MutationResult> {
    let safety = mgr.restore(&id)?;
    Ok(MutationResult {
        variables: mgr.list(&[])?,
        backups: vec![safety],
    })
}

#[tauri::command]
pub async fn preview_restore(mgr: State<'_, EnvManager>, id: String) -> CmdResult<RestorePreview> {
    mgr.preview_restore(&id)
}

// ----- terminal sync -----

#[tauri::command]
pub async fn get_terminal_sync(mgr: State<'_, EnvManager>) -> CmdResult<TerminalSyncInfo> {
    Ok(mgr.terminal_sync_info())
}

/// Install/remove return the refreshed hook list plus the profile backup.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HookResult {
    pub sync: TerminalSyncInfo,
    pub backups: Vec<BackupOutcome>,
}

#[tauri::command]
pub async fn install_terminal_hook(
    mgr: State<'_, EnvManager>,
    kind: ShellKind,
) -> CmdResult<HookResult> {
    let backup = mgr.install_hook(kind)?;
    Ok(HookResult {
        sync: mgr.terminal_sync_info(),
        backups: backup.into_iter().collect(),
    })
}

#[tauri::command]
pub async fn remove_terminal_hook(
    mgr: State<'_, EnvManager>,
    kind: ShellKind,
) -> CmdResult<HookResult> {
    let backup = mgr.uninstall_hook(kind)?;
    Ok(HookResult {
        sync: mgr.terminal_sync_info(),
        backups: backup.into_iter().collect(),
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncResult {
    pub report: SyncReport,
    pub variables: Vec<EnvVarView>,
}

#[tauri::command]
pub async fn sync_terminals(mgr: State<'_, EnvManager>) -> CmdResult<SyncResult> {
    let report = mgr.sync_terminals()?;
    Ok(SyncResult {
        report,
        variables: mgr.list(&[])?,
    })
}

// ----- .env import / export -----

#[tauri::command]
pub async fn read_env_file(path: PathBuf) -> CmdResult<ParsedEnv> {
    dotenv::read_file(&path)
}

#[tauri::command]
pub async fn import_variables(
    mgr: State<'_, EnvManager>,
    entries: Vec<EnvEntry>,
    scope: Scope,
) -> CmdResult<MutationResult> {
    let backups = mgr.import_env(&entries, scope)?;
    Ok(MutationResult {
        variables: mgr.list(&[])?,
        backups,
    })
}

#[tauri::command]
pub async fn export_env_file(
    mgr: State<'_, EnvManager>,
    path: PathBuf,
    items: Vec<ExportItem>,
) -> CmdResult<ExportReport> {
    mgr.export_env(&path, &items)
}
