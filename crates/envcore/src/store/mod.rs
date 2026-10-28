//! Persistent (user-level) variable stores.
//!
//! * Windows: [`windows_registry::RegistryStore`] — `HKCU\Environment`.
//! * macOS / Linux: [`shell_file::ShellFileStore`] — the managed block in
//!   one or more shell config files.

pub mod shell_file;
#[cfg(windows)]
pub mod windows_registry;

use serde::Serialize;

use crate::Result;
use crate::backup::{BackupOutcome, BackupStore};
use crate::model::StoredVar;

/// What a store is backed by, for display in the UI.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum StoreInfo {
    #[serde(rename_all = "camelCase")]
    Registry { location: String },
    #[serde(rename_all = "camelCase")]
    ShellFile {
        path: std::path::PathBuf,
        syntax: crate::managed_block::Syntax,
    },
}

pub trait PersistentStore: Send + Sync {
    fn info(&self) -> StoreInfo;

    /// Variables this store currently persists. Values that can only be
    /// expressed as shell expressions (e.g. `"$HOME/bin"`) are returned raw.
    fn list(&self) -> Result<Vec<StoredVar>>;

    /// Create or update `key`. Returns the backup taken (or reused, if an
    /// identical one already existed), or `None` if the store already held
    /// exactly this value and nothing was written.
    fn set(&self, key: &str, value: &str, backups: &BackupStore) -> Result<Option<BackupOutcome>>;

    /// Apply several changes with a single backup: `Some(value)` sets,
    /// `None` removes. Every entry is validated before anything is written.
    fn update_many(
        &self,
        changes: &[(String, Option<String>)],
        reason: &str,
        backups: &BackupStore,
    ) -> Result<Option<BackupOutcome>>;

    /// Create or update several variables with a single backup (.env import).
    fn set_many(
        &self,
        entries: &[(String, String)],
        reason: &str,
        backups: &BackupStore,
    ) -> Result<Option<BackupOutcome>> {
        let changes: Vec<(String, Option<String>)> = entries
            .iter()
            .map(|(k, v)| (k.clone(), Some(v.clone())))
            .collect();
        self.update_many(&changes, reason, backups)
    }

    /// Write `line` (a shell expression assigning `key`) verbatim, or remove
    /// `key` when `None`. Only startup files support this.
    fn set_expression(
        &self,
        _key: &str,
        _line: Option<String>,
        _reason: &str,
        _backups: &BackupStore,
    ) -> Result<Option<BackupOutcome>> {
        Err(crate::Error::Unsupported("writing shell expressions"))
    }

    /// Delete `key`. Returns the backup taken, or `None` if it wasn't there.
    fn remove(&self, key: &str, backups: &BackupStore) -> Result<Option<BackupOutcome>>;
}
