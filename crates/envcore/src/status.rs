//! Computes the dashboard status pill for every variable.
//!
//! Two sources are compared:
//! * **persistent** — what the user-level store holds (registry / rc block);
//! * **current** — what this app's process sees: the environment it was
//!   launched with, overlaid with process-level variables set in the app.

use std::collections::{BTreeMap, HashMap};

use crate::model::{EnvVarView, Scope, Status, StoredVar, ValueKind};
use crate::validate::is_sensitive;

/// A case-aware snapshot of an environment.
#[derive(Debug, Clone, Default)]
pub struct EnvSnapshot {
    case_insensitive: bool,
    vars: HashMap<String, (String, String)>, // normalised key -> (key, value)
}

impl EnvSnapshot {
    pub fn from_pairs<I, K, V>(pairs: I, case_insensitive: bool) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        let mut snap = EnvSnapshot {
            case_insensitive,
            vars: HashMap::new(),
        };
        for (k, v) in pairs {
            let k = k.into();
            snap.vars.insert(snap.norm(&k), (k, v.into()));
        }
        snap
    }

    /// The environment this process was started with.
    pub fn capture() -> Self {
        Self::from_pairs(
            std::env::vars_os().map(|(k, v)| {
                (
                    k.to_string_lossy().into_owned(),
                    v.to_string_lossy().into_owned(),
                )
            }),
            cfg!(windows),
        )
    }

    fn norm(&self, key: &str) -> String {
        if self.case_insensitive {
            key.to_ascii_uppercase()
        } else {
            key.to_string()
        }
    }

    pub fn set(&mut self, key: &str, value: impl Into<String>) {
        let n = self.norm(key);
        // Keep the existing spelling (e.g. `Path`) when updating.
        let name = self
            .vars
            .get(&n)
            .map(|(k, _)| k.clone())
            .unwrap_or_else(|| key.to_string());
        self.vars.insert(n, (name, value.into()));
    }

    pub fn remove(&mut self, key: &str) {
        let n = self.norm(key);
        self.vars.remove(&n);
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.vars.get(&self.norm(key)).map(|(_, v)| v.as_str())
    }

    pub fn is_case_insensitive(&self) -> bool {
        self.case_insensitive
    }
}

/// How to turn a stored value into what a freshly started process would see.
/// Returns `None` when that can't be known (e.g. a shell expression like
/// `"$HOME/bin:$PATH"`), in which case only presence is compared.
pub type Resolver<'a> = &'a dyn Fn(&StoredVar) -> Option<String>;

/// Default resolver: literal values resolve to themselves, expandable ones
/// are unknown.
pub fn literal_resolver(v: &StoredVar) -> Option<String> {
    match v.kind {
        ValueKind::String => Some(v.value.clone()),
        ValueKind::ExpandString => None,
    }
}

pub struct StatusInput<'a> {
    pub persistent: &'a [StoredVar],
    /// Temporary variables set inside the app this session.
    pub overlay: &'a BTreeMap<String, String>,
    pub live: &'a EnvSnapshot,
    /// Keys a profile expects to exist; shown as Missing if absent.
    pub tracked: &'a [String],
    pub path_separator: char,
}

pub fn compute(input: &StatusInput<'_>, resolve: Resolver<'_>) -> Vec<EnvVarView> {
    let ci = input.live.is_case_insensitive();
    let norm = |k: &str| {
        if ci {
            k.to_ascii_uppercase()
        } else {
            k.to_string()
        }
    };
    let current = |key: &str| input.live.get(key);

    // A saved and a temporary variable may share a name; each gets a row.
    let mut rows: BTreeMap<(String, u8), EnvVarView> = BTreeMap::new();

    for var in input.persistent {
        let cur = current(&var.key);
        let status = if var.value.trim().is_empty() {
            Status::MissingStale
        } else {
            match (cur, resolve(var)) {
                (None, _) => Status::PersistentOnly,
                (Some(_), None) => Status::ActiveSynced, // can't evaluate; present is the best we know
                (Some(c), Some(expected))
                    if values_match(&var.key, &expected, c, input.path_separator, ci) =>
                {
                    Status::ActiveSynced
                }
                (Some(_), Some(_)) => Status::PersistentOnly,
            }
        };
        rows.insert(
            (norm(&var.key), 0),
            EnvVarView {
                key: var.key.clone(),
                live_value: cur.filter(|c| *c != var.value).map(str::to_string),
                sensitive: is_sensitive(&var.key, &var.value),
                value: var.value.clone(),
                scope: Scope::User,
                // Shell expressions can't be pushed, so they're never pending.
                pending_sync: status == Status::PersistentOnly && resolve(var).is_some(),
                status,
                kind: var.kind,
            },
        );
    }

    for (key, value) in input.overlay {
        rows.insert(
            (norm(key), 1),
            EnvVarView {
                key: key.clone(),
                value: value.clone(),
                live_value: None,
                scope: Scope::Process,
                status: if value.is_empty() {
                    Status::MissingStale
                } else {
                    Status::ProcessOnly
                },
                kind: ValueKind::String,
                sensitive: is_sensitive(key, value),
                // Open terminals get it at the next sync.
                pending_sync: current(key) != Some(value.as_str()),
            },
        );
    }

    for key in input.tracked {
        if rows.contains_key(&(norm(key), 0)) {
            continue;
        }
        rows.entry((norm(key), 1)).or_insert_with(|| {
            let cur = current(key);
            EnvVarView {
                key: key.clone(),
                value: cur.unwrap_or_default().to_string(),
                live_value: None,
                scope: Scope::Process,
                status: match cur {
                    Some(v) if !v.is_empty() => Status::ProcessOnly,
                    _ => Status::MissingStale,
                },
                kind: ValueKind::String,
                sensitive: is_sensitive(key, cur.unwrap_or_default()),
                pending_sync: false,
            }
        });
    }

    let mut out: Vec<EnvVarView> = rows.into_values().collect();
    out.sort_by_key(|r| (r.key.to_ascii_lowercase(), r.scope == Scope::Process));
    out
}

