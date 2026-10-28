//! Terminal sync on top of the manager: installing the per-shell hook into
//! profiles, and publishing variables to hooked terminals.

use std::collections::BTreeMap;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::backup::BackupOutcome;
use crate::managed_block::{self, HOOK};
use crate::manager::{lock, resolver, EnvManager};
use crate::shells::{self, DetectedShell, ShellKind};
use crate::sync::{self, Desired, HookShell, SyncState};
use crate::{fsutil, Error, Result};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HookStatus {
    pub kind: ShellKind,
    pub name: &'static str,
    pub profile: Option<PathBuf>,
    /// Why this shell can't get the hook, if it can't.
    pub unsupported: Option<&'static str>,
    pub installed: bool,
    /// The profile couldn't be read or its hook section is damaged.
    pub problem: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalSyncInfo {
    pub generation: u64,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub hooks: Vec<HookStatus>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncReport {
    pub generation: u64,
    pub synced_at: DateTime<Utc>,
    /// Variables pushed (PATH counts once).
    pub pushed: usize,
    /// Variables removed from terminals.
    pub removed: usize,
    /// User variables that couldn't be pushed (shell expressions such as
    /// `"$HOME/bin:$PATH"` only make sense to the shell that defined them).
    pub skipped: Vec<String>,
    pub hooks_installed: usize,
}

fn hook_shell(kind: ShellKind) -> std::result::Result<HookShell, &'static str> {
    match kind {
        ShellKind::Bash | ShellKind::Zsh | ShellKind::GitBash => Ok(HookShell::Posix),
        ShellKind::Fish => Ok(HookShell::Fish),
        ShellKind::PowerShell | ShellKind::WindowsPowerShell => Ok(HookShell::PowerShell),
        ShellKind::Cmd => {
            Err("Command Prompt has no prompt hook. Open a new window to pick up changes.")
        }
        ShellKind::Nushell => {
            Err("Nushell isn't supported yet. Open a new terminal to pick up changes.")
        }
    }
}

const NOTICE: &str =
    "# Applies variables from Environment Manager's \"Sync to terminals\" before each prompt.";

impl EnvManager {
    fn hook_section(&self, shell: HookShell) -> Vec<String> {
        vec![
            HOOK.start.to_string(),
            NOTICE.to_string(),
            sync::profile_line(shell, &sync::hook_file(&self.hooks_dir, shell)),
            HOOK.end.to_string(),
        ]
    }

    fn status_for(&self, s: &DetectedShell) -> HookStatus {
        let mut status = HookStatus {
            kind: s.kind,
            name: s.name,
            profile: s.config_path.clone(),
            unsupported: None,
            installed: false,
            problem: None,
        };
        if let Err(why) = hook_shell(s.kind) {
            status.unsupported = Some(why);
            return status;
        }
        if let Some(path) = &s.config_path {
            match fsutil::read_text(path).map(|f| managed_block::has_section(&f.content, HOOK)) {
                Ok(Ok(installed)) => status.installed = installed,
                Ok(Err(reason)) => {
                    status.problem = Some(format!("The hook section is damaged: {reason}."))
                }
                Err(e) => status.problem = Some(e.to_string()),
            }
        }
        status
    }

    pub fn terminal_sync_info(&self) -> TerminalSyncInfo {
        let state = SyncState::load(&self.sync_dir);
        TerminalSyncInfo {
            generation: state.generation,
            last_synced_at: state.synced_at,
            hooks: shells::detect()
                .iter()
                .map(|s| self.status_for(s))
                .collect(),
        }
    }

    /// Rewrite the hook scripts (they embed the sync folder's path).
    pub fn refresh_hook_scripts(&self) -> Result<()> {
        sync::write_hooks(&self.hooks_dir, &self.sync_dir)
    }

    fn detected(kind: ShellKind) -> Result<(HookShell, PathBuf)> {
        let shell = shells::detect()
            .into_iter()
            .find(|s| s.kind == kind)
            .ok_or(Error::Unsupported("a shell that isn't installed"))?;
        let hook = hook_shell(kind).map_err(Error::Unsupported)?;
        let path = shell
            .config_path
            .ok_or(Error::Unsupported("a shell without a profile file"))?;
        Ok((hook, path))
    }

    /// Add the hook to a shell's profile (backed up first). Returns the
    /// backup, or `None` if the hook was already installed.
    pub fn install_hook(&self, kind: ShellKind) -> Result<Option<BackupOutcome>> {
        let (shell, path) = Self::detected(kind)?;
        self.install_into(&path, shell)
    }

    pub(crate) fn install_into(
        &self,
        path: &std::path::Path,
        shell: HookShell,
    ) -> Result<Option<BackupOutcome>> {
        self.refresh_hook_scripts()?;
        self.edit_profile(
            path,
            &self.hook_section(shell),
            "Before installing terminal sync",
        )
    }

    pub fn uninstall_hook(&self, kind: ShellKind) -> Result<Option<BackupOutcome>> {
        let (_, path) = Self::detected(kind)?;
        self.uninstall_hook_from(&path)
    }

    pub(crate) fn uninstall_hook_from(
        &self,
        path: &std::path::Path,
    ) -> Result<Option<BackupOutcome>> {
        self.edit_profile(path, &[], "Before removing terminal sync")
    }

    fn edit_profile(
        &self,
        path: &std::path::Path,
        section: &[String],
        reason: &str,
    ) -> Result<Option<BackupOutcome>> {
        let _guard = self.begin_write();
        let mut file = fsutil::read_text(path)?;
        if !path.exists()
            && path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("ps1"))
        {
            // New PowerShell profile: BOM so Windows PowerShell 5.1 reads it as UTF-8.
            file.bom = true;
        }
        let malformed = |reason: String| Error::MalformedBlock {
            path: path.to_path_buf(),
            reason,
        };
        let updated =
            managed_block::replace_section(&file.content, HOOK, section).map_err(malformed)?;
        if updated == file.content {
            return Ok(None);
        }
        let backup = self.backups.backup_file(path, reason)?;
        file.content = updated;
        fsutil::write_atomic(path, file.render().as_bytes())?;
        Ok(Some(backup))
    }

    /// What terminals should have: every saved variable the shells can take
    /// literally, the user's PATH entries, and temporary variables (which
    /// win over a saved variable of the same name).
    fn desired(&self) -> Result<(Desired, Vec<String>)> {
        let sep = if cfg!(windows) { ';' } else { ':' };
        let mut desired = Desired::default();
        let mut skipped = Vec::new();
        for var in self.persistent()? {
            let Some(value) = resolver(&var) else {
                skipped.push(var.key);
                continue;
            };
            if var.key.eq_ignore_ascii_case("PATH") {
                desired.path = value
                    .split(sep)
                    .filter(|p| !p.trim().is_empty())
                    .map(str::to_string)
                    .collect();
            } else {
                desired.vars.push((var.key, value));
            }
        }
        for (key, value) in self.process_overlay() {
            if key.eq_ignore_ascii_case("PATH") {
                for p in value.split(sep).filter(|p| !p.trim().is_empty()) {
                    desired.path.push(p.to_string());
                }
                continue;
            }
            let ci = self.ci;
            desired.vars.retain(|(k, _)| {
                if ci {
                    !k.eq_ignore_ascii_case(&key)
                } else {
                    *k != key
                }
            });
            desired.vars.push((key, value));
        }
        Ok((desired, skipped))
    }

    /// Publish the saved variables to every terminal that has the hook.
    /// They pick the change up before their next prompt.
    pub fn sync_terminals(&self) -> Result<SyncReport> {
        let (desired, skipped) = self.desired()?;
        let _guard = self.begin_write();
        let previous = SyncState::load(&self.sync_dir);
        let plan = sync::plan(&desired, &previous, &system_environment(), self.ci);
        self.refresh_hook_scripts()?;
        sync::publish(&self.sync_dir, &plan)?;

        // The app now treats the synced values as "current", so the
        // dashboard shows them as Active & synced.
        {
            let sep = if cfg!(windows) { ';' } else { ':' };
            let mut live = lock(&self.live);
            for (k, v) in &plan.set {
                live.set(k, v.clone());
            }
            for (k, fallback) in &plan.unset {
                match fallback {
                    Some(v) => live.set(k, v.clone()),
                    None => live.remove(k),
                }
            }
            if !plan.path_add.is_empty() || !plan.path_remove.is_empty() {
                let current = live.get("PATH").unwrap_or_default().to_string();
                live.set(
                    "PATH",
                    merge_path(&current, &plan.path_add, &plan.path_remove, sep, self.ci),
                );
            }
        }

        let hooks_installed = shells::detect()
            .iter()
            .map(|s| self.status_for(s))
            .filter(|h| h.installed)
            .count();
        Ok(SyncReport {
            generation: plan.state.generation,
            synced_at: plan.state.synced_at.unwrap_or_else(Utc::now),
            pushed: plan.set.len() + usize::from(!plan.path_add.is_empty()),
            removed: plan.unset.len(),
            skipped,
            hooks_installed,
        })
    }
}

