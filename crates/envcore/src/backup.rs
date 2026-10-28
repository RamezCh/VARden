//! Timestamped backups taken before every modification.
//!
//! Layout (inside the app's local data dir):
//!
//! ```text
//! backups/
//!   20261028T081530.123Z/
//!     meta.json      ← BackupEntry
//!     payload        ← exact original bytes (shell file) or JSON (registry)
//! ```

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::model::StoredVar;
use crate::{fsutil, Error, Result};

const META: &str = "meta.json";
const PAYLOAD: &str = "payload";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum BackupTarget {
    /// A shell config file. `existed: false` means the file was absent, so
    /// restoring the backup deletes it again.
    #[serde(rename_all = "camelCase")]
    ShellFile { path: PathBuf, existed: bool },
    /// A full snapshot of HKCU\Environment.
    Registry,
}

impl BackupTarget {
    fn same_target(&self, other: &BackupTarget) -> bool {
        match (self, other) {
            (BackupTarget::Registry, BackupTarget::Registry) => true,
            (BackupTarget::ShellFile { path: a, .. }, BackupTarget::ShellFile { path: b, .. }) => {
                a == b
            }
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupEntry {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub target: BackupTarget,
    /// Why the backup was taken, e.g. "set OPENAI_API_KEY".
    pub reason: String,
    pub size_bytes: u64,
    /// The first backup ever taken of this target, i.e. its state before
    /// VARden touched it. Never pruned. Computed by `list`.
    #[serde(default)]
    pub original: bool,
}

/// The result of asking for a backup.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupOutcome {
    #[serde(flatten)]
    pub entry: BackupEntry,
    /// `true` when no new backup was written because an existing one
    /// already holds exactly this state; `entry` is that existing backup.
    pub reused: bool,
}

impl std::ops::Deref for BackupOutcome {
    type Target = BackupEntry;
    fn deref(&self) -> &BackupEntry {
        &self.entry
    }
}

/// Result of deleting backups.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteReport {
    pub deleted: usize,
    /// Originals that were asked for but kept: they are never deleted.
    pub kept_originals: usize,
}

#[derive(Debug, Clone)]
pub struct BackupStore {
    root: PathBuf,
    /// How many backups to keep per target (besides the original) before
    /// pruning the oldest.
    keep_per_target: usize,
}

impl BackupStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        BackupStore {
            root: root.into(),
            keep_per_target: 50,
        }
    }

    pub fn with_retention(mut self, keep_per_target: usize) -> Self {
        self.keep_per_target = keep_per_target.max(1);
        self
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Back up a shell file exactly as it is on disk right now.
    pub fn backup_file(&self, path: &Path, reason: &str) -> Result<BackupOutcome> {
        let target_path = fsutil::resolve_target(path);
        let (bytes, existed) = match fs::read(&target_path) {
            Ok(b) => (b, true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (Vec::new(), false),
            Err(e) => return Err(Error::io(&target_path, e)),
        };
        self.write_backup(
            BackupTarget::ShellFile {
                path: path.to_path_buf(),
                existed,
            },
            reason,
            &bytes,
        )
    }

    /// Back up a snapshot of every user-level registry variable.
    pub fn backup_registry(&self, snapshot: &[StoredVar], reason: &str) -> Result<BackupOutcome> {
        let bytes = serde_json::to_vec_pretty(snapshot)?;
        self.write_backup(BackupTarget::Registry, reason, &bytes)
    }

    fn write_backup(
        &self,
        target: BackupTarget,
        reason: &str,
        payload: &[u8],
    ) -> Result<BackupOutcome> {
        // Only distinct states are kept: if this exact content was already
        // backed up for this target, that backup already covers it.
        if let Some(existing) = self.find_duplicate(&target, payload)? {
            return Ok(BackupOutcome {
                entry: existing,
                reused: true,
            });
        }
        fs::create_dir_all(&self.root).map_err(|e| Error::io(&self.root, e))?;
        let now = Utc::now();
        let base = now.format("%Y%m%dT%H%M%S%.3fZ").to_string();
        let mut id = base.clone();
        let mut n = 1;
        while self.root.join(&id).exists() {
            id = format!("{base}-{n}");
            n += 1;
        }
        let dir = self.root.join(&id);
        fs::create_dir(&dir).map_err(|e| Error::io(&dir, e))?;

        let mut entry = BackupEntry {
            id,
            created_at: now,
            target,
            reason: reason.to_string(),
            size_bytes: payload.len() as u64,
            original: false,
        };
        let payload_path = dir.join(PAYLOAD);
        fs::write(&payload_path, payload).map_err(|e| Error::io(&payload_path, e))?;
        // meta.json is written last: a directory without it is an incomplete
        // backup and is ignored by `list`.
        let meta_path = dir.join(META);
        fs::write(&meta_path, serde_json::to_vec_pretty(&entry)?)
            .map_err(|e| Error::io(&meta_path, e))?;

        self.prune(&entry.target)?;
        entry.original = self.get(&entry.id)?.original;
        Ok(BackupOutcome {
            entry,
            reused: false,
        })
    }

    /// An existing backup of `target` whose content is byte-identical to `payload`.
    fn find_duplicate(&self, target: &BackupTarget, payload: &[u8]) -> Result<Option<BackupEntry>> {
        for entry in self.list()? {
            if entry.target == *target
                && entry.size_bytes == payload.len() as u64
                && self.payload(&entry.id).is_ok_and(|p| p == payload)
            {
                return Ok(Some(entry));
            }
        }
        Ok(None)
    }

    /// Move every backup from an older backup folder into this store.
    /// Entries already present here are left as they are. Returns how many
    /// were moved; errors if any could not be, so the caller keeps the old
    /// folder in that case.
    pub fn adopt_from(&self, old_root: &Path) -> Result<usize> {
        let rd = match fs::read_dir(old_root) {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
            Err(e) => return Err(Error::io(old_root, e)),
        };
        fs::create_dir_all(&self.root).map_err(|e| Error::io(&self.root, e))?;
        let mut moved = 0;
        for dirent in rd.flatten() {
            let src = dirent.path();
            if !src.join(META).is_file() {
                continue;
            }
            let dst = self.root.join(dirent.file_name());
            if dst.exists() {
                continue;
            }
            if fs::rename(&src, &dst).is_err() {
                // Different volume: copy, then remove the source.
                fs::create_dir(&dst).map_err(|e| Error::io(&dst, e))?;
                for name in [PAYLOAD, META] {
                    fs::copy(src.join(name), dst.join(name))
                        .map_err(|e| Error::io(src.join(name), e))?;
                }
                let _ = fs::remove_dir_all(&src);
            }
            moved += 1;
        }
        Ok(moved)
    }

    /// Delete the given backups. Originals (each target's state from before
    /// the app first changed it) are always kept; unknown ids are ignored.
    pub fn delete(&self, ids: &[String]) -> Result<DeleteReport> {
        let all = self.list()?;
        let mut report = DeleteReport::default();
        for id in ids {
            Self::check_id(id)?;
            let Some(entry) = all.iter().find(|e| &e.id == id) else {
                continue;
            };
            if entry.original {
                report.kept_originals += 1;
                continue;
            }
            let dir = self.root.join(id);
            // Remove meta.json first so a half-deleted folder is never listed.
            fs::remove_file(dir.join(META)).map_err(|e| Error::io(dir.join(META), e))?;
            let _ = fs::remove_dir_all(&dir);
            report.deleted += 1;
        }
        Ok(report)
    }

    /// All complete backups, newest first.
    pub fn list(&self) -> Result<Vec<BackupEntry>> {
        let rd = match fs::read_dir(&self.root) {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(Error::io(&self.root, e)),
        };
        let mut out = Vec::new();
        for dirent in rd.flatten() {
            let meta = dirent.path().join(META);
            if let Ok(bytes) = fs::read(&meta) {
                if let Ok(entry) = serde_json::from_slice::<BackupEntry>(&bytes) {
                    out.push(entry);
                }
            }
        }
        out.sort_by(|a, b| b.created_at.cmp(&a.created_at).then(b.id.cmp(&a.id)));
        // The oldest backup of each target is its original state.
        for i in 0..out.len() {
            let first_of_target = !out[i + 1..]
                .iter()
                .any(|e| e.target.same_target(&out[i].target));
            out[i].original = first_of_target;
        }
        Ok(out)
    }

    /// ids are generated by us; refuse anything that could escape the root.
    fn check_id(id: &str) -> Result<()> {
        if id.is_empty() || id.contains(['/', '\\']) || id.contains("..") {
            return Err(Error::BackupNotFound(id.to_string()));
        }
        Ok(())
    }

    pub fn get(&self, id: &str) -> Result<BackupEntry> {
        Self::check_id(id)?;
        let meta = self.root.join(id).join(META);
        if !meta.is_file() {
            return Err(Error::BackupNotFound(id.to_string()));
        }
        self.list()?
            .into_iter()
            .find(|e| e.id == id)
            .ok_or_else(|| Error::BackupNotFound(id.to_string()))
    }

    pub fn payload(&self, id: &str) -> Result<Vec<u8>> {
        Self::check_id(id)?;
        let p = self.root.join(id).join(PAYLOAD);
        fs::read(&p).map_err(|e| Error::io(&p, e))
    }

    pub fn registry_snapshot(&self, id: &str) -> Result<Vec<StoredVar>> {
        Ok(serde_json::from_slice(&self.payload(id)?)?)
    }

    /// Label for the safety backup taken before restoring `entry`.
    pub fn restore_reason(entry: &BackupEntry) -> String {
        format!(
            "Before restoring the backup from {}",
            entry.created_at.format("%Y-%m-%d %H:%M:%S UTC")
        )
    }

    /// Restore a shell-file backup. The current state is backed up first
    /// (unless an identical backup already exists), so a restore can itself
    /// be undone.
    pub fn restore_file(&self, id: &str) -> Result<BackupOutcome> {
        let entry = self.get(id)?;
        let BackupTarget::ShellFile { path, existed } = &entry.target else {
            return Err(Error::Unsupported("restoring a registry backup as a file"));
        };
        // Read the payload first: the safety backup below may prune old
        // entries, possibly including the one being restored.
        let payload = self.payload(id)?;
        let safety = self.backup_file(path, &Self::restore_reason(&entry))?;
        if *existed {
            fsutil::write_atomic(path, &payload)?;
        } else {
            let target = fsutil::resolve_target(path);
            match fs::remove_file(&target) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(Error::io(&target, e)),
            }
        }
        Ok(safety)
    }

    fn prune(&self, target: &BackupTarget) -> Result<()> {
        let same: Vec<BackupEntry> = self
            .list()?
            .into_iter()
            .filter(|e| e.target.same_target(target))
            .collect();
        // `same` is newest first; the last one is the original and is kept.
        let prunable = same.len().saturating_sub(1);
        for old in same[..prunable].iter().skip(self.keep_per_target) {
            let _ = fs::remove_dir_all(self.root.join(&old.id));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_backup_and_restore() {
        let dir = tempfile::tempdir().unwrap();
        let store = BackupStore::new(dir.path().join("backups"));
        let rc = dir.path().join(".bashrc");
        fs::write(&rc, "original\n").unwrap();

        let b = store.backup_file(&rc, "test").unwrap();
        fs::write(&rc, "broken\n").unwrap();
        let safety = store.restore_file(&b.id).unwrap();

        assert_eq!(fs::read_to_string(&rc).unwrap(), "original\n");
        assert_eq!(store.payload(&safety.id).unwrap(), b"broken\n");
        assert_eq!(store.list().unwrap().len(), 2);
    }

    #[test]
    fn restoring_absent_file_deletes_it() {
        let dir = tempfile::tempdir().unwrap();
        let store = BackupStore::new(dir.path().join("backups"));
        let rc = dir.path().join(".zshrc");
        let b = store.backup_file(&rc, "first write").unwrap();
        fs::write(&rc, "created by app\n").unwrap();
        store.restore_file(&b.id).unwrap();
        assert!(!rc.exists());
    }

    #[test]
    fn prunes_per_target() {
        let dir = tempfile::tempdir().unwrap();
        let store = BackupStore::new(dir.path().join("b")).with_retention(3);
        let a = dir.path().join("a");
        let b = dir.path().join("b_rc");
        for i in 0..5 {
            fs::write(&a, format!("version {i}")).unwrap();
            store.backup_file(&a, "x").unwrap();
        }
        store.backup_file(&b, "x").unwrap();
        let all = store.list().unwrap();
        let count = |p: &Path| {
            all.iter()
                .filter(|e| matches!(&e.target, BackupTarget::ShellFile { path, .. } if path == p))
                .count()
        };
        assert_eq!(count(&a), 4, "3 newest + the original");
        assert_eq!(
            all.iter().filter(|e| e.original).count(),
            2,
            "one per target"
        );
        assert!(all
            .iter()
            .any(|e| e.original && store.payload(&e.id).unwrap() == b"version 0"));
        assert_eq!(count(&b), 1);
    }

    #[test]
    fn rejects_path_traversal_ids() {
        let dir = tempfile::tempdir().unwrap();
        let store = BackupStore::new(dir.path());
        assert!(store.get("../etc").is_err());
        assert!(store.get("a/b").is_err());
    }

    #[test]
    fn identical_states_are_backed_up_once() {
        // Original -> set X -> delete X -> restore "with X":
        // only two distinct states ever existed, so only two backups.
        let dir = tempfile::tempdir().unwrap();
        let store = BackupStore::new(dir.path().join("backups"));
        let rc = dir.path().join(".bashrc");
        fs::write(&rc, "original\n").unwrap();

        let original = store.backup_file(&rc, "Before setting X").unwrap();
        fs::write(&rc, "original\nexport X=1\n").unwrap();
        let with_x = store.backup_file(&rc, "Before removing X").unwrap();
        fs::write(&rc, "original\n").unwrap();
        let safety = store.restore_file(&with_x.id).unwrap();

        assert!(!original.reused && original.original);
        assert!(!with_x.reused && !with_x.original);
        assert!(safety.reused, "caller is told no new backup was made");
        assert_eq!(
            safety.id, original.id,
            "safety backup reuses the identical original"
        );
        assert_eq!(store.list().unwrap().len(), 2);
        assert_eq!(fs::read_to_string(&rc).unwrap(), "original\nexport X=1\n");
    }

    #[test]
    fn absent_and_empty_file_are_different_states() {
        let dir = tempfile::tempdir().unwrap();
        let store = BackupStore::new(dir.path().join("backups"));
        let rc = dir.path().join(".zshrc");
        store.backup_file(&rc, "absent").unwrap();
        fs::write(&rc, "").unwrap();
        store.backup_file(&rc, "empty").unwrap();
        assert_eq!(store.list().unwrap().len(), 2);
    }

    #[test]
    fn adopts_backups_from_old_folder() {
        let dir = tempfile::tempdir().unwrap();
        let old = BackupStore::new(dir.path().join("old"));
        let rc = dir.path().join(".bashrc");
        fs::write(&rc, "v1").unwrap();
        let original = old.backup_file(&rc, "first").unwrap();
        fs::write(&rc, "v2").unwrap();
        old.backup_file(&rc, "second").unwrap();

        let new = BackupStore::new(dir.path().join("new"));
        assert_eq!(new.adopt_from(old.root()).unwrap(), 2);
        let listed = new.list().unwrap();
        assert_eq!(listed.len(), 2);
        assert!(listed.iter().any(|e| e.id == original.id && e.original));
        assert!(old.list().unwrap().is_empty());
        assert_eq!(new.adopt_from(&dir.path().join("missing")).unwrap(), 0);
    }

    #[test]
    fn delete_keeps_originals() {
        let dir = tempfile::tempdir().unwrap();
        let store = BackupStore::new(dir.path().join("b"));
        let rc = dir.path().join(".bashrc");
        let mut ids = Vec::new();
        for i in 0..3 {
            fs::write(&rc, format!("v{i}")).unwrap();
            ids.push(store.backup_file(&rc, "x").unwrap().id.clone());
        }
        let report = store
            .delete(&[ids[0].clone(), ids[1].clone(), "missing".to_string()])
            .unwrap();
        assert_eq!(
            report,
            DeleteReport {
                deleted: 1,
                kept_originals: 1
            }
        );
        let left: Vec<String> = store.list().unwrap().into_iter().map(|e| e.id).collect();
        assert_eq!(left, vec![ids[2].clone(), ids[0].clone()]);
        assert!(store
            .list()
            .unwrap()
            .iter()
            .any(|e| e.id == ids[0] && e.original));
        assert!(store.delete(&["../x".to_string()]).is_err());
    }
}
