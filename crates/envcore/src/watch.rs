//! Notices when variables change outside the app (`setx`, the Windows
//! dialog, another editor touching `.zshrc`, ...), so the dashboard can
//! refresh on its own.
//!
//! * Windows: a background thread blocks on `RegNotifyChangeKeyValue` for
//!   `HKCU\Environment`.
//! * Every platform: a file watcher on the folders holding the startup files
//!   (watching the folder catches atomic renames, which replace the file).
//!
//! Events arriving in a burst are combined into one callback.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::{Duration, Instant};

use notify::event::{AccessKind, AccessMode};
use notify::{EventKind, RecursiveMode, Watcher as _};

/// Quiet period that ends a burst of events.
const SETTLE: Duration = Duration::from_millis(300);
/// A burst never delays the callback longer than this.
const MAX_DELAY: Duration = Duration::from_secs(2);

/// Keeps the watchers alive; dropping it stops file watching.
pub struct Watcher {
    _files: Option<notify::RecommendedWatcher>,
}

/// Where a change was noticed, for logging.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Registry,
    File,
}

pub fn spawn(files: Vec<PathBuf>, on_change: impl Fn(Source) + Send + 'static) -> Watcher {
    let (tx, rx) = mpsc::channel::<Source>();

    thread::Builder::new()
        .name("env-watch-debounce".into())
        .spawn(move || {
            while let Ok(mut source) = rx.recv() {
                let started = Instant::now();
                while started.elapsed() < MAX_DELAY {
                    match rx.recv_timeout(SETTLE) {
                        Ok(s) => source = s,
                        Err(mpsc::RecvTimeoutError::Timeout) => break,
                        Err(mpsc::RecvTimeoutError::Disconnected) => return,
                    }
                }
                on_change(source);
            }
        })
        .ok();

    #[cfg(windows)]
    registry::spawn(tx.clone());

    let files = match watch_files(&files, tx) {
        Ok(w) => Some(w),
        Err(e) => {
            log::warn!("File watching unavailable: {e}");
            None
        }
    };
    Watcher { _files: files }
}

/// Does this event change content (as opposed to merely reading it)?
fn is_write(kind: &EventKind) -> bool {
    match kind {
        EventKind::Access(AccessKind::Close(AccessMode::Write)) => true,
        // Opens and read-closes: the app's own refreshes read these files.
        EventKind::Access(_) => false,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) | EventKind::Any => true,
        EventKind::Other => false,
    }
}

fn canonical_dir(dir: &Path) -> PathBuf {
    std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf())
}

fn watch_files(
    files: &[PathBuf],
    tx: Sender<Source>,
) -> notify::Result<notify::RecommendedWatcher> {
    // Follow symlinks: dotfile managers point ~/.zshrc elsewhere.
    let targets: BTreeSet<PathBuf> = files
        .iter()
        .map(|f| crate::fsutil::resolve_target(f))
        .filter_map(|f| Some(canonical_dir(f.parent()?).join(f.file_name()?)))
        .collect();
    let dirs: BTreeSet<PathBuf> = targets
        .iter()
        .filter_map(|t| t.parent().map(Path::to_path_buf))
        .filter(|d| d.is_dir())
        .collect();

    let wanted = targets.clone();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        let Ok(event) = res else { return };
        if !is_write(&event.kind) {
            return;
        }
        let hit = event.paths.iter().any(|p| {
            let p = match (p.parent(), p.file_name()) {
                (Some(d), Some(n)) => canonical_dir(d).join(n),
                _ => p.clone(),
            };
            wanted.contains(&p)
        });
        if hit {
            let _ = tx.send(Source::File);
        }
    })?;
    for dir in &dirs {
        watcher.watch(dir, RecursiveMode::NonRecursive)?;
    }
    log::info!(
        "Watching {} startup file(s) for outside changes",
        targets.len()
    );
    Ok(watcher)
}

#[cfg(windows)]
mod registry {
    use super::Source;
    use std::sync::mpsc::Sender;

    /// Blocks a background thread on registry change notifications.
    pub fn spawn(tx: Sender<Source>) {
        let started = std::thread::Builder::new()
            .name("env-watch-registry".into())
            .spawn(move || {
                use windows_sys::Win32::System::Registry::{
                    HKEY, HKEY_CURRENT_USER, KEY_NOTIFY, REG_NOTIFY_CHANGE_LAST_SET,
                    REG_NOTIFY_CHANGE_NAME, RegCloseKey, RegNotifyChangeKeyValue, RegOpenKeyExW,
                };
                let name: Vec<u16> = "Environment\0".encode_utf16().collect();
                let mut key: HKEY = std::ptr::null_mut();
                // SAFETY: `name` is NUL-terminated; `key` receives an owned handle
                // that is closed below.
                unsafe {
                    if RegOpenKeyExW(HKEY_CURRENT_USER, name.as_ptr(), 0, KEY_NOTIFY, &mut key) != 0
                    {
                        log::warn!(
                            "Can't watch HKCU\\Environment; outside changes need a manual refresh"
                        );
                        return;
                    }
                    log::info!("Watching HKCU\\Environment for outside changes");
                    loop {
                        // Synchronous: returns when a value is added, removed or changed.
                        let rc = RegNotifyChangeKeyValue(
                            key,
                            0,
                            REG_NOTIFY_CHANGE_NAME | REG_NOTIFY_CHANGE_LAST_SET,
                            std::ptr::null_mut(),
                            0,
                        );
                        if rc != 0 {
                            log::warn!("Registry watch stopped (error {rc})");
                            break;
                        }
                        if tx.send(Source::Registry).is_err() {
                            break;
                        }
                    }
                    RegCloseKey(key);
                }
            });
        if let Err(e) = started {
            log::warn!("Can't start the registry watcher: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn notices_writes_and_renames_but_not_reads() {
        let dir = tempfile::tempdir().unwrap();
        let rc = dir.path().join(".zshrc");
        let other = dir.path().join("unrelated.txt");
        std::fs::write(&rc, "a\n").unwrap();

        let hits = Arc::new(Mutex::new(0));
        let h = hits.clone();
        let _w = spawn(vec![rc.clone()], move |s| {
            assert_eq!(s, Source::File);
            *h.lock().unwrap() += 1;
        });
        let settle = || std::thread::sleep(SETTLE * 3);
        settle();

        let _ = std::fs::read_to_string(&rc).unwrap();
        std::fs::write(&other, "x").unwrap();
        settle();
        assert_eq!(
            *hits.lock().unwrap(),
            0,
            "reads and other files are ignored"
        );

        // An atomic replace, like the app (and most editors) do.
        crate::fsutil::write_atomic(&rc, b"b\n").unwrap();
        std::fs::write(&rc, "c\n").unwrap();
        settle();
        assert_eq!(*hits.lock().unwrap(), 1, "one burst, one callback");
    }
}
