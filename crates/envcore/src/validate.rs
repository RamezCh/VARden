use crate::{Error, Result};

/// Portable variable names: `[A-Za-z_][A-Za-z0-9_]*`.
///
/// Windows technically allows more, but anything outside this set can't be
/// exported from bash/zsh/fish, so we keep one rule everywhere.
pub fn validate_key(key: &str) -> Result<()> {
    let mut chars = key.chars();
    let ok = match chars.next() {
        Some(c) if c == '_' || c.is_ascii_alphabetic() => {
            chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
        }
        _ => false,
    };
    if ok {
        Ok(())
    } else {
        Err(Error::InvalidKey(key.to_string()))
    }
}

pub fn validate_value(key: &str, value: &str) -> Result<()> {
    if value.contains('\0') {
        return Err(Error::InvalidValue {
            key: key.into(),
            reason: "contains a NUL byte".into(),
        });
    }
    // Registry values are capped at 32,767 characters for environment variables.
    if value.encode_utf16().count() > 32_767 {
        return Err(Error::InvalidValue {
            key: key.into(),
            reason: "longer than 32,767 characters".into(),
        });
    }
    Ok(())
}

const SENSITIVE_MARKERS: &[&str] = &[
    "KEY",
    "TOKEN",
    "SECRET",
    "PASSWORD",
    "PASSWD",
    "PASS",
    "CREDENTIAL",
    "AUTH",
    "PRIVATE",
    "SESSION",
    "COOKIE",
    "DSN",
];

/// Should this variable be masked in the UI by default?
pub fn is_sensitive(key: &str, value: &str) -> bool {
    let upper = key.to_ascii_uppercase();
    // Common false positives.
    if matches!(upper.as_str(), "PWD" | "OLDPWD" | "KEYBOARD" | "PATHEXT") {
        return false;
    }
    let by_name = upper.split('_').any(|part| {
        SENSITIVE_MARKERS
            .iter()
            .any(|m| part == *m || part.ends_with(m))
    });
    by_name || has_url_credentials(value)
}

/// `scheme://user:password@host` style connection strings.
fn has_url_credentials(value: &str) -> bool {
    let Some(idx) = value.find("://") else {
        return false;
    };
    let rest = &value[idx + 3..];
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    match authority.rsplit_once('@') {
        Some((userinfo, _)) => userinfo.contains(':'),
        None => false,
    }
}

/// Variables whose value is a list of paths, joined by the platform separator.
const PATH_LISTS: &[&str] = &[
    "PATH",
    "PYTHONPATH",
    "CLASSPATH",
    "PSMODULEPATH",
    "LD_LIBRARY_PATH",
    "DYLD_LIBRARY_PATH",
    "DYLD_FRAMEWORK_PATH",
    "MANPATH",
    "INFOPATH",
    "PKG_CONFIG_PATH",
    "NODE_PATH",
    "PERL5LIB",
    "GOPATH",
    "CMAKE_PREFIX_PATH",
];

/// The result of checking a value for paths that don't exist.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PathCheck {
    /// How many paths were checked (0: the value isn't a path).
    pub checked: usize,
    pub missing: Vec<String>,
}

/// An absolute path on this system. `C:\Tools` is only checked on Windows,
/// and `/usr/bin` only on macOS and Linux: on Windows `/usr/bin` means
/// `\usr\bin` on the current drive, which is how Git Bash and MSYS values
/// look, not a real folder.
fn is_absolute_path(p: &str) -> bool {
    if p.starts_with("~/") || p == "~" {
        return true; // resolved against the home folder
    }
    if cfg!(windows) {
        let b = p.as_bytes();
        b.len() >= 3
            && b[0].is_ascii_alphabetic()
            && b[1] == b':'
            && (b[2] == b'\\' || b[2] == b'/')
    } else {
        p.starts_with('/')
    }
}

/// Network paths can take seconds to time out when unreachable; skip them.
fn is_network_path(p: &str) -> bool {
    p.starts_with("\\\\") || p.starts_with("//")
}

/// Check `value` (already expanded) for paths that don't exist. Path lists
/// are checked entry by entry; other values only when the whole value is an
/// absolute path. URLs, relative paths and network paths are never checked.
pub fn check_paths(key: &str, value: &str, sep: char, home: Option<&std::path::Path>) -> PathCheck {
    let upper = key.to_ascii_uppercase();
    let value = value.trim();
    if value.is_empty() || value.contains("://") || value.contains('\n') {
        return PathCheck::default();
    }
    // Installers sometimes write a single folder with a trailing separator
    // (JetBrains: `IntelliJ IDEA` = `C:\...\bin;`), meant to be spliced
    // into PATH. Any value whose parts are all absolute paths is a list.
    let all_paths = value.contains(sep)
        && value
            .split(sep)
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .all(|p| is_absolute_path(p.trim_matches('"')));
    let is_list = PATH_LISTS.contains(&upper.as_str())
        || (upper.ends_with("PATH") && value.contains(sep))
        || all_paths;
    let entries: Vec<&str> = if is_list {
        value.split(sep).collect()
    } else {
        vec![value]
    };

    let mut check = PathCheck::default();
    for raw in entries {
        let entry = raw.trim().trim_matches('"');
        if entry.is_empty() || !is_absolute_path(entry) || is_network_path(entry) {
            continue;
        }
        let path = match (entry.strip_prefix('~'), home) {
            (Some(rest), Some(h)) => h.join(rest.trim_start_matches(['/', '\\'])),
            (Some(_), None) => continue,
            (None, _) => std::path::PathBuf::from(entry),
        };
        check.checked += 1;
        if !path.exists() {
            check.missing.push(entry.to_string());
        }
    }
    check
}

