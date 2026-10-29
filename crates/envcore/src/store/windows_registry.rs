//! `HKEY_CURRENT_USER\Environment` — user-level variables on Windows.
//!
//! Writing here never needs elevation. After every change we broadcast
//! `WM_SETTINGCHANGE("Environment")` so Explorer (and anything launched from
//! it afterwards) picks up the new values without logging out.

use std::borrow::Cow;
use std::io;
use std::iter::once;

use std::collections::BTreeMap;

use winreg::enums::{
    HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WRITE, REG_EXPAND_SZ, REG_SZ,
};
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
                ValueKind::String | ValueKind::Expression => REG_SZ,
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
        self.set_many(
            &[(key.to_string(), value.to_string())],
            &format!("Before setting {key}"),
            backups,
        )
    }

    fn update_many(
        &self,
        changes: &[(String, Option<String>)],
        reason: &str,
        backups: &BackupStore,
    ) -> Result<Option<BackupOutcome>> {
        for (key, value) in changes {
            validate_key(key)?;
            if let Some(value) = value {
                validate_value(key, value)?;
            }
        }
        let current = self.list()?;

        // (name, Some(value) to write | None to delete, kind)
        let mut writes: Vec<(String, Option<&str>, ValueKind)> = Vec::new();
        for (key, value) in changes {
            let existing = Self::find(&current, key);
            let Some(value) = value else {
                if let Some(v) = existing {
                    writes.retain(|(n, _, _)| !n.eq_ignore_ascii_case(&v.key));
                    writes.push((v.key.clone(), None, v.kind));
                }
                continue;
            };
            // Keep REG_EXPAND_SZ if the value already was one (PATH usually is);
            // otherwise only use it when the value actually references %VARS%.
            let kind = match existing {
                Some(v) if v.kind == ValueKind::ExpandString => ValueKind::ExpandString,
                _ if references_variables(value) => ValueKind::ExpandString,
                _ => ValueKind::String,
            };
            if existing.is_some_and(|v| v.value == *value && v.kind == kind) {
                continue;
            }
            // Reuse the existing name's casing so we never create `path` next to `Path`.
            let name = existing
                .map(|v| v.key.clone())
                .unwrap_or_else(|| key.clone());
            writes.retain(|(n, _, _)| !n.eq_ignore_ascii_case(&name));
            writes.push((name, Some(value.as_str()), kind));
        }
        if writes.is_empty() {
            return Ok(None);
        }

        let backup = backups.backup_registry(&current, reason)?;
        let reg = Self::open_write()?;
        for (name, value, kind) in writes {
            match value {
                Some(value) => Self::write_value(&reg, &name, value, kind)?,
                None => reg.delete_value(&name).map_err(reg_err)?,
            }
        }
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
/// window and must never block the UI. Calls that arrive while a broadcast is
/// running are folded into one more broadcast afterwards, so a burst of saves
/// never piles up waiting threads.
pub fn broadcast_environment_change() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static PENDING: AtomicBool = AtomicBool::new(false);
    static RUNNING: AtomicBool = AtomicBool::new(false);

    PENDING.store(true, Ordering::SeqCst);
    if RUNNING.swap(true, Ordering::SeqCst) {
        return; // the running thread sends again when it's done
    }
    std::thread::spawn(|| {
        loop {
            while PENDING.swap(false, Ordering::SeqCst) {
                send_setting_change();
            }
            RUNNING.store(false, Ordering::SeqCst);
            // A call may have slipped in after the last send; take it over
            // unless another thread already has.
            if !PENDING.load(Ordering::SeqCst) || RUNNING.swap(true, Ordering::SeqCst) {
                break;
            }
        }
    });
}

fn send_setting_change() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        HWND_BROADCAST, SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_SETTINGCHANGE,
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
}

/// Machine-level variables (read-only; reading HKLM needs no elevation),
/// expanded. Used so that deleting a user variable that shadows a machine
/// variable restores the machine value in synced terminals.
pub fn system_environment() -> BTreeMap<String, String> {
    let Ok(key) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey_with_flags(
        r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment",
        KEY_READ,
    ) else {
        return BTreeMap::new();
    };
    key.enum_values()
        .filter_map(|item| item.ok())
        .filter_map(|(name, raw)| {
            let expand = raw.vtype == REG_EXPAND_SZ;
            if raw.vtype != REG_SZ && !expand {
                return None;
            }
            let value = String::from_reg_value(&raw).ok()?;
            Some((
                name,
                if expand {
                    expand_env_strings(&value)
                } else {
                    value
                },
            ))
        })
        .collect()
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
