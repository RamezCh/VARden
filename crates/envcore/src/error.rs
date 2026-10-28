use std::path::PathBuf;

use serde::{Serialize, Serializer};

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error on {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("registry error: {0}")]
    Registry(String),

    #[error("invalid variable name {0:?}: use letters, digits and underscores, not starting with a digit")]
    InvalidKey(String),

    #[error("invalid value for {key}: {reason}")]
    InvalidValue { key: String, reason: String },

    #[error("the managed block in {path} is malformed ({reason}); fix it by hand or restore a backup before editing")]
    MalformedBlock { path: PathBuf, reason: String },

    #[error("backup {0} not found")]
    BackupNotFound(String),

    #[error("{0} is not supported on this platform")]
    Unsupported(&'static str),

    /// An action that isn't allowed right now; the message says why.
    #[error("{0}")]
    Refused(String),

    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

impl Error {
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Error::Io {
            path: path.into(),
            source,
        }
    }
}

/// Errors cross the Tauri IPC boundary as plain strings so the frontend can
/// show them directly.
impl Serialize for Error {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}
