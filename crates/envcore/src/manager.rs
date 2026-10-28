//! The façade the Tauri commands talk to.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::backup::{BackupEntry, BackupOutcome, BackupStore, BackupTarget, DeleteReport};
use crate::managed_block::{self, Syntax};
use crate::model::{EnvVarView, Scope, StoredVar};
use crate::status::{self, EnvSnapshot, StatusInput};
use crate::store::shell_file::ShellFileStore;
use crate::store::{PersistentStore, StoreInfo};
use crate::validate::is_sensitive;
use crate::validate::{validate_key, validate_value};
use crate::{fsutil, paths, shells};
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

/// App settings saved in `settings.json`.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Settings {
    /// Startup files chosen on macOS/Linux (`None`: the login shell's).
    #[serde(default)]
    startup_files: Option<Vec<ShellTarget>>,
}

/// One variable that a restore would change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VarChange {
    pub key: String,
    /// `None`: the variable doesn't exist now (the restore adds it).
    pub current: Option<String>,
    /// `None`: the restore deletes the variable.
    pub restored: Option<String>,
    pub sensitive: bool,
}

/// What restoring a backup would do, shown before the user confirms.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestorePreview {
    pub changes: Vec<VarChange>,
    /// The file also differs outside the variables block (other edits).
    pub other_changes: bool,
    /// The file didn't exist when the backup was taken; restoring deletes it.
    pub deletes_file: bool,
}

pub struct EnvManager {
    pub(crate) backups: BackupStore,
    /// The environment this app considers "current": what it was launched
    /// with, updated by "Sync to terminals".
    pub(crate) live: Mutex<EnvSnapshot>,
    /// Windows variable names are case-insensitive.
    pub(crate) ci: bool,
    overlay: Mutex<BTreeMap<String, String>>,
    stores: Mutex<Vec<Arc<dyn PersistentStore>>>,
    pub(crate) sync_dir: PathBuf,
    pub(crate) hooks_dir: PathBuf,
    /// Saved profiles and which one is active.
    pub(crate) profiles_path: PathBuf,
    /// App settings (startup files chosen on macOS/Linux).
    pub(crate) settings_path: PathBuf,
    /// Serialises every write so two commands never edit a file at once.
    pub(crate) write_lock: Mutex<()>,
    /// When the app itself last finished writing (to tell its own changes
    /// apart from outside ones).
    last_write: Mutex<Option<Instant>>,
}

/// Held for the duration of a write; records the time when dropped.
pub(crate) struct WriteGuard<'a> {
    _lock: MutexGuard<'a, ()>,
    last_write: &'a Mutex<Option<Instant>>,
}

impl Drop for WriteGuard<'_> {
    fn drop(&mut self) {
        *lock(self.last_write) = Some(Instant::now());
    }
}

pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    // A panic mid-command must not brick the app: recover poisoned locks.
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl EnvManager {
    /// Production setup: registry on Windows, the login shell's rc file elsewhere.
    pub fn new(backup_dir: impl Into<PathBuf>) -> Self {
        let manager = Self::with_stores(
            BackupStore::new(backup_dir),
            EnvSnapshot::capture(),
            default_stores(),
        )
        .with_dirs(paths::sync_dir(), paths::hooks_dir());
        manager.restore_saved_startup_files();
        manager
    }

    fn load_settings(&self) -> Settings {
        std::fs::read(&self.settings_path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    /// Use the startup files the user chose earlier (macOS/Linux), if any.
    pub fn restore_saved_startup_files(&self) {
        if cfg!(windows) {
            return;
        }
        if let Some(files) = self.load_settings().startup_files.filter(|f| !f.is_empty()) {
            self.replace_targets(files);
        }
    }

    fn replace_targets(&self, targets: Vec<ShellTarget>) {
        let mut stores: Vec<Arc<dyn PersistentStore>> = Vec::new();
        for t in targets {
            let syntax = t.syntax.unwrap_or_else(|| shells::syntax_for_path(&t.path));
            if !stores.iter().any(
                |s| matches!(s.info(), StoreInfo::ShellFile { ref path, .. } if *path == t.path),
            ) {
                stores.push(Arc::new(ShellFileStore::new(t.path, syntax)));
            }
        }
        let _guard = self.begin_write();
        *lock(&self.stores) = stores;
    }

    /// Where sync files and hook scripts go (tests point these at temp dirs).
    pub fn with_dirs(
        mut self,
        sync_dir: impl Into<PathBuf>,
        hooks_dir: impl Into<PathBuf>,
    ) -> Self {
        self.sync_dir = sync_dir.into();
        self.hooks_dir = hooks_dir.into();
        self
    }

    pub fn with_stores(
        backups: BackupStore,
        live: EnvSnapshot,
        stores: Vec<Arc<dyn PersistentStore>>,
    ) -> Self {
        let base = backups
            .root()
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(paths::app_data_dir);
        EnvManager {
            backups,
            ci: live.is_case_insensitive(),
            live: Mutex::new(live),
            sync_dir: base.join("sync"),
            hooks_dir: base.join("shell"),
            profiles_path: base.join("profiles.json"),
            settings_path: base.join("settings.json"),
            overlay: Mutex::new(BTreeMap::new()),
            stores: Mutex::new(stores),
            write_lock: Mutex::new(()),
            last_write: Mutex::new(None),
        }
    }

    pub(crate) fn begin_write(&self) -> WriteGuard<'_> {
        WriteGuard {
            _lock: lock(&self.write_lock),
            last_write: &self.last_write,
        }
    }

    /// Did the app itself write within `window`? Watch events right after
    /// the app's own writes are echoes, not outside changes.
    pub fn wrote_within(&self, window: Duration) -> bool {
        lock(&self.last_write).is_some_and(|t| t.elapsed() < window)
    }

    /// Files whose outside changes should refresh the dashboard: the
    /// startup files variables are written to, plus every detected shell
    /// profile (their terminal-sync status is shown).
    pub fn watch_paths(&self) -> Vec<PathBuf> {
        let mut paths: Vec<PathBuf> = self
            .stores()
            .iter()
            .filter_map(|s| match s.info() {
                StoreInfo::ShellFile { path, .. } => Some(path),
                StoreInfo::Registry { .. } => None,
            })
            .collect();
        paths.extend(shells::detect().into_iter().filter_map(|s| s.config_path));
        paths.sort();
        paths.dedup();
        paths
    }

    pub fn platform(&self) -> PlatformInfo {
        PlatformInfo {
            os: std::env::consts::OS,
            stores: lock(&self.stores).iter().map(|s| s.info()).collect(),
            backup_dir: self.backups.root().to_path_buf(),
            configurable_targets: !cfg!(windows),
        }
    }

    pub(crate) fn stores(&self) -> Vec<Arc<dyn PersistentStore>> {
        lock(&self.stores).clone()
    }

    /// Merged view of all persistent stores; the first store wins on conflicts.
    pub(crate) fn persistent(&self) -> Result<Vec<StoredVar>> {
        let ci = self.ci;
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
        // Variables the active profile expects show up even when missing.
        let mut tracked = tracked.to_vec();
        tracked.extend(self.active_profile_keys());
        let tracked = &tracked[..];
        let persistent = self.persistent()?;
        let overlay = lock(&self.overlay).clone();
        let live = lock(&self.live);
        let input = StatusInput {
            persistent: &persistent,
            overlay: &overlay,
            live: &live,
            tracked,
            path_separator: if cfg!(windows) { ';' } else { ':' },
        };
        Ok(status::compute(&input, &resolver))
    }

    /// Create or update a variable. Returns the backups taken or reused.
    pub fn set(&self, key: &str, value: &str, scope: Scope) -> Result<Vec<BackupOutcome>> {
        validate_key(key)?;
        validate_value(key, value)?;
        let _guard = self.begin_write();
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

    /// Create or update several variables at once with a single backup per
    /// store (used by .env import). Nothing is written if any entry is invalid.
    pub fn set_many(
        &self,
        entries: &[(String, String)],
        scope: Scope,
    ) -> Result<Vec<BackupOutcome>> {
        for (key, value) in entries {
            validate_key(key)?;
            validate_value(key, value)?;
        }
        if entries.is_empty() {
            return Ok(Vec::new());
        }
        let reason = match entries.len() {
            1 => format!("Before importing {}", entries[0].0),
            n => format!("Before importing {n} variables"),
        };
        let changes: Vec<(String, Option<String>)> = entries
            .iter()
            .map(|(k, v)| (k.clone(), Some(v.clone())))
            .collect();
        self.apply_changes(&changes, scope, &reason)
    }

    /// Apply sets (`Some`) and removals (`None`) in one scope with a single
    /// backup per store. Setting a user variable drops a temporary copy of
    /// it, like promoting does.
    pub(crate) fn apply_changes(
        &self,
        changes: &[(String, Option<String>)],
        scope: Scope,
        reason: &str,
    ) -> Result<Vec<BackupOutcome>> {
        for (key, value) in changes {
            validate_key(key)?;
            if let Some(value) = value {
                validate_value(key, value)?;
            }
        }
        if changes.is_empty() {
            return Ok(Vec::new());
        }
        let _guard = self.begin_write();
        match scope {
            Scope::Process => {
                for (k, v) in changes {
                    self.remove_overlay(k);
                    if let Some(v) = v {
                        lock(&self.overlay).insert(k.clone(), v.clone());
                    }
                }
                Ok(Vec::new())
            }
            Scope::User => {
                let mut taken = Vec::new();
                for store in self.stores() {
                    taken.extend(store.update_many(changes, reason, &self.backups)?);
                }
                for (k, v) in changes {
                    if v.is_some() {
                        self.remove_overlay(k);
                    }
                }
                Ok(taken)
            }
        }
    }

    /// The current value of `key` in `scope`, if any.
    pub(crate) fn current_value(&self, key: &str, scope: Scope) -> Result<Option<String>> {
        let same = |k: &str| {
            if self.ci {
                k.eq_ignore_ascii_case(key)
            } else {
                k == key
            }
        };
        Ok(match scope {
            Scope::User => self
                .persistent()?
                .into_iter()
                .find(|v| same(&v.key))
                .map(|v| v.value),
            Scope::Process => lock(&self.overlay)
                .iter()
                .find(|(k, _)| same(k))
                .map(|(_, v)| v.clone()),
        })
    }

    pub fn remove(&self, key: &str, scope: Scope) -> Result<Vec<BackupOutcome>> {
        let _guard = self.begin_write();
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
        let ci = self.ci;
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
        if targets.is_empty() {
            return Err(Error::Refused(
                "Keep at least one startup file, or there is nowhere to save variables.".into(),
            ));
        }
        self.replace_targets(targets);
        // Remember the choice across restarts.
        let mut settings = self.load_settings();
        settings.startup_files = Some(self.shell_targets());
        fsutil::write_atomic(&self.settings_path, &serde_json::to_vec_pretty(&settings)?)
    }

    // ----- backups -----

    pub fn backups(&self) -> Result<Vec<BackupEntry>> {
        self.backups.list()
    }

    /// Delete backups by id; originals are always kept.
    pub fn delete_backups(&self, ids: &[String]) -> Result<DeleteReport> {
        let _guard = self.begin_write();
        self.backups.delete(ids)
    }

    /// What restoring backup `id` would change, without changing anything.
    pub fn preview_restore(&self, id: &str) -> Result<RestorePreview> {
        let entry = self.backups.get(id)?;
        match &entry.target {
            BackupTarget::Registry => {
                let current = current_registry()?;
                let restored = self.backups.registry_snapshot(id)?;
                let pairs =
                    |v: Vec<StoredVar>| v.into_iter().map(|s| (s.key, s.value)).collect::<Vec<_>>();
                Ok(RestorePreview {
                    changes: diff_vars(&pairs(current), &pairs(restored), self.ci),
                    other_changes: false,
                    deletes_file: false,
                })
            }
            BackupTarget::ShellFile { path, existed } => {
                let syntax = self
                    .stores()
                    .iter()
                    .find_map(|s| match s.info() {
                        StoreInfo::ShellFile { path: p, syntax } if p == *path => Some(syntax),
                        _ => None,
                    })
                    .unwrap_or_else(|| shells::syntax_for_path(path));
                let now = fsutil::read_text(path)?.content;
                let then = if *existed {
                    fsutil::TextFile::parse(&String::from_utf8_lossy(&self.backups.payload(id)?))
                        .content
                } else {
                    String::new()
                };
                let (now_vars, now_rest) = split_block(&now, syntax);
                let (then_vars, then_rest) = split_block(&then, syntax);
                Ok(RestorePreview {
                    changes: diff_vars(&now_vars, &then_vars, self.ci),
                    other_changes: now_rest != then_rest,
                    deletes_file: !existed,
                })
            }
        }
    }

    /// Roll back to a backup. Returns the safety backup of the state that
    /// was replaced, so the restore itself can be undone.
    pub fn restore(&self, id: &str) -> Result<BackupOutcome> {
        let _guard = self.begin_write();
        let entry = self.backups.get(id)?;
        match entry.target {
            BackupTarget::ShellFile { .. } => self.backups.restore_file(id),
            BackupTarget::Registry => restore_registry(&self.backups, &entry),
        }
    }
}

/// Variables in the managed block (literal and raw expressions), plus the
/// rest of the file with the block cut out. A malformed block counts as
/// "other content" so the preview still flags that the file changes.
fn split_block(content: &str, syntax: Syntax) -> (Vec<(String, String)>, String) {
    match managed_block::read_block(content, syntax) {
        Ok(block) => {
            let block = block.unwrap_or_default();
            let mut vars = block.entries();
            vars.extend(block.opaque_assignments());
            let rest = managed_block::replace_section(content, managed_block::VARIABLES, &[])
                .unwrap_or_else(|_| content.to_string());
            (vars, rest)
        }
        Err(_) => (Vec::new(), content.to_string()),
    }
}

fn diff_vars(
    current: &[(String, String)],
    restored: &[(String, String)],
    ci: bool,
) -> Vec<VarChange> {
    let norm = |k: &str| {
        if ci {
            k.to_ascii_uppercase()
        } else {
            k.to_string()
        }
    };
    let mut keys: BTreeMap<String, String> = BTreeMap::new();
    for (k, _) in current.iter().chain(restored) {
        keys.entry(norm(k)).or_insert_with(|| k.clone());
    }
    let find = |list: &[(String, String)], n: &str| {
        list.iter()
            .rev()
            .find(|(k, _)| norm(k) == n)
            .map(|(_, v)| v.clone())
    };
    keys.into_iter()
        .filter_map(|(n, key)| {
            let (cur, res) = (find(current, &n), find(restored, &n));
            (cur != res).then(|| VarChange {
                sensitive: is_sensitive(&key, cur.as_deref().or(res.as_deref()).unwrap_or("")),
                key,
                current: cur,
                restored: res,
            })
        })
        .collect()
}

#[cfg(windows)]
fn current_registry() -> Result<Vec<StoredVar>> {
    crate::store::windows_registry::RegistryStore::new().list()
}

#[cfg(not(windows))]
fn current_registry() -> Result<Vec<StoredVar>> {
    Err(Error::Unsupported("previewing a Windows registry backup"))
}

#[cfg(windows)]
pub(crate) fn resolver(v: &StoredVar) -> Option<String> {
    use crate::model::ValueKind;
    Some(match v.kind {
        ValueKind::String => v.value.clone(),
        ValueKind::ExpandString => crate::store::windows_registry::expand_env_strings(&v.value),
    })
}

#[cfg(not(windows))]
pub(crate) fn resolver(v: &StoredVar) -> Option<String> {
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

    #[test]
    fn batch_set_takes_one_backup() {
        let (_dir, m) = setup();
        let entries = vec![
            ("A".to_string(), "1".to_string()),
            ("B".to_string(), "2".to_string()),
        ];
        assert_eq!(m.set_many(&entries, Scope::User).unwrap().len(), 1);
        assert_eq!(m.backups().unwrap().len(), 1);
        assert_eq!(
            m.backups().unwrap()[0].reason,
            "Before importing 2 variables"
        );
        let bad = vec![
            ("OK".to_string(), "1".to_string()),
            ("NOT-OK".to_string(), "2".to_string()),
        ];
        assert!(m.set_many(&bad, Scope::User).is_err());
        assert!(
            m.list(&[]).unwrap().iter().all(|r| r.key != "OK"),
            "nothing written"
        );
    }

    #[test]
    fn restore_preview_lists_changes() {
        let (dir, m) = setup();
        std::fs::write(dir.path().join(".bashrc"), "alias ll='ls -l'\n").unwrap();
        let first = m.set("KEEP", "1", Scope::User).unwrap().remove(0);
        m.set("API_TOKEN", "secret", Scope::User).unwrap();
        m.set("KEEP", "2", Scope::User).unwrap();

        let preview = m.preview_restore(&first.id).unwrap();
        assert_eq!(
            preview.changes,
            vec![
                VarChange {
                    key: "API_TOKEN".into(),
                    current: Some("secret".into()),
                    restored: None,
                    sensitive: true
                },
                VarChange {
                    key: "KEEP".into(),
                    current: Some("2".into()),
                    restored: None,
                    sensitive: false
                },
            ]
        );
        assert!(!preview.other_changes && !preview.deletes_file);
    }

    #[cfg(unix)]
    #[test]
    fn chosen_startup_files_survive_a_restart() {
        let (dir, m) = setup();
        let zshrc = dir.path().join(".zshrc");
        let fish = dir.path().join("config.fish");
        m.set_shell_targets(vec![
            ShellTarget {
                path: zshrc.clone(),
                syntax: None,
            },
            ShellTarget {
                path: fish.clone(),
                syntax: None,
            },
        ])
        .unwrap();
        assert!(m.set_shell_targets(vec![]).is_err());

        // A fresh manager over the same folder picks the choice up again.
        let again = EnvManager::with_stores(
            BackupStore::new(dir.path().join("backups")),
            EnvSnapshot::default(),
            vec![],
        );
        again.restore_saved_startup_files();
        let targets = again.shell_targets();
        assert_eq!(targets.len(), 2);
        assert_eq!(targets[1].path, fish);
        assert_eq!(targets[1].syntax, Some(Syntax::Fish));
    }
}
