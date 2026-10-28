//! Platform core for Environment Manager.
//!
//! Everything in here is plain Rust with no Tauri dependency, so it can be
//! unit-tested on any machine and reused by a future CLI (`envmanager hook`).
//!
//! Scope is deliberately limited to **user-level** (persistent) and
//! **process-level** (ephemeral) variables. Nothing here ever writes to
//! machine/system scope, so no elevation (UAC / sudo) is ever required.

pub mod backup;
pub mod error;
mod fsutil;
pub mod managed_block;
pub mod manager;
pub mod model;
pub mod paths;
pub mod shells;
pub mod status;
pub mod store;
pub mod validate;

pub use error::{Error, Result};
