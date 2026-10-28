use std::path::{Path, PathBuf};

use crate::backup::{BackupOutcome, BackupStore};
use crate::managed_block::{self, Block, Syntax};
use crate::model::{StoredVar, ValueKind};
use crate::store::{PersistentStore, StoreInfo};
use crate::validate::{validate_key, validate_value};
use crate::{fsutil, Error, Result};

/// A shell config file whose managed block holds our variables.
#[derive(Debug, Clone)]
pub struct ShellFileStore {
    path: PathBuf,
    syntax: Syntax,
}

impl ShellFileStore {
    pub fn new(path: impl Into<PathBuf>, syntax: Syntax) -> Self {
        ShellFileStore {
            path: path.into(),
            syntax,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn syntax(&self) -> Syntax {
        self.syntax
    }

    fn malformed(&self, reason: String) -> Error {
        Error::MalformedBlock {
            path: self.path.clone(),
            reason,
        }
    }

    fn read(&self) -> Result<(fsutil::TextFile, Block)> {
        let file = fsutil::read_text(&self.path)?;
        let block = managed_block::read_block(&file.content, self.syntax)
            .map_err(|r| self.malformed(r))?
            .unwrap_or_default();
        Ok((file, block))
    }

    /// Apply `edit` to the block; back up and write only if the file changes.
    fn modify(
        &self,
        reason: &str,
        backups: &BackupStore,
        edit: impl FnOnce(&mut Block),
    ) -> Result<Option<BackupOutcome>> {
        let (mut file, mut block) = self.read()?;
        edit(&mut block);
        let new_content = managed_block::write_block(&file.content, self.syntax, &block)
            .map_err(|r| self.malformed(r))?;
        if new_content == file.content {
            return Ok(None);
        }
        let backup = backups.backup_file(&self.path, reason)?;
        file.content = new_content;
        fsutil::write_atomic(&self.path, file.render().as_bytes())?;
        Ok(Some(backup))
    }
}

impl PersistentStore for ShellFileStore {
    fn info(&self) -> StoreInfo {
        StoreInfo::ShellFile {
            path: self.path.clone(),
            syntax: self.syntax,
        }
    }

    fn list(&self) -> Result<Vec<StoredVar>> {
        let (_, block) = self.read()?;
        let mut out: Vec<StoredVar> = block
            .entries()
            .into_iter()
            .map(|(key, value)| StoredVar {
                key,
                value,
                kind: ValueKind::String,
            })
            .collect();
        for (key, raw) in block.opaque_assignments() {
            out.retain(|v| v.key != key);
            out.push(StoredVar {
                key,
                value: raw,
                kind: ValueKind::ExpandString,
            });
        }
        Ok(out)
    }

    fn set(&self, key: &str, value: &str, backups: &BackupStore) -> Result<Option<BackupOutcome>> {
        self.set_many(
            &[(key.to_string(), value.to_string())],
            &format!("Before setting {key}"),
            backups,
        )
    }

    fn update_many(
        &self,
        changes: &[(String, Option<String>)],
        reason: &str,
        backups: &BackupStore,
    ) -> Result<Option<BackupOutcome>> {
        for (key, value) in changes {
            validate_key(key)?;
            let Some(value) = value else { continue };
            validate_value(key, value)?;
            if self.syntax != Syntax::Nushell && value.contains(['\n', '\r']) {
                return Err(Error::InvalidValue {
                    key: key.clone(),
                    reason: "multi-line values can't be stored in this shell file".into(),
                });
            }
        }
        self.modify(reason, backups, |b| {
            for (key, value) in changes {
                match value {
                    Some(value) => b.set(key, value),
                    None => {
                        b.remove(key);
                    }
                }
            }
        })
    }

    fn remove(&self, key: &str, backups: &BackupStore) -> Result<Option<BackupOutcome>> {
        self.modify(&format!("Before removing {key}"), backups, |b| {
            b.remove(key);
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn set_list_remove_with_backups() {
        let dir = tempfile::tempdir().unwrap();
        let rc = dir.path().join(".zshrc");
        fs::write(&rc, "source ~/.aliases\r\n").unwrap();
        let backups = BackupStore::new(dir.path().join("backups"));
        let store = ShellFileStore::new(&rc, Syntax::Posix);

        assert!(store.set("API_KEY", "s3cr'et", &backups).unwrap().is_some());
        // Same value again: no write, no extra backup.
        assert!(store.set("API_KEY", "s3cr'et", &backups).unwrap().is_none());
        assert_eq!(backups.list().unwrap().len(), 1);

        let vars = store.list().unwrap();
        assert_eq!(vars.len(), 1);
        assert_eq!(vars[0].value, "s3cr'et");

        let content = fs::read_to_string(&rc).unwrap();
        assert!(
            content.starts_with("source ~/.aliases\r\n\r\n# >>>"),
            "CRLF preserved: {content:?}"
        );

        assert!(store.remove("API_KEY", &backups).unwrap().is_some());
        assert!(store.remove("API_KEY", &backups).unwrap().is_none());
        assert_eq!(fs::read_to_string(&rc).unwrap(), "source ~/.aliases\r\n");
    }

    #[test]
    fn rejects_bad_input_without_touching_file() {
        let dir = tempfile::tempdir().unwrap();
        let rc = dir.path().join(".bashrc");
        let backups = BackupStore::new(dir.path().join("backups"));
        let store = ShellFileStore::new(&rc, Syntax::Posix);
        assert!(store.set("1BAD", "x", &backups).is_err());
        assert!(store.set("OK", "two\nlines", &backups).is_err());
        assert!(!rc.exists());
        assert!(backups.list().unwrap().is_empty());
    }

    #[test]
    fn creates_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = dir.path().join("fish").join("config.fish");
        let backups = BackupStore::new(dir.path().join("backups"));
        let store = ShellFileStore::new(&cfg, Syntax::Fish);
        store.set("X", "1", &backups).unwrap();
        assert!(fs::read_to_string(&cfg).unwrap().contains("set -gx X '1'"));
    }

    #[test]
    fn malformed_block_is_never_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let rc = dir.path().join(".bashrc");
        let original = format!("{}\nexport A=1\n", managed_block::START_MARKER);
        fs::write(&rc, &original).unwrap();
        let backups = BackupStore::new(dir.path().join("backups"));
        let store = ShellFileStore::new(&rc, Syntax::Posix);
        assert!(matches!(
            store.set("B", "2", &backups),
            Err(Error::MalformedBlock { .. })
        ));
        assert_eq!(fs::read_to_string(&rc).unwrap(), original);
    }
}
