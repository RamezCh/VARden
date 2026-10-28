//! Platform core for VARden.
//!
//! Everything in here is plain Rust with no Tauri dependency, so it can be
//! unit-tested on any machine.
//!
//! Scope is deliberately limited to **user-level** (persistent) and
//! **process-level** (ephemeral) variables. Nothing here ever writes to
//! machine/system scope, so no elevation (UAC / sudo) is ever required.

pub mod backup;
pub mod diagnostics;
pub mod dotenv;
pub mod error;
mod fsutil;
pub mod managed_block;
pub mod manager;
pub mod model;
pub mod path_editor;
pub mod paths;
pub mod profiles;
pub mod secret_store;
pub mod shells;
pub mod status;
pub mod store;
pub mod sync;
pub mod terminal;
pub mod transfer;
pub mod validate;
pub mod watch;

pub use error::{Error, Result};
