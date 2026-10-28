//! `HKEY_CURRENT_USER\Environment` — user-level variables on Windows.
//!
//! Writing here never needs elevation. After every change we broadcast
//! `WM_SETTINGCHANGE("Environment")` so Explorer (and anything launched from
//! it afterwards) picks up the new values without logging out.

use std::borrow::Cow;
use std::io;
use std::iter::once;

use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_EXPAND_SZ, REG_SZ};
use winreg::types::FromRegValue;
use winreg::{RegKey, RegValue};

use crate::backup::{BackupOutcome, BackupStore};
use crate::model::{StoredVar, ValueKind};
use crate::store::{PersistentStore, StoreInfo};
use crate::validate::{references_variables, validate_key, validate_value};
use crate::{Error, Result};

const ENV_SUBKEY: &str = "Environment";

fn reg_err(e: io::Error) -> Error {
    Error::Registry(e.to_string())
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RegistryStore;

impl RegistryStore {
    pub fn new() -> Self {
        RegistryStore
    }

    fn open_read() -> io::Result<RegKey> {
        RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(ENV_SUBKEY, KEY_READ)
    }

    fn open_write() -> Result<RegKey> {
        RegKey::predef(HKEY_CURRENT_USER)
            .create_subkey_with_flags(ENV_SUBKEY, KEY_READ | KEY_WRITE)
            .map(|(key, _)| key)
            .map_err(reg_err)
    }

    fn find<'a>(vars: &'a [StoredVar], key: &str) -> Option<&'a StoredVar> {
        // Windows variable names are case-insensitive.
        vars.iter().find(|v| v.key.eq_ignore_ascii_case(key))
    }

    fn write_value(reg: &RegKey, key: &str, value: &str, kind: ValueKind) -> Result<()> {
        let bytes: Vec<u8> = value
            .encode_utf16()
            .chain(once(0))
            .flat_map(u16::to_le_bytes)
            .collect();
        let raw = RegValue {
            bytes: Cow::Owned(bytes),
            vtype: match kind {
                ValueKind::String => REG_SZ,
                ValueKind::ExpandString => REG_EXPAND_SZ,
            },
        };
        reg.set_raw_value(key, &raw).map_err(reg_err)
    }

    /// Replace the whole of HKCU\Environment with `snapshot` (used by the
    /// rollback UI). The current state is backed up first.
    pub fn restore_snapshot(
        &self,
        snapshot: &[StoredVar],
        backups: &BackupStore,
        reason: &str,
    ) -> Result<BackupOutcome> {
        let current = self.list()?;
        let safety = backups.backup_registry(&current, reason)?;
        let reg = Self::open_write()?;
        for var in &current {
            if Self::find(snapshot, &var.key).is_none() {
                reg.delete_value(&var.key).map_err(reg_err)?;
            }
        }
        for var in snapshot {
            Self::write_value(&reg, &var.key, &var.value, var.kind)?;
        }
        broadcast_environment_change();
        Ok(safety)
    }
}

impl PersistentStore for RegistryStore {
    fn info(&self) -> StoreInfo {
        StoreInfo::Registry {
            location: r"HKEY_CURRENT_USER\Environment".into(),
        }
    }

    fn list(&self) -> Result<Vec<StoredVar>> {
        let reg = match Self::open_read() {
            Ok(k) => k,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(reg_err(e)),
        };
        let mut out = Vec::new();
        for item in reg.enum_values() {
            let (name, raw) = item.map_err(reg_err)?;
            let kind = if raw.vtype == REG_SZ {
                ValueKind::String
            } else if raw.vtype == REG_EXPAND_SZ {
                ValueKind::ExpandString
            } else {
                continue; // DWORDs/binary values aren't environment variables
            };
            let value = String::from_reg_value(&raw).map_err(reg_err)?;
            out.push(StoredVar {
                key: name,
                value,
                kind,
            });
        }
        out.sort_by_key(|v| v.key.to_ascii_uppercase());
        Ok(out)
    }

    fn set(&self, key: &str, value: &str, backups: &BackupStore) -> Result<Option<BackupOutcome>> {
        validate_key(key)?;
        validate_value(key, value)?;
        let current = self.list()?;
        let existing = Self::find(&current, key);

        // Keep REG_EXPAND_SZ if the value already was one (PATH usually is);
        // otherwise only use it when the value actually references %VARS%.
        let kind = match existing {
            Some(v) if v.kind == ValueKind::ExpandString => ValueKind::ExpandString,
            _ if references_variables(value) => ValueKind::ExpandString,
            _ => ValueKind::String,
        };
        if existing.is_some_and(|v| v.value == value && v.kind == kind) {
            return Ok(None);
        }
        // Reuse the existing name's casing so we never create `path` next to `Path`.
        let name = existing.map(|v| v.key.as_str()).unwrap_or(key);

        let backup = backups.backup_registry(&current, &format!("Before setting {key}"))?;
        Self::write_value(&Self::open_write()?, name, value, kind)?;
        broadcast_environment_change();
        Ok(Some(backup))
    }

    fn remove(&self, key: &str, backups: &BackupStore) -> Result<Option<BackupOutcome>> {
        let current = self.list()?;
        let Some(existing) = Self::find(&current, key) else {
            return Ok(None);
        };
        let backup = backups.backup_registry(&current, &format!("Before removing {key}"))?;
        Self::open_write()?
            .delete_value(&existing.key)
            .map_err(reg_err)?;
        broadcast_environment_change();
        Ok(Some(backup))
    }
}

/// Tell running programs (Explorer in particular) that the user environment
/// changed. Runs on a background thread: a broadcast waits on every top-level
/// window and must never block the UI.
pub fn broadcast_environment_change() {
    std::thread::spawn(|| {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE,
        };
        let area: Vec<u16> = "Environment".encode_utf16().chain(once(0)).collect();
        let mut result: usize = 0;
        // SAFETY: `area` is a NUL-terminated UTF-16 string that outlives the call.
        unsafe {
            SendMessageTimeoutW(
                HWND_BROADCAST,
                WM_SETTINGCHANGE,
                0,
                area.as_ptr() as isize,
                SMTO_ABORTIFHUNG,
                5000,
                &mut result,
            );
        }
    });
}

/// Expand `%VARS%` using this process's environment (like Windows does when
/// it loads REG_EXPAND_SZ values).
pub fn expand_env_strings(value: &str) -> String {
    use windows_sys::Win32::System::Environment::ExpandEnvironmentStringsW;
    if !value.contains('%') {
        return value.to_string();
    }
    let src: Vec<u16> = value.encode_utf16().chain(once(0)).collect();
    // SAFETY: `src` is NUL-terminated; `buf` is sized from the first call.
    unsafe {
        let needed = ExpandEnvironmentStringsW(src.as_ptr(), std::ptr::null_mut(), 0);
        if needed == 0 {
            return value.to_string();
        }
        let mut buf = vec![0u16; needed as usize];
        let written = ExpandEnvironmentStringsW(src.as_ptr(), buf.as_mut_ptr(), needed);
        if written == 0 || written > needed {
            return value.to_string();
        }
        String::from_utf16_lossy(&buf[..written as usize - 1])
    }
}
