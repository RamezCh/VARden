//! Where the app keeps its own files.
//!
//! Everything lives under one neutral folder in the per-user local data dir:
//!
//! | OS      | Location                                   |
//! |---------|--------------------------------------------|
//! | Windows | `%LOCALAPPDATA%\EnvManager`                |
//! | macOS   | `~/Library/Application Support/EnvManager` |
//! | Linux   | `~/.local/share/EnvManager`                |

use std::path::PathBuf;

const APP_DIR: &str = "EnvManager";
/// Folder used by builds before the rename; migrated on startup.
const LEGACY_DIR: &str = "dev.chreide.envmanager";

pub fn app_data_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")))
        .join(APP_DIR)
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

pub fn legacy_data_dir() -> Option<PathBuf> {
    dirs::data_local_dir().map(|d| d.join(LEGACY_DIR))
}
