//! Where the app keeps its own files.
//!
//! Everything lives under one folder in the per-user local data dir:
//!
//! | OS      | Location                               |
//! |---------|----------------------------------------|
//! | Windows | `%LOCALAPPDATA%\VARden`                |
//! | macOS   | `~/Library/Application Support/VARden` |
//! | Linux   | `~/.local/share/VARden`                |

use std::fs;
use std::path::{Path, PathBuf};

use crate::backup::BackupStore;
use crate::{Error, Result};

const APP_DIR: &str = "VARden";
/// Folders used by earlier builds, newest first; migrated on startup.
const LEGACY_DIRS: [&str; 2] = ["EnvManager", "dev.chreide.envmanager"];

fn data_local() -> PathBuf {
    dirs::data_local_dir().unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")))
}

pub fn app_data_dir() -> PathBuf {
    data_local().join(APP_DIR)
}

pub fn backups_dir() -> PathBuf {
    app_data_dir().join("backups")
}

/// Generated per-shell hook scripts that profiles source.
pub fn hooks_dir() -> PathBuf {
    app_data_dir().join("shell")
}

/// What "Sync to terminals" publishes for the hooks to pick up.
pub fn sync_dir() -> PathBuf {
    app_data_dir().join("sync")
}

/// Diagnostic logs (rotated by the app).
pub fn logs_dir() -> PathBuf {
    app_data_dir().join("logs")
}

pub fn legacy_data_dirs() -> Vec<PathBuf> {
    LEGACY_DIRS.iter().map(|d| data_local().join(d)).collect()
}

/// Move everything from older app folders into `new_root`, then remove the
/// old folders. Backups are merged (originals included); other folders are
/// moved when `new_root` doesn't have them yet (sync state, hook scripts)
/// and dropped otherwise (old logs, the old WebView cache). An old folder is
/// only removed once all of its backups were moved. Returns what was done,
/// for the log.
pub fn migrate_legacy(new_root: &Path, legacy: &[PathBuf]) -> Result<Vec<String>> {
    let mut done = Vec::new();
    for old in legacy.iter().filter(|d| d.is_dir()) {
        fs::create_dir_all(new_root).map_err(|e| Error::io(new_root, e))?;
        let entries = fs::read_dir(old).map_err(|e| Error::io(old, e))?;
        for entry in entries.flatten() {
            let name = entry.file_name();
            let (from, to) = (entry.path(), new_root.join(&name));
            if name == "backups" {
                let moved = BackupStore::new(&to).adopt_from(&from)?;
                done.push(format!("moved {moved} backup(s) from {}", old.display()));
            } else if !to.exists() && from.is_dir() && fs::rename(&from, &to).is_ok() {
                done.push(format!("moved {}", from.display()));
            }
        }
        fs::remove_dir_all(old).map_err(|e| Error::io(old, e))?;
        done.push(format!("removed {}", old.display()));
    }
    Ok(done)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_both_older_folders_into_the_new_one() {
        let base = tempfile::tempdir().unwrap();
        let (new, env_mgr, chreide) = (
            base.path().join("VARden"),
            base.path().join("EnvManager"),
            base.path().join("dev.chreide.envmanager"),
        );
        let rc = base.path().join(".bashrc");

        // EnvManager: backups, sync state and logs.
        let old_store = BackupStore::new(env_mgr.join("backups"));
        fs::write(&rc, "v1").unwrap();
        let original = old_store
            .backup_file(&rc, "first")
            .unwrap()
            .entry
            .id
            .clone();
        fs::create_dir_all(env_mgr.join("sync")).unwrap();
        fs::write(env_mgr.join("sync/generation"), "7\n").unwrap();
        fs::create_dir_all(env_mgr.join("logs")).unwrap();
        // dev.chreide.envmanager: one more backup and a WebView cache.
        fs::write(&rc, "v0").unwrap();
        BackupStore::new(chreide.join("backups"))
            .backup_file(&rc, "older")
            .unwrap();
        fs::create_dir_all(chreide.join("EBWebView")).unwrap();
        // The new folder already has logs (the log plugin starts first).
        fs::create_dir_all(new.join("logs")).unwrap();
        fs::write(new.join("logs/varden.log"), "x").unwrap();

        migrate_legacy(&new, &[env_mgr.clone(), chreide.clone()]).unwrap();

        assert!(!env_mgr.exists() && !chreide.exists());
        assert_eq!(
            fs::read_to_string(new.join("sync/generation")).unwrap(),
            "7\n"
        );
        assert_eq!(
            fs::read_to_string(new.join("logs/varden.log")).unwrap(),
            "x"
        );
        let backups = BackupStore::new(new.join("backups")).list().unwrap();
        assert_eq!(backups.len(), 2);
        assert!(backups.iter().any(|b| b.id == original));
        // Nothing left to do on the next start.
        assert!(migrate_legacy(&new, &[env_mgr, chreide])
            .unwrap()
            .is_empty());
    }
}
