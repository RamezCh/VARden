//! Where profiles keep their secret values.
//!
//! Values whose names or contents look secret (API keys, tokens, passwords,
//! connection strings) are kept in the operating system's own password store
//! instead of in `profiles.json`:
//!
//! * Windows: Credential Manager
//! * macOS: the login keychain
//! * Linux: the Secret Service (GNOME Keyring, KWallet)
//!
//! When that store isn't there or refuses a value (a Linux desktop without a
//! keyring, or a value too long for Credential Manager), the value stays in
//! `profiles.json` as before, so nothing is ever lost.

use std::collections::BTreeMap;
use std::sync::Mutex;

/// Service name the entries are filed under in the system password store.
pub const SERVICE: &str = "io.github.ramezch.varden";

pub trait SecretVault: Send + Sync {
    /// Store a value. `false` when the vault isn't available or refused it.
    fn set(&self, account: &str, value: &str) -> bool;
    /// `None` when there is no such entry or the vault can't be read.
    fn get(&self, account: &str) -> Option<String>;
    fn delete(&self, account: &str);
    /// Shown in Settings.
    fn describe(&self) -> &'static str;
}

/// No vault: secret values stay in the file.
pub struct NoVault;

impl SecretVault for NoVault {
    fn set(&self, _: &str, _: &str) -> bool {
        false
    }
    fn get(&self, _: &str) -> Option<String> {
        None
    }
    fn delete(&self, _: &str) {}
    fn describe(&self) -> &'static str {
        "the profiles file"
    }
}

/// The system password store, through the `keyring` crate.
pub struct OsKeychain;

impl SecretVault for OsKeychain {
    fn set(&self, account: &str, value: &str) -> bool {
        match keyring::Entry::new(SERVICE, account).and_then(|e| e.set_password(value)) {
            Ok(()) => true,
            Err(e) => {
                log::warn!("Couldn't save a profile secret to the system password store: {e}");
                false
            }
        }
    }

    fn get(&self, account: &str) -> Option<String> {
        match keyring::Entry::new(SERVICE, account).and_then(|e| e.get_password()) {
            Ok(v) => Some(v),
            Err(keyring::Error::NoEntry) => None,
            Err(e) => {
                log::warn!("Couldn't read a profile secret from the system password store: {e}");
                None
            }
        }
    }

    fn delete(&self, account: &str) {
        match keyring::Entry::new(SERVICE, account).and_then(|e| e.delete_credential()) {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(e) => {
                log::warn!("Couldn't remove a profile secret from the system password store: {e}")
            }
        }
    }

    fn describe(&self) -> &'static str {
        if cfg!(windows) {
            "Windows Credential Manager"
        } else if cfg!(target_os = "macos") {
            "the macOS keychain"
        } else {
            "the system keyring"
        }
    }
}

/// In memory, for tests.
#[derive(Default)]
pub struct MemoryVault {
    pub entries: Mutex<BTreeMap<String, String>>,
    /// Refuse values longer than this (like Credential Manager's limit).
    pub max_len: Option<usize>,
}

impl SecretVault for MemoryVault {
    fn set(&self, account: &str, value: &str) -> bool {
        if self.max_len.is_some_and(|m| value.len() > m) {
            return false;
        }
        crate::manager::lock(&self.entries).insert(account.into(), value.into());
        true
    }
    fn get(&self, account: &str) -> Option<String> {
        crate::manager::lock(&self.entries).get(account).cloned()
    }
    fn delete(&self, account: &str) {
        crate::manager::lock(&self.entries).remove(account);
    }
    fn describe(&self) -> &'static str {
        "memory"
    }
}