/// The same PATH merge the apply scripts perform.
pub(crate) fn merge_path(
    current: &str,
    add: &[String],
    remove: &[String],
    sep: char,
    ci: bool,
) -> String {
    let norm = |p: &str| {
        let p = p.trim_end_matches(['/', '\\']);
        if ci {
            p.to_ascii_lowercase()
        } else {
            p.to_string()
        }
    };
    let drop: Vec<String> = remove.iter().map(|p| norm(p)).collect();
    let mut out: Vec<String> = current
        .split(sep)
        .filter(|p| !p.is_empty() && !drop.contains(&norm(p)))
        .map(str::to_string)
        .collect();
    for p in add {
        if !out.iter().any(|o| norm(o) == norm(p)) {
            out.push(p.clone());
        }
    }
    out.join(&sep.to_string())
}

#[cfg(windows)]
fn system_environment() -> BTreeMap<String, String> {
    crate::store::windows_registry::system_environment()
}

#[cfg(not(windows))]
fn system_environment() -> BTreeMap<String, String> {
    BTreeMap::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backup::BackupStore;
    use crate::managed_block::Syntax;
    use crate::model::{Scope, Status};
    use crate::status::EnvSnapshot;
    use crate::store::shell_file::ShellFileStore;
    use std::sync::Arc;

    #[test]
    fn merge_path_matches_apply_scripts() {
        let add = vec!["/y".to_string(), "/a".to_string()];
        let rm = vec!["/x".to_string()];
        assert_eq!(
            merge_path("/a:/x:/b:/x/", &add, &rm, ':', false),
            "/a:/b:/y"
        );
        assert_eq!(
            merge_path(r"C:\Win;C:\Tools\", &[r"c:\tools".into()], &[], ';', true),
            r"C:\Win;C:\Tools\"
        );
    }

    #[test]
    fn sync_publishes_and_marks_variables_synced() {
        let dir = tempfile::tempdir().unwrap();
        let rc = dir.path().join(".bashrc");
        let m = EnvManager::with_stores(
            BackupStore::new(dir.path().join("backups")),
            EnvSnapshot::from_pairs([("PATH", "/usr/bin")], false),
            vec![Arc::new(ShellFileStore::new(&rc, Syntax::Posix))],
        )
        .with_dirs(dir.path().join("sync"), dir.path().join("shell"));

        m.set("API_URL", "https://x", Scope::User).unwrap();
        m.set("PATH", "/opt/tool/bin", Scope::User).unwrap();
        std::fs::write(
            &rc,
            std::fs::read_to_string(&rc)
                .unwrap()
                .replace("# <<< END", "export GOPATH=\"$HOME/go\"\n# <<< END"),
        )
        .unwrap();
        assert!(m
            .list(&[])
            .unwrap()
            .iter()
            .all(|r| r.status != Status::ActiveSynced));

        let report = m.sync_terminals().unwrap();
        assert_eq!(
            (report.generation, report.pushed, report.removed),
            (1, 2, 0)
        );
        assert_eq!(report.skipped, vec!["GOPATH".to_string()]);
        assert!(dir.path().join("sync/apply.sh").is_file());
        assert!(dir.path().join("shell/hook.ps1").is_file());

        let rows = m.list(&[]).unwrap();
        let status = |k: &str| rows.iter().find(|r| r.key == k).unwrap().status;
        assert_eq!(status("API_URL"), Status::ActiveSynced);
        assert_eq!(
            status("PATH"),
            Status::ActiveSynced,
            "user PATH entry merged into live PATH"
        );

        // Temporary variables reach terminals too, and leave them when removed.
        m.set("TEMP_FLAG", "on", Scope::Process).unwrap();
        assert!(m
            .list(&[])
            .unwrap()
            .iter()
            .any(|r| r.key == "TEMP_FLAG" && r.pending_sync));
        m.sync_terminals().unwrap();
        assert!(std::fs::read_to_string(dir.path().join("sync/apply.sh"))
            .unwrap()
            .contains("export TEMP_FLAG='on'"));
        assert!(
            m.list(&[]).unwrap().iter().all(|r| !r.pending_sync),
            "nothing left to sync"
        );
        m.remove("TEMP_FLAG", Scope::Process).unwrap();
        m.sync_terminals().unwrap();
        assert!(std::fs::read_to_string(dir.path().join("sync/apply.sh"))
            .unwrap()
            .contains("unset TEMP_FLAG"));

        m.remove("API_URL", Scope::User).unwrap();
        let report = m.sync_terminals().unwrap();
        assert_eq!(report.generation, 4);
        assert!(report.removed >= 1);
        assert!(std::fs::read_to_string(dir.path().join("sync/apply.sh"))
            .unwrap()
            .contains("unset API_URL"));
    }

    #[cfg(unix)]
    #[test]
    fn installed_profile_hook_loads_and_applies() {
        let dir = tempfile::tempdir().unwrap();
        let profile = dir.path().join(".zshrc");
        std::fs::write(&profile, "echo user-line\n").unwrap();
        let m = EnvManager::with_stores(
            BackupStore::new(dir.path().join("backups")),
            EnvSnapshot::from_pairs([("PATH", "/usr/bin:/bin")], false),
            vec![Arc::new(ShellFileStore::new(
                dir.path().join(".bashrc"),
                Syntax::Posix,
            ))],
        )
        .with_dirs(dir.path().join("sync"), dir.path().join("shell"));

        assert!(m
            .install_into(&profile, HookShell::Posix)
            .unwrap()
            .is_some());
        assert!(
            m.install_into(&profile, HookShell::Posix)
                .unwrap()
                .is_none(),
            "idempotent"
        );
        let content = std::fs::read_to_string(&profile).unwrap();
        assert!(content.starts_with("echo user-line\n\n# >>> ENVMANAGER SHELL HOOK >>>"));

        // Sync 1 is live when the shell starts; sync 2 is staged and copied
        // in while the shell runs, as if the user clicked "Sync" meanwhile.
        let sync_dir = dir.path().join("sync");
        let stage = dir.path().join("stage");
        m.sync_terminals().unwrap();
        let copy_dir = |from: &std::path::Path, to: &std::path::Path| {
            std::fs::create_dir_all(to).unwrap();
            for e in std::fs::read_dir(from).unwrap().flatten() {
                std::fs::copy(e.path(), to.join(e.file_name())).unwrap();
            }
        };
        let gen1 = dir.path().join("gen1");
        copy_dir(&sync_dir, &gen1);
        m.set("FRESH", "from sync 2", Scope::User).unwrap();
        m.sync_terminals().unwrap();
        copy_dir(&sync_dir, &stage);

        let script = r#". "$PROFILE_FILE"
printf '%s\n' "${FRESH-unset}"
cp "$STAGE"/* "$SYNC"/
__envmanager_apply
printf '%s\n' "${FRESH-unset}"
"#;
        for (shell, flag) in [("bash", "--norc"), ("zsh", "-f")] {
            let Ok(exe) = which::which(shell) else {
                continue;
            };
            copy_dir(&gen1, &sync_dir); // each shell starts on sync 1
            let out = std::process::Command::new(exe)
                .args([flag, "-c", script])
                .env("PROFILE_FILE", &profile)
                .env("STAGE", &stage)
                .env("SYNC", &sync_dir)
                .env_remove("FRESH")
                .output()
                .unwrap();
            assert_eq!(
                String::from_utf8_lossy(&out.stdout),
                "user-line\nunset\nfrom sync 2\n",
                "{shell}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }

        m.uninstall_hook_from(&profile).unwrap();
        assert_eq!(
            std::fs::read_to_string(&profile).unwrap(),
            "echo user-line\n"
        );
    }
}
