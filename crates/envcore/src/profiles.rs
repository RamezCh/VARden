//! Profiles: named sets of variables (for example "Client A" and "Client B")
//! that are applied in one step, as user variables or as temporary ones.
//!
//! * One profile is active at a time; applying another turns the first off.
//! * Applying remembers each variable's previous value. Turning the profile
//!   off puts those back, except where the user changed the value since
//!   (their newer edit wins).
//! * An entry with an empty value means "this profile expects the variable":
//!   it isn't written, and while the profile is active the variable shows as
//!   Missing until it has a value.
//!
//! Stored in `profiles.json` in the app folder. Values that look secret
//! (by name, or a connection string with a password) are kept in the system
//! password store instead (see [`crate::secret_store`]); the file then only
//! records that they are there. Where no password store is available they stay
//! in the file.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::backup::BackupOutcome;
use crate::dotenv::EnvEntry;
use crate::manager::EnvManager;
use crate::model::Scope;
use crate::validate::{is_sensitive, validate_key, validate_value};
use crate::{Error, Result, fsutil};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    /// Empty when creating; assigned on save.
    #[serde(default)]
    pub id: String,
    pub name: String,
    pub vars: Vec<EnvEntry>,
}

impl Profile {
    /// The entries that are written when applying (empty ones are only expected).
    fn to_apply(&self) -> Vec<EnvEntry> {
        self.vars
            .iter()
            .filter(|e| !e.value.is_empty())
            .cloned()
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ActiveProfile {
    id: String,
    scope: Scope,
    applied_at: DateTime<Utc>,
    /// What was written.
    applied: Vec<EnvEntry>,
    /// Each applied variable's value before applying (`None`: didn't exist).
    previous: BTreeMap<String, Option<String>>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct ProfilesFile {
    profiles: Vec<Profile>,
    active: Option<ActiveProfile>,
    /// Values kept in the system password store, by account name (see
    /// [`account`]). Their place in this file holds an empty string.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    vaulted: BTreeSet<String>,
}

/// Where in the file a value lives; also its account name in the vault.
enum Slot<'a> {
    Profile(&'a str, &'a str),
    Applied(&'a str),
    Previous(&'a str),
}

fn account(slot: Slot) -> String {
    match slot {
        Slot::Profile(id, key) => format!("profile/{id}/{key}"),
        Slot::Applied(key) => format!("active/applied/{key}"),
        Slot::Previous(key) => format!("active/previous/{key}"),
    }
}

impl ProfilesFile {
    /// Every value in the file with its account name, for moving secrets in
    /// and out of the vault.
    fn values_mut(&mut self) -> Vec<(String, &str, &mut String)> {
        let mut out = Vec::new();
        for p in self.profiles.iter_mut() {
            for e in p.vars.iter_mut() {
                out.push((
                    account(Slot::Profile(&p.id, &e.key)),
                    e.key.as_str(),
                    &mut e.value,
                ));
            }
        }
        if let Some(a) = self.active.as_mut() {
            for e in a.applied.iter_mut() {
                out.push((account(Slot::Applied(&e.key)), e.key.as_str(), &mut e.value));
            }
            for (k, v) in a.previous.iter_mut() {
                if let Some(v) = v.as_mut() {
                    out.push((account(Slot::Previous(k)), k.as_str(), v));
                }
            }
        }
        out
    }
}

/// What the UI sees. Previous values are never sent to the frontend.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveView {
    pub id: String,
    pub name: String,
    pub scope: Scope,
    pub applied_at: DateTime<Utc>,
    /// The profile was edited after it was applied.
    pub outdated: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfilesView {
    pub profiles: Vec<Profile>,
    pub active: Option<ActiveView>,
}

fn slug(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let s = s.trim_matches('-').to_string();
    if s.is_empty() { "profile".into() } else { s }
}

impl EnvManager {
    /// The file as stored: vaulted values are still empty.
    fn read_profiles_file(&self) -> ProfilesFile {
        fs::read(&self.profiles_path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    /// The file with every value filled in, vaulted ones included.
    fn load_profiles(&self) -> ProfilesFile {
        let mut file = self.read_profiles_file();
        if file.vaulted.is_empty() {
            return file;
        }
        let vaulted = std::mem::take(&mut file.vaulted);
        for (acct, key, value) in file.values_mut() {
            if vaulted.contains(&acct) {
                match self.vault.get(&acct) {
                    Some(v) => *value = v,
                    None => log::warn!(
                        "The saved value of {key} is missing from {}.",
                        self.vault.describe()
                    ),
                }
            }
        }
        file
    }

    /// Save the file, moving secret values into the vault and removing vault
    /// entries that are no longer used.
    fn save_profiles(&self, file: &ProfilesFile) -> Result<()> {
        let before = self.read_profiles_file().vaulted;
        let mut out = ProfilesFile {
            profiles: file.profiles.clone(),
            active: file.active.clone(),
            vaulted: BTreeSet::new(),
        };
        let mut vaulted = BTreeSet::new();
        for (acct, key, value) in out.values_mut() {
            if !value.is_empty() && is_sensitive(key, value) && self.vault.set(&acct, value) {
                value.clear();
                vaulted.insert(acct);
            }
        }
        out.vaulted = vaulted;
        fsutil::write_atomic(&self.profiles_path, &serde_json::to_vec_pretty(&out)?)?;
        for acct in before.difference(&out.vaulted) {
            self.vault.delete(acct);
        }
        Ok(())
    }

    /// Move secret values an older version left in `profiles.json` into the
    /// system password store.
    pub fn secure_profile_secrets(&self) {
        // Vaulted values are empty in the file, so anything secret left is plain text.
        let mut raw = self.read_profiles_file();
        let plain = raw
            .values_mut()
            .iter()
            .any(|(_, key, v)| !v.is_empty() && is_sensitive(key, v));
        if plain {
            let file = self.load_profiles();
            if let Err(e) = self.save_profiles(&file) {
                log::warn!("Couldn't move profile secrets to the password store: {e}");
            }
        }
    }

    /// Where secret profile values are kept, for Settings.
    pub fn secret_storage(&self) -> &'static str {
        self.vault.describe()
    }

    /// Keys of the active profile, shown as Missing when they have no value.
    pub(crate) fn active_profile_keys(&self) -> Vec<String> {
        // Names only: no need to read the vault.
        let file = self.read_profiles_file();
        let Some(active) = &file.active else {
            return Vec::new();
        };
        file.profiles
            .iter()
            .find(|p| p.id == active.id)
            .map(|p| p.vars.iter().map(|e| e.key.clone()).collect())
            .unwrap_or_default()
    }

    pub fn profiles(&self) -> ProfilesView {
        let file = self.load_profiles();
        let active = file.active.as_ref().and_then(|a| {
            let p = file.profiles.iter().find(|p| p.id == a.id)?;
            Some(ActiveView {
                id: a.id.clone(),
                name: p.name.clone(),
                scope: a.scope,
                applied_at: a.applied_at,
                outdated: p.to_apply() != a.applied,
            })
        });
        ProfilesView {
            profiles: file.profiles,
            active,
        }
    }

    /// Create (empty `id`) or update a profile.
    pub fn save_profile(&self, mut profile: Profile) -> Result<ProfilesView> {
        profile.name = profile.name.trim().to_string();
        if profile.name.is_empty() {
            return Err(Error::Refused("Give the profile a name.".into()));
        }
        for e in profile.vars.iter_mut() {
            e.key = e.key.trim().to_string();
            validate_key(&e.key)?;
            validate_value(&e.key, &e.value)?;
        }
        let mut seen = std::collections::BTreeSet::new();
        for e in &profile.vars {
            let k = if self.ci {
                e.key.to_ascii_uppercase()
            } else {
                e.key.clone()
            };
            if !seen.insert(k) {
                return Err(Error::Refused(format!(
                    "{} appears twice in this profile.",
                    e.key
                )));
            }
        }

        let mut file = self.load_profiles();
        if file
            .profiles
            .iter()
            .any(|p| p.id != profile.id && p.name.eq_ignore_ascii_case(&profile.name))
        {
            return Err(Error::Refused(format!(
                "A profile named \"{}\" already exists.",
                profile.name
            )));
        }
        if profile.id.is_empty() {
            profile.id = format!(
                "{}-{}",
                slug(&profile.name),
                Utc::now().format("%Y%m%d%H%M%S%3f")
            );
        }
        match file.profiles.iter_mut().find(|p| p.id == profile.id) {
            Some(existing) => *existing = profile,
            None => file.profiles.push(profile),
        }
        self.save_profiles(&file)?;
        Ok(self.profiles())
    }

    pub fn delete_profile(&self, id: &str) -> Result<ProfilesView> {
        let mut file = self.load_profiles();
        if file.active.as_ref().is_some_and(|a| a.id == id) {
            return Err(Error::Refused(
                "Turn the profile off before deleting it.".into(),
            ));
        }
        file.profiles.retain(|p| p.id != id);
        self.save_profiles(&file)?;
        Ok(self.profiles())
    }

    /// Apply a profile. Another active profile is turned off first.
    pub fn apply_profile(&self, id: &str, scope: Scope) -> Result<Vec<BackupOutcome>> {
        let mut taken = self.deactivate_profile()?;
        let mut file = self.load_profiles();
        let profile = file
            .profiles
            .iter()
            .find(|p| p.id == id)
            .cloned()
            .ok_or_else(|| Error::Refused("That profile no longer exists.".into()))?;
        let applied = profile.to_apply();

        let mut previous = BTreeMap::new();
        for e in &applied {
            previous.insert(e.key.clone(), self.current_value(&e.key, scope)?);
        }
        let changes: Vec<(String, Option<String>)> = applied
            .iter()
            .map(|e| (e.key.clone(), Some(e.value.clone())))
            .collect();
        taken.extend(self.apply_changes(
            &changes,
            scope,
            &format!("Before applying profile {}", profile.name),
        )?);

        file.active = Some(ActiveProfile {
            id: profile.id,
            scope,
            applied_at: Utc::now(),
            applied,
            previous,
        });
        self.save_profiles(&file)?;
        Ok(taken)
    }

    /// Turn the active profile off: restore what its variables were before,
    /// leaving any variable the user has changed since.
    pub fn deactivate_profile(&self) -> Result<Vec<BackupOutcome>> {
        let mut file = self.load_profiles();
        let Some(active) = file.active.take() else {
            return Ok(Vec::new());
        };
        let name = file
            .profiles
            .iter()
            .find(|p| p.id == active.id)
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "profile".into());

        let mut changes = Vec::new();
        for e in &active.applied {
            if self.current_value(&e.key, active.scope)?.as_deref() == Some(e.value.as_str()) {
                changes.push((
                    e.key.clone(),
                    active.previous.get(&e.key).cloned().flatten(),
                ));
            }
        }
        let taken = self.apply_changes(
            &changes,
            active.scope,
            &format!("Before turning off profile {name}"),
        )?;
        self.save_profiles(&file)?;
        Ok(taken)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backup::BackupStore;
    use crate::managed_block::Syntax;
    use crate::manager::lock;
    use crate::model::Status;
    use crate::secret_store::MemoryVault;
    use crate::status::EnvSnapshot;
    use crate::store::shell_file::ShellFileStore;
    use std::sync::Arc;

    fn entry(k: &str, v: &str) -> EnvEntry {
        EnvEntry {
            key: k.into(),
            value: v.into(),
        }
    }

    fn setup() -> (tempfile::TempDir, EnvManager) {
        let dir = tempfile::tempdir().unwrap();
        let m = EnvManager::with_stores(
            BackupStore::new(dir.path().join("backups")),
            EnvSnapshot::default(),
            vec![Arc::new(ShellFileStore::new(
                dir.path().join(".bashrc"),
                Syntax::Posix,
            ))],
        );
        (dir, m)
    }

    fn setup_vault(max_len: Option<usize>) -> (tempfile::TempDir, EnvManager, Arc<MemoryVault>) {
        let (dir, m) = setup();
        let vault = Arc::new(MemoryVault {
            max_len,
            ..Default::default()
        });
        (dir, m.with_vault(vault.clone()), vault)
    }

    fn file_text(m: &EnvManager) -> String {
        fs::read_to_string(&m.profiles_path).unwrap()
    }

    #[test]
    fn secret_values_live_in_the_vault_not_the_file() {
        let (_dir, m, vault) = setup_vault(None);
        m.set("OPENAI_API_KEY", "sk-old-one", Scope::User).unwrap();
        let id = m
            .save_profile(Profile {
                id: String::new(),
                name: "Client A".into(),
                vars: vec![
                    entry("OPENAI_API_KEY", "sk-client-a"),
                    entry("DATABASE_URL", "postgres://me:hunter2@db/app"),
                    entry("REGION", "eu-west-1"),
                ],
            })
            .unwrap()
            .profiles[0]
            .id
            .clone();

        let text = file_text(&m);
        assert!(
            !text.contains("sk-client-a") && !text.contains("hunter2"),
            "{text}"
        );
        assert!(
            text.contains("eu-west-1"),
            "ordinary values stay in the file"
        );
        assert_eq!(lock(&vault.entries).len(), 2);

        // The app still sees the real values.
        let p = &m.profiles().profiles[0];
        assert_eq!(p.vars[0].value, "sk-client-a");
        assert_eq!(p.vars[1].value, "postgres://me:hunter2@db/app");

        // Applying keeps the remembered old value out of the file too.
        m.apply_profile(&id, Scope::User).unwrap();
        let text = file_text(&m);
        assert!(
            !text.contains("sk-old-one") && !text.contains("sk-client-a"),
            "{text}"
        );
        assert!(!m.profiles().active.unwrap().outdated);
        m.deactivate_profile().unwrap();
        assert_eq!(
            value(&m, "OPENAI_API_KEY", Scope::User).as_deref(),
            Some("sk-old-one")
        );

        // Entries that are no longer used are removed from the vault.
        m.delete_profile(&id).unwrap();
        assert!(lock(&vault.entries).is_empty());
    }

    #[test]
    fn values_the_vault_refuses_stay_in_the_file() {
        let (_dir, m, vault) = setup_vault(Some(8));
        m.save_profile(Profile {
            id: String::new(),
            name: "Long".into(),
            vars: vec![
                entry("SHORT_TOKEN", "t-123"),
                entry("LONG_TOKEN", "t-1234567890"),
            ],
        })
        .unwrap();
        assert_eq!(lock(&vault.entries).len(), 1);
        assert!(file_text(&m).contains("t-1234567890"));
        assert_eq!(m.profiles().profiles[0].vars[1].value, "t-1234567890");
    }

    #[test]
    fn plain_text_secrets_from_older_versions_are_moved() {
        let (_dir, m) = setup();
        m.save_profile(Profile {
            id: String::new(),
            name: "Old".into(),
            vars: vec![entry("GITHUB_TOKEN", "ghp_abc")],
        })
        .unwrap();
        assert!(
            file_text(&m).contains("ghp_abc"),
            "no vault: stays in the file"
        );

        let vault = Arc::new(MemoryVault::default());
        let m = m.with_vault(vault.clone());
        m.secure_profile_secrets();
        assert!(!file_text(&m).contains("ghp_abc"));
        assert_eq!(m.profiles().profiles[0].vars[0].value, "ghp_abc");
    }

    fn value(m: &EnvManager, k: &str, scope: Scope) -> Option<String> {
        m.current_value(k, scope).unwrap()
    }

    #[test]
    fn apply_and_turn_off_restores_previous_values() {
        let (_dir, m) = setup();
        m.set("API_URL", "https://prod", Scope::User).unwrap();
        let view = m
            .save_profile(Profile {
                id: String::new(),
                name: "Client A".into(),
                vars: vec![
                    entry("API_URL", "https://client-a"),
                    entry("CLIENT_TOKEN", "t-a"),
                    entry("STRIPE_KEY", ""),
                ],
            })
            .unwrap();
        let id = view.profiles[0].id.clone();
        assert!(id.starts_with("client-a-"));

        let backups = m.apply_profile(&id, Scope::User).unwrap();
        assert_eq!(backups.len(), 1, "one backup for the whole profile");
        assert_eq!(
            value(&m, "API_URL", Scope::User).as_deref(),
            Some("https://client-a")
        );
        assert_eq!(
            value(&m, "STRIPE_KEY", Scope::User),
            None,
            "expected-only entries aren't written"
        );

        // The expected variable shows as missing while the profile is active.
        let rows = m.list(&[]).unwrap();
        let stripe = rows.iter().find(|r| r.key == "STRIPE_KEY").unwrap();
        assert_eq!(stripe.status, Status::MissingStale);
        assert_eq!(m.profiles().active.unwrap().name, "Client A");

        // A value the user changes while the profile is active is kept.
        m.set("CLIENT_TOKEN", "edited", Scope::User).unwrap();
        m.deactivate_profile().unwrap();
        assert_eq!(
            value(&m, "API_URL", Scope::User).as_deref(),
            Some("https://prod")
        );
        assert_eq!(
            value(&m, "CLIENT_TOKEN", Scope::User).as_deref(),
            Some("edited")
        );
        assert!(m.profiles().active.is_none());
        assert!(m.list(&[]).unwrap().iter().all(|r| r.key != "STRIPE_KEY"));
    }

    #[test]
    fn temporary_profiles_and_switching() {
        let (_dir, m) = setup();
        let a = m
            .save_profile(Profile {
                id: String::new(),
                name: "A".into(),
                vars: vec![entry("X", "a")],
            })
            .unwrap()
            .profiles[0]
            .id
            .clone();
        let b = m
            .save_profile(Profile {
                id: String::new(),
                name: "B".into(),
                vars: vec![entry("X", "b"), entry("Y", "b")],
            })
            .unwrap()
            .profiles
            .iter()
            .find(|p| p.name == "B")
            .unwrap()
            .id
            .clone();

        assert!(
            m.apply_profile(&a, Scope::Process).unwrap().is_empty(),
            "temporary: nothing saved"
        );
        assert_eq!(value(&m, "X", Scope::Process).as_deref(), Some("a"));
        assert_eq!(value(&m, "X", Scope::User), None);

        m.apply_profile(&b, Scope::Process).unwrap(); // switching turns A off first
        assert_eq!(value(&m, "X", Scope::Process).as_deref(), Some("b"));
        m.deactivate_profile().unwrap();
        assert_eq!(value(&m, "X", Scope::Process), None);
        assert_eq!(value(&m, "Y", Scope::Process), None);
    }

    #[test]
    fn edits_mark_active_profile_outdated_and_rules_are_enforced() {
        let (_dir, m) = setup();
        let mut p = m
            .save_profile(Profile {
                id: String::new(),
                name: "Work".into(),
                vars: vec![entry("A", "1")],
            })
            .unwrap()
            .profiles[0]
            .clone();
        m.apply_profile(&p.id, Scope::Process).unwrap();
        assert!(!m.profiles().active.unwrap().outdated);
        p.vars.push(entry("B", "2"));
        m.save_profile(p.clone()).unwrap();
        assert!(m.profiles().active.unwrap().outdated);

        assert!(matches!(m.delete_profile(&p.id), Err(Error::Refused(_))));
        let dup = Profile {
            id: String::new(),
            name: "work".into(),
            vars: vec![],
        };
        assert!(matches!(m.save_profile(dup), Err(Error::Refused(_))));
        let twice = Profile {
            id: String::new(),
            name: "Other".into(),
            vars: vec![entry("A", "1"), entry("A", "2")],
        };
        assert!(matches!(m.save_profile(twice), Err(Error::Refused(_))));
        let bad = Profile {
            id: String::new(),
            name: "Bad".into(),
            vars: vec![entry("1A", "x")],
        };
        assert!(m.save_profile(bad).is_err());

        m.deactivate_profile().unwrap();
        m.delete_profile(&p.id).unwrap();
        assert!(m.profiles().profiles.is_empty());
    }
}
