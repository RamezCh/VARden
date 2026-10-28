//! `.env` import and export on top of the manager.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::Result;
use crate::backup::BackupOutcome;
use crate::dotenv::{self, EnvEntry};
use crate::manager::{EnvManager, resolver};
use crate::model::Scope;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportItem {
    pub key: String,
    pub scope: Scope,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportReport {
    pub path: PathBuf,
    pub written: usize,
    /// Variables left out, with the reason.
    pub skipped: Vec<(String, String)>,
}

impl EnvManager {
    pub fn import_env(&self, entries: &[EnvEntry], scope: Scope) -> Result<Vec<BackupOutcome>> {
        let pairs: Vec<(String, String)> = entries
            .iter()
            .map(|e| (e.key.clone(), e.value.clone()))
            .collect();
        self.set_many(&pairs, scope)
    }

    /// Write the chosen variables to a `.env` file. Windows `%VAR%` values
    /// are written expanded, since `.env` loaders don't expand them.
    pub fn export_env(&self, path: &Path, items: &[ExportItem]) -> Result<ExportReport> {
        let persistent = self.persistent()?;
        let overlay = self.process_overlay();
        let same = |a: &str, b: &str| {
            if self.ci {
                a.eq_ignore_ascii_case(b)
            } else {
                a == b
            }
        };

        let mut entries = Vec::new();
        let mut skipped = Vec::new();
        for item in items {
            let value = match item.scope {
                Scope::Process => overlay
                    .iter()
                    .find(|(k, _)| same(k, &item.key))
                    .map(|(_, v)| v.clone()),
                Scope::User => match persistent.iter().find(|v| same(&v.key, &item.key)) {
                    Some(var) => match resolver(var) {
                        Some(v) => Some(v),
                        None => {
                            skipped.push((
                                item.key.clone(),
                                "uses shell syntax that a .env file can't express".into(),
                            ));
                            continue;
                        }
                    },
                    None => None,
                },
            };
            match value {
                Some(value) => entries.push(EnvEntry {
                    key: item.key.clone(),
                    value,
                }),
                None => skipped.push((item.key.clone(), "no longer exists".into())),
            }
        }
        dotenv::write_file(path, &entries)?;
        Ok(ExportReport {
            path: path.to_path_buf(),
            written: entries.len(),
            skipped,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backup::BackupStore;
    use crate::managed_block::Syntax;
    use crate::status::EnvSnapshot;
    use crate::store::shell_file::ShellFileStore;
    use std::sync::Arc;

    #[test]
    fn import_then_export_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let rc = dir.path().join(".zshrc");
        let m = EnvManager::with_stores(
            BackupStore::new(dir.path().join("backups")),
            EnvSnapshot::default(),
            vec![Arc::new(ShellFileStore::new(&rc, Syntax::Posix))],
        );
        let src = dir.path().join("in.env");
        std::fs::write(
            &src,
            "DB_URL=\"postgres://u:p@h/db\"\nexport GREETING='hi there'\n",
        )
        .unwrap();
        let parsed = dotenv::read_file(&src).unwrap();
        assert_eq!(m.import_env(&parsed.entries, Scope::User).unwrap().len(), 1);
        m.set("SESSION_ONLY", "s", Scope::Process).unwrap();

        // A hand-written expression in the block can't be exported.
        let content = std::fs::read_to_string(&rc).unwrap();
        std::fs::write(
            &rc,
            content.replace("# <<< END", "export LIBS=\"$HOME/lib\"\n# <<< END"),
        )
        .unwrap();

        let out = dir.path().join("out.env");
        let items = [
            ExportItem {
                key: "DB_URL".into(),
                scope: Scope::User,
            },
            ExportItem {
                key: "GREETING".into(),
                scope: Scope::User,
            },
            ExportItem {
                key: "SESSION_ONLY".into(),
                scope: Scope::Process,
            },
            ExportItem {
                key: "LIBS".into(),
                scope: Scope::User,
            },
            ExportItem {
                key: "GONE".into(),
                scope: Scope::User,
            },
        ];
        let report = m.export_env(&out, &items).unwrap();
        assert_eq!(report.written, 3);
        assert_eq!(
            report
                .skipped
                .iter()
                .map(|(k, _)| k.as_str())
                .collect::<Vec<_>>(),
            ["LIBS", "GONE"]
        );
        let back = dotenv::read_file(&out).unwrap().entries;
        assert_eq!(
            back[0],
            EnvEntry {
                key: "DB_URL".into(),
                value: "postgres://u:p@h/db".into()
            }
        );
        assert_eq!(back[1].value, "hi there");
        assert_eq!(back[2].key, "SESSION_ONLY");
    }
}
