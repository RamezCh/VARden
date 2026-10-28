use serde::{Deserialize, Serialize};

/// Where a variable lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Scope {
    /// Persisted for the current user (HKCU\Environment or a shell rc file).
    User,
    /// Only exists inside this app's session; gone when the app closes.
    Process,
}

/// The dashboard status pill.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    /// 🟢 Present in the running environment and in the persistent store, same value.
    ActiveSynced,
    /// 🟡 Persisted, but the running environment doesn't have it (or has an older value).
    PersistentOnly,
    /// 🔵 Set for this session only; not saved anywhere.
    ProcessOnly,
    /// 🔴 Tracked/expected but empty or missing.
    MissingStale,
}

/// How the value is stored. Only meaningful on Windows, where the registry
/// distinguishes plain strings from strings containing `%VAR%` references.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ValueKind {
    #[default]
    String,
    /// REG_EXPAND_SZ: `%USERPROFILE%\bin` is expanded when read by Windows.
    ExpandString,
}

/// A raw persisted entry, as read from a store.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredVar {
    pub key: String,
    pub value: String,
    #[serde(default)]
    pub kind: ValueKind,
}

/// One row of the dashboard.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvVarView {
    pub key: String,
    /// The value the dashboard should show/edit (persisted value if there is one).
    pub value: String,
    /// The value the running app process currently sees, if different.
    pub live_value: Option<String>,
    pub scope: Scope,
    pub status: Status,
    pub kind: ValueKind,
    /// Heuristic: should the UI mask this value by default?
    pub sensitive: bool,
}