/// `true` if `value` contains something like `%USERPROFILE%`.
pub fn references_variables(value: &str) -> bool {
    let mut parts = value.split('%');
    parts.next();
    // Between every pair of '%' there must be at least one char.
    let mut inside: Vec<&str> = parts.collect();
    if inside.len() < 2 {
        return false;
    }
    inside.pop(); // text after the last '%'
    inside.iter().step_by(2).any(|name| !name.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys() {
        for k in ["A", "_x", "OPENAI_API_KEY", "a1"] {
            assert!(validate_key(k).is_ok(), "{k}");
        }
        for k in ["", "1A", "A-B", "A B", "A=B", "ÄPFEL", "$X"] {
            assert!(validate_key(k).is_err(), "{k}");
        }
    }

    #[test]
    fn sensitivity() {
        assert!(is_sensitive("OPENAI_API_KEY", "sk-1"));
        assert!(is_sensitive("GITHUB_TOKEN", ""));
        assert!(is_sensitive("DB_PASSWORD", ""));
        assert!(is_sensitive("DATABASE_URL", "postgres://u:p@host/db"));
        assert!(!is_sensitive("DATABASE_URL", "postgres://host/db"));
        assert!(!is_sensitive("PWD", "/home"));
        assert!(!is_sensitive("PATH", "/usr/bin"));
        assert!(!is_sensitive("EDITOR", "vim"));
    }

    #[test]
    fn detects_variable_references() {
        assert!(references_variables("%USERPROFILE%\\bin"));
        assert!(references_variables("a;%X%;b"));
        assert!(!references_variables("50%"));
        assert!(!references_variables("100%%"));
        assert!(!references_variables("plain"));
    }

    #[test]
    fn path_checks() {
        // Lists use the platform's separator: ';' on Windows (where ':' is
        // part of "C:\..."), ':' elsewhere.
        let sep = if cfg!(windows) { ';' } else { ':' };
        let dir = tempfile::tempdir().unwrap();
        let ok = dir.path().join("bin");
        std::fs::create_dir(&ok).unwrap();
        let ok = ok.to_string_lossy().to_string();
        let gone = dir.path().join("gone").to_string_lossy().to_string();
        let home = Some(dir.path());

        let list = format!("{ok}{sep}{gone}{sep}relative/dir{sep}{sep}{ok}");
        assert_eq!(
            check_paths("PATH", &list, sep, home),
            PathCheck {
                checked: 3,
                missing: vec![gone.clone()]
            }
        );
        assert_eq!(
            check_paths("JAVA_HOME", &gone, sep, home).missing,
            vec![gone.clone()]
        );
        assert_eq!(
            check_paths("JAVA_HOME", &ok, sep, home),
            PathCheck {
                checked: 1,
                missing: vec![]
            }
        );
        assert_eq!(
            check_paths("HOME_BIN", "~/bin", sep, home).missing,
            Vec::<String>::new()
        );
        assert_eq!(
            check_paths("HOME_BIN", "~/nope", sep, home).missing,
            vec!["~/nope".to_string()]
        );
        // Not paths: never flagged.
        for v in [
            "postgres://u:p@h/db",
            "code --wait",
            "relative/dir",
            "1",
            "\\\\server\\share\\x",
        ] {
            assert_eq!(check_paths("ANY", v, sep, home).checked, 0, "{v}");
        }
        // A folder with a trailing separator (JetBrains' `IntelliJ IDEA`
        // variable) is the folder, not a missing path ending in ';'.
        assert_eq!(
            check_paths("IntelliJ IDEA", &format!("{ok}{sep}"), sep, home),
            PathCheck {
                checked: 1,
                missing: vec![]
            }
        );
        // A custom *_PATH variable holding a list is split too.
        assert_eq!(
            check_paths("MY_TOOLS_PATH", &format!("{ok}{sep}{gone}"), sep, home).checked,
            2
        );
        // Paths of the other system's kind aren't checked: `Z:\...` only on
        // Windows, `/usr/...` (Git Bash and MSYS style) only elsewhere.
        let windows_style = check_paths("TOOLS", r"Z:\VARden\does\not\exist", ';', home);
        let unix_style = check_paths("TOOLS", "/varden/does/not/exist", ':', home);
        if cfg!(windows) {
            assert_eq!(windows_style.missing.len(), 1);
            assert_eq!(unix_style.checked, 0);
        } else {
            assert_eq!(windows_style.checked, 0);
            assert_eq!(unix_style.missing.len(), 1);
        }
    }
}
