//! Tauri commands — thin wrappers over `envcore::manager::EnvManager`.
//!
//! All commands are `async` so Tauri runs them off the main thread: registry
//! and file I/O must never freeze the window.

use envcore::backup::{BackupEntry, BackupOutcome};
use envcore::manager::{EnvManager, PlatformInfo, ShellTarget};
use envcore::model::{EnvVarView, Scope};
use envcore::shells::{self, DetectedShell};
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
