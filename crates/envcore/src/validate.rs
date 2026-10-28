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
}