/// Equal, or — for PATH-like variables — every persisted entry is present in
/// the current value. (On Windows the process PATH is the system PATH plus
/// the user PATH, so it never equals the user value exactly.)
fn values_match(key: &str, expected: &str, current: &str, sep: char, ci: bool) -> bool {
    if expected == current {
        return true;
    }
    if !key.eq_ignore_ascii_case("PATH") {
        return false;
    }
    let clean = |s: &str| {
        let s = s.trim().trim_end_matches(['/', '\\']).to_string();
        if ci {
            s.to_ascii_lowercase()
        } else {
            s
        }
    };
    let have: Vec<String> = current.split(sep).map(clean).collect();
    expected
        .split(sep)
        .map(clean)
        .filter(|s| !s.is_empty())
        .all(|e| have.contains(&e))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn var(k: &str, v: &str) -> StoredVar {
        StoredVar {
            key: k.into(),
            value: v.into(),
            kind: ValueKind::String,
        }
    }

    fn run(
        persistent: &[StoredVar],
        overlay: &[(&str, &str)],
        live: &[(&str, &str)],
        ci: bool,
    ) -> Vec<EnvVarView> {
        let overlay: BTreeMap<String, String> = overlay
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        let live = EnvSnapshot::from_pairs(live.iter().copied(), ci);
        let tracked = vec!["EXPECTED".to_string()];
        compute(
            &StatusInput {
                persistent,
                overlay: &overlay,
                live: &live,
                tracked: &tracked,
                path_separator: if ci { ';' } else { ':' },
            },
            &literal_resolver,
        )
    }

    fn status_of(rows: &[EnvVarView], key: &str) -> Status {
        rows.iter().find(|r| r.key == key).unwrap().status
    }

    #[test]
    fn four_states() {
        let rows = run(
            &[
                var("SYNCED", "1"),
                var("PENDING", "new"),
                var("NOT_LOADED", "x"),
                var("EMPTY", ""),
            ],
            &[("SESSION_ONLY", "s")],
            &[("SYNCED", "1"), ("PENDING", "old"), ("UNRELATED", "z")],
            false,
        );
        assert_eq!(status_of(&rows, "SYNCED"), Status::ActiveSynced);
        assert_eq!(status_of(&rows, "PENDING"), Status::PersistentOnly);
        assert_eq!(status_of(&rows, "NOT_LOADED"), Status::PersistentOnly);
        assert_eq!(status_of(&rows, "EMPTY"), Status::MissingStale);
        assert_eq!(status_of(&rows, "SESSION_ONLY"), Status::ProcessOnly);
        assert_eq!(status_of(&rows, "EXPECTED"), Status::MissingStale);
        assert!(
            rows.iter().all(|r| r.key != "UNRELATED"),
            "inherited vars aren't listed"
        );
        let pending = rows.iter().find(|r| r.key == "PENDING").unwrap();
        assert_eq!(pending.live_value.as_deref(), Some("old"));
    }

    #[test]
    fn temporary_and_saved_with_same_name_get_separate_rows() {
        let rows = run(
            &[var("A", "saved")],
            &[("A", "temp")],
            &[("A", "saved")],
            false,
        );
        let a: Vec<(Scope, Status, bool)> = rows
            .iter()
            .filter(|r| r.key == "A")
            .map(|r| (r.scope, r.status, r.pending_sync))
            .collect();
        assert_eq!(
            a,
            vec![
                (Scope::User, Status::ActiveSynced, false),
                (Scope::Process, Status::ProcessOnly, true),
            ]
        );
    }

    #[test]
    fn windows_path_is_subset_and_case_insensitive() {
        let rows = run(
            &[var("Path", r"C:\Users\me\bin\;C:\Tools")],
            &[],
            &[("PATH", r"C:\Windows;c:\users\me\bin;C:\Tools")],
            true,
        );
        assert_eq!(status_of(&rows, "Path"), Status::ActiveSynced);
    }
}
