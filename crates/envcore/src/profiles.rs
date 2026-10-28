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
//! Stored in `profiles.json` in the app folder. Like the registry and the
//! startup files themselves, values are plain text.

use std::collections::BTreeMap;
use std::fs;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::backup::BackupOutcome;
use crate::dotenv::EnvEntry;
use crate::manager::EnvManager;
use crate::model::Scope;
use crate::validate::{validate_key, validate_value};
use crate::{fsutil, Error, Result};

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
    if s.is_empty() {
        "profile".into()
    } else {
        s
    }
}

impl EnvManager {
    fn load_profiles(&self) -> ProfilesFile {
        fs::read(&self.profiles_path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    fn save_profiles(&self, file: &ProfilesFile) -> Result<()> {
        fsutil::write_atomic(&self.profiles_path, &serde_json::to_vec_pretty(file)?)
    }

    /// Keys of the active profile, shown as Missing when they have no value.
    pub(crate) fn active_profile_keys(&self) -> Vec<String> {
        let file = self.load_profiles();
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
    use crate::model::Status;
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
