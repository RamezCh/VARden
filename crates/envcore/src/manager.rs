//! The façade the Tauri commands talk to.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

use serde::{Deserialize, Serialize};

use crate::backup::{BackupEntry, BackupOutcome, BackupStore, BackupTarget};
use crate::managed_block::Syntax;
use crate::model::{EnvVarView, Scope, StoredVar};
use crate::shells;
use crate::status::{self, EnvSnapshot, StatusInput};
use crate::store::shell_file::ShellFileStore;
use crate::store::{PersistentStore, StoreInfo};
use crate::validate::{validate_key, validate_value};
use crate::{Error, Result};

/// A shell file the user chose as a persistence target (Unix).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellTarget {
    pub path: PathBuf,
    /// Guessed from the file name when omitted.
    pub syntax: Option<Syntax>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformInfo {
    pub os: &'static str,
    pub stores: Vec<StoreInfo>,
    pub backup_dir: PathBuf,
    /// Whether the user can pick which shell files to write (Unix only).
    pub configurable_targets: bool,
}

pub struct EnvManager {
    backups: BackupStore,
    live: EnvSnapshot,
    overlay: Mutex<BTreeMap<String, String>>,
    stores: Mutex<Vec<Arc<dyn PersistentStore>>>,
    /// Serialises every write so two commands never edit a file at once.
    write_lock: Mutex<()>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    // A panic mid-command must not brick the app: recover poisoned locks.
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl EnvManager {
    /// Production setup: registry on Windows, the login shell's rc file elsewhere.
    pub fn new(backup_dir: impl Into<PathBuf>) -> Self {
        Self::with_stores(
            BackupStore::new(backup_dir),
            EnvSnapshot::capture(),
            default_stores(),
        )
    }

    pub fn with_stores(
        backups: BackupStore,
        live: EnvSnapshot,
        stores: Vec<Arc<dyn PersistentStore>>,
    ) -> Self {
        EnvManager {
            backups,
            live,
            overlay: Mutex::new(BTreeMap::new()),
            stores: Mutex::new(stores),
            write_lock: Mutex::new(()),
        }
    }

    pub fn platform(&self) -> PlatformInfo {
        PlatformInfo {
            os: std::env::consts::OS,
            stores: lock(&self.stores).iter().map(|s| s.info()).collect(),
            backup_dir: self.backups.root().to_path_buf(),
            configurable_targets: !cfg!(windows),
        }
    }

    fn stores(&self) -> Vec<Arc<dyn PersistentStore>> {
        lock(&self.stores).clone()
    }

    /// Merged view of all persistent stores; the first store wins on conflicts.
    fn persistent(&self) -> Result<Vec<StoredVar>> {
        let ci = self.live.is_case_insensitive();
        let mut out: Vec<StoredVar> = Vec::new();
        for store in self.stores() {
            for var in store.list()? {
                let dup = out.iter().any(|v| {
                    if ci {
                        v.key.eq_ignore_ascii_case(&var.key)
                    } else {
                        v.key == var.key
                    }
                });
                if !dup {
                    out.push(var);
                }
            }
        }
        Ok(out)
    }

    pub fn list(&self, tracked: &[String]) -> Result<Vec<EnvVarView>> {
        let persistent = self.persistent()?;
        let overlay = lock(&self.overlay).clone();
        let input = StatusInput {
            persistent: &persistent,
            overlay: &overlay,
            live: &self.live,
            tracked,
            path_separator: if cfg!(windows) { ';' } else { ':' },
        };
        Ok(status::compute(&input, &resolver))
    }

    /// Create or update a variable. Returns the backups taken or reused.
    pub fn set(&self, key: &str, value: &str, scope: Scope) -> Result<Vec<BackupOutcome>> {
        validate_key(key)?;
        validate_value(key, value)?;
        let _guard = lock(&self.write_lock);
        match scope {
            Scope::Process => {
                lock(&self.overlay).insert(key.to_string(), value.to_string());
                Ok(Vec::new())
            }
            Scope::User => {
                let mut taken = Vec::new();
                for store in self.stores() {
                    taken.extend(store.set(key, value, &self.backups)?);
                }
                // Promoting a session variable to user scope: it is now owned
                // by the persistent store, so drop the session copy.
                self.remove_overlay(key);
                Ok(taken)
            }
        }
    }

    pub fn remove(&self, key: &str, scope: Scope) -> Result<Vec<BackupOutcome>> {
        let _guard = lock(&self.write_lock);
        match scope {
            Scope::Process => {
                self.remove_overlay(key);
                Ok(Vec::new())
            }
            Scope::User => {
                let mut taken = Vec::new();
                for store in self.stores() {
                    taken.extend(store.remove(key, &self.backups)?);
                }
                Ok(taken)
            }
        }
    }

    fn remove_overlay(&self, key: &str) {
        let ci = self.live.is_case_insensitive();
        lock(&self.overlay).retain(|k, _| {
            if ci {
                !k.eq_ignore_ascii_case(key)
            } else {
                k != key
            }
        });
    }

    /// Process-level variables, e.g. to pass to child processes.
    pub fn process_overlay(&self) -> BTreeMap<String, String> {
        lock(&self.overlay).clone()
    }

    // ----- shell targets (Unix) -----

    pub fn shell_targets(&self) -> Vec<ShellTarget> {
        lock(&self.stores)
            .iter()
            .filter_map(|s| match s.info() {
                StoreInfo::ShellFile { path, syntax } => Some(ShellTarget {
                    path,
                    syntax: Some(syntax),
                }),
                StoreInfo::Registry { .. } => None,
            })
            .collect()
    }

    pub fn set_shell_targets(&self, targets: Vec<ShellTarget>) -> Result<()> {
        if cfg!(windows) {
            return Err(Error::Unsupported(
                "choosing shell files as the user-level store on Windows",
            ));
        }
        let mut stores: Vec<Arc<dyn PersistentStore>> = Vec::new();
        for t in targets {
            let syntax = t.syntax.unwrap_or_else(|| shells::syntax_for_path(&t.path));
            if !stores.iter().any(
                |s| matches!(s.info(), StoreInfo::ShellFile { ref path, .. } if *path == t.path),
            ) {
                stores.push(Arc::new(ShellFileStore::new(t.path, syntax)));
            }
        }
        let _guard = lock(&self.write_lock);
        *lock(&self.stores) = stores;
        Ok(())
    }

    // ----- backups -----

    pub fn backups(&self) -> Result<Vec<BackupEntry>> {
        self.backups.list()
    }

    /// Roll back to a backup. Returns the safety backup of the state that
    /// was replaced, so the restore itself can be undone.
    pub fn restore(&self, id: &str) -> Result<BackupOutcome> {
        let _guard = lock(&self.write_lock);
        let entry = self.backups.get(id)?;
        match entry.target {
            BackupTarget::ShellFile { .. } => self.backups.restore_file(id),
            BackupTarget::Registry => restore_registry(&self.backups, &entry),
        }
    }
}

#[cfg(windows)]
fn resolver(v: &StoredVar) -> Option<String> {
    use crate::model::ValueKind;
    Some(match v.kind {
        ValueKind::String => v.value.clone(),
        ValueKind::ExpandString => crate::store::windows_registry::expand_env_strings(&v.value),
    })
}

#[cfg(not(windows))]
fn resolver(v: &StoredVar) -> Option<String> {
    status::literal_resolver(v)
}

#[cfg(windows)]
fn restore_registry(backups: &BackupStore, entry: &BackupEntry) -> Result<BackupOutcome> {
    let snapshot = backups.registry_snapshot(&entry.id)?;
    crate::store::windows_registry::RegistryStore::new().restore_snapshot(
        &snapshot,
        backups,
        &BackupStore::restore_reason(entry),
    )
}

#[cfg(not(windows))]
fn restore_registry(_: &BackupStore, _: &BackupEntry) -> Result<BackupOutcome> {
    Err(Error::Unsupported("restoring a Windows registry backup"))
}

#[cfg(windows)]
fn default_stores() -> Vec<Arc<dyn PersistentStore>> {
    vec![Arc::new(
        crate::store::windows_registry::RegistryStore::new(),
    )]
}

#[cfg(not(windows))]
fn default_stores() -> Vec<Arc<dyn PersistentStore>> {
    let detected = shells::detect();
    let pick = detected
        .iter()
        .find(|s| s.is_default)
        .or_else(|| detected.iter().find(|s| s.config_exists))
        .and_then(|s| Some((s.config_path.clone()?, s.syntax?)));
    let (path, syntax) = pick.unwrap_or_else(|| {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let rc = if cfg!(target_os = "macos") {
            ".zshrc"
        } else {
            ".bashrc"
        };
        (home.join(rc), Syntax::Posix)
    });
    vec![Arc::new(ShellFileStore::new(path, syntax))]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Status;

    fn setup() -> (tempfile::TempDir, EnvManager) {
        let dir = tempfile::tempdir().unwrap();
        let rc = dir.path().join(".bashrc");
        let m = EnvManager::with_stores(
            BackupStore::new(dir.path().join("backups")),
            EnvSnapshot::from_pairs([("HOME", "/h")], false),
            vec![Arc::new(ShellFileStore::new(rc, Syntax::Posix))],
        );
        (dir, m)
    }

    #[test]
    fn user_and_process_flow() {
        let (_dir, m) = setup();
        m.set("SESSION", "1", Scope::Process).unwrap();
        assert_eq!(m.set("API_TOKEN", "abc", Scope::User).unwrap().len(), 1);

        let rows = m.list(&[]).unwrap();
        let get = |k: &str| rows.iter().find(|r| r.key == k).unwrap();
        assert_eq!(get("SESSION").status, Status::ProcessOnly);
        assert_eq!(get("API_TOKEN").status, Status::PersistentOnly);
        assert!(get("API_TOKEN").sensitive);

        // Promote the session var to user scope.
        m.set("SESSION", "1", Scope::User).unwrap();
        let rows = m.list(&[]).unwrap();
        let s = rows.iter().find(|r| r.key == "SESSION").unwrap();
        assert_eq!((s.scope, s.status), (Scope::User, Status::PersistentOnly));
        assert!(m.process_overlay().is_empty());
    }

    #[test]
    fn restore_rolls_back_and_is_undoable() {
        let (_dir, m) = setup();
        m.set("A", "1", Scope::User).unwrap();
        let before_b = m.set("B", "2", Scope::User).unwrap().remove(0);
        m.restore(&before_b.id).unwrap();
        let keys: Vec<String> = m.list(&[]).unwrap().into_iter().map(|r| r.key).collect();
        assert_eq!(keys, vec!["A"]);
    }

    #[test]
    fn invalid_keys_rejected_for_both_scopes() {
        let (_dir, m) = setup();
        assert!(m.set("BAD-KEY", "x", Scope::Process).is_err());
        assert!(m.set("BAD-KEY", "x", Scope::User).is_err());
    }
}
