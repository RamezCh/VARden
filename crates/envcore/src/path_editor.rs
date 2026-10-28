//! The safe PATH editor.
//!
//! Only your own PATH entries can be changed. The part of PATH that comes
//! from the system is shown read-only:
//!
//! * Windows: your entries are the user `Path` in `HKCU\Environment`. The
//!   system `Path` (HKLM) always comes first and needs an administrator to
//!   change, so it is never written.
//! * macOS/Linux: your entries are written as one line in the managed block
//!   that adds them around the PATH the shell already has
//!   (`export PATH="$HOME/bin:$PATH"`), so the system PATH is never replaced.

use std::path::Path;

use serde::Serialize;

use crate::backup::BackupOutcome;
use crate::managed_block::Syntax;
use crate::manager::EnvManager;
use crate::model::{Scope, ValueKind};
use crate::store::StoreInfo;
use crate::{Error, Result};

/// The variable name the editor manages (Windows spells it `Path`).
pub const PATH_KEY: &str = "PATH";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathEntry {
    /// As written (`%USERPROFILE%\bin`, `$HOME/bin`).
    pub value: String,
    /// The folder it points to, when that differs from `value`.
    pub expanded: Option<String>,
    /// Whether the folder could be checked (absolute, no unknown variables).
    pub checked: bool,
    /// Checked and the folder doesn't exist.
    pub missing: bool,
    /// Another entry earlier in the full PATH points to the same folder.
    pub duplicate: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathInfo {
    pub separator: char,
    /// Your entries, in order. These are the only ones that can change.
    pub entries: Vec<PathEntry>,
    /// Where the system PATH sits among your entries (0: before them).
    pub base_at: usize,
    /// Can your entries go before the system PATH? (Not on Windows.)
    pub base_movable: bool,
    /// The read-only part of PATH.
    pub base: Vec<PathEntry>,
    /// Something the user should know before editing (shown as a warning).
    pub note: Option<String>,
}

fn sep() -> char {
    if cfg!(windows) {
        ';'
    } else {
        ':'
    }
}

// ---------- parsing ----------

/// Your entries and where the inherited PATH goes, read from what a startup
/// file assigns to PATH. `None` when the line is too clever to edit safely.
pub fn parse_expression(syntax: Syntax, raw: &str) -> Option<(Vec<String>, usize)> {
    let tokens: Vec<String> = match syntax {
        Syntax::Posix => {
            let inner = raw
                .trim()
                .strip_prefix('"')
                .and_then(|r| r.strip_suffix('"'))
                .unwrap_or(raw.trim());
            if inner.contains(['"', '\'', '`', '\\']) || inner.contains("$(") {
                return None;
            }
            inner.split(':').map(str::to_string).collect()
        }
        Syntax::Fish => fish_words(raw)?,
        Syntax::Nushell | Syntax::PowerShell => return None,
    };
    let is_base = |t: &str| t == "$PATH" || t == "${PATH}";
    if tokens.iter().filter(|t| is_base(t)).count() != 1 {
        return None;
    }
    let mut entries = Vec::new();
    let mut base_at = 0;
    for t in tokens {
        if is_base(&t) {
            base_at = entries.len();
        } else if !t.is_empty() {
            entries.push(t);
        }
    }
    Some((entries, base_at))
}

/// Split fish arguments: bare words, or words in double quotes.
fn fish_words(raw: &str) -> Option<Vec<String>> {
    let mut out = Vec::new();
    let mut chars = raw.trim().chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
            continue;
        }
        let mut word = String::new();
        if c == '"' {
            chars.next();
            loop {
                match chars.next()? {
                    '"' => break,
                    '\\' => return None,
                    ch => word.push(ch),
                }
            }
            if chars.peek().is_some_and(|c| !c.is_whitespace()) {
                return None;
            }
        } else {
            while let Some(&ch) = chars.peek() {
                if ch.is_whitespace() {
                    break;
                }
                word.push(ch);
                chars.next();
            }
        }
        if word.contains(['"', '\'', '\\', '(', ')']) {
            return None;
        }
        out.push(word);
    }
    Some(out)
}

/// The line that writes your entries around the inherited PATH.
pub fn render_expression(syntax: Syntax, entries: &[String], base_at: usize) -> Result<String> {
    let home = |e: &str| match e.strip_prefix('~') {
        Some(rest) if rest.is_empty() || rest.starts_with('/') => format!("$HOME{rest}"),
        _ => e.to_string(),
    };
    let mut words: Vec<String> = entries.iter().map(|e| home(e)).collect();
    let base_at = base_at.min(words.len());
    match syntax {
        Syntax::Posix => {
            words.insert(base_at, "$PATH".into());
            Ok(format!("export PATH=\"{}\"", words.join(":")))
        }
        Syntax::Fish => {
            let mut quoted: Vec<String> = words.iter().map(|w| format!("\"{w}\"")).collect();
            quoted.insert(base_at, "$PATH".into());
            Ok(format!("set -gx PATH {}", quoted.join(" ")))
        }
        Syntax::Nushell | Syntax::PowerShell => Err(Error::Refused(
            "The PATH editor can't write Nushell or PowerShell startup files yet. Edit PATH there by hand.".into(),
        )),
    }
}

/// Reject entries that would break the PATH line or the PATH itself.
pub fn validate_entries(entries: &[String]) -> Result<()> {
    let bad = |e: &str, reason: &str| Error::InvalidValue {
        key: PATH_KEY.into(),
        reason: format!("{e:?} {reason}"),
    };
    for e in entries {
        if e.trim().is_empty() {
            return Err(bad(e, "is empty"));
        }
        if e.trim() != e {
            return Err(bad(e, "starts or ends with a space"));
        }
        if e.contains(sep()) {
            return Err(bad(
                e,
                &format!("contains {:?}, which separates PATH entries", sep()),
            ));
        }
        if e.contains(['\n', '\r', '\0']) {
            return Err(bad(e, "contains a line break"));
        }
        if !cfg!(windows) && (e.contains(['"', '\'', '`', '\\']) || !simple_variables_only(e)) {
            return Err(bad(
                e,
                "can only use plain variables like $HOME or ${HOME}, without quotes, backslashes or commands",
            ));
        }
    }
    Ok(())
}

/// `$NAME` and `${NAME}` only: the line is written inside double quotes,
/// where anything fancier (`$(…)`, `${x:-…}`, `${x@P}`) could run code.
fn simple_variables_only(entry: &str) -> bool {
    let name_ok = |n: &str| {
        let mut c = n.chars();
        matches!(c.next(), Some(f) if f == '_' || f.is_ascii_alphabetic())
            && c.all(|c| c == '_' || c.is_ascii_alphanumeric())
    };
    let mut rest = entry;
    while let Some(i) = rest.find('$') {
        let after = &rest[i + 1..];
        if let Some(braced) = after.strip_prefix('{') {
            let Some(end) = braced.find('}') else {
                return false;
            };
            if !name_ok(&braced[..end]) {
                return false;
            }
            rest = &braced[end + 1..];
        } else {
            let end = after
                .find(|c: char| !(c == '_' || c.is_ascii_alphanumeric()))
                .unwrap_or(after.len());
            if !name_ok(&after[..end]) {
                return false;
            }
            rest = &after[end..];
        }
    }
    true
}

// ---------- checking ----------

/// Expand `~`, `$HOME`, `$VAR`/`${VAR}` (Unix) or `%VAR%` (Windows).
/// `None` when a variable it uses isn't set.
pub fn expand_entry(entry: &str, lookup: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    if cfg!(windows) {
        return expand_percent(entry, lookup);
    }
    let mut s = entry.to_string();
    if s == "~" || s.starts_with("~/") {
        s = format!("{}{}", lookup("HOME")?, &s[1..]);
    }
    let mut out = String::new();
    let mut rest = s.as_str();
    while let Some(i) = rest.find('$') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        let (name, tail) = if let Some(b) = after.strip_prefix('{') {
            let end = b.find('}')?;
            (&b[..end], &b[end + 1..])
        } else {
            let end = after
                .find(|c: char| !(c == '_' || c.is_ascii_alphanumeric()))
                .unwrap_or(after.len());
            (&after[..end], &after[end..])
        };
        if name.is_empty() {
            return None;
        }
        out.push_str(&lookup(name)?);
        rest = tail;
    }
    out.push_str(rest);
    Some(out)
}

fn expand_percent(entry: &str, lookup: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    let mut out = String::new();
    let mut parts = entry.split('%');
    out.push_str(parts.next().unwrap_or(""));
    let rest: Vec<&str> = parts.collect();
    let mut i = 0;
    while i < rest.len() {
        if i + 1 < rest.len() && !rest[i].is_empty() {
            out.push_str(&lookup(rest[i])?);
            out.push_str(rest[i + 1]);
            i += 2;
        } else {
            out.push('%');
            out.push_str(rest[i]);
            i += 1;
        }
    }
    Some(out)
}

fn normalise(p: &str) -> String {
    let t = p.trim_end_matches(['/', '\\']);
    let t = if t.is_empty() { p } else { t };
    if cfg!(windows) {
        t.replace('/', "\\").to_lowercase()
    } else {
        t.to_string()
    }
}

/// Check each entry. `seen` holds folders already on PATH before these
/// entries (for duplicate detection) and is extended with them.
fn inspect(
    entries: &[String],
    lookup: &dyn Fn(&str) -> Option<String>,
    exists: &dyn Fn(&Path) -> bool,
    seen: &mut Vec<String>,
) -> Vec<PathEntry> {
    entries
        .iter()
        .map(|value| {
            let expanded = expand_entry(value, lookup);
            let target = expanded.clone().unwrap_or_else(|| value.clone());
            // `%IntelliJ IDEA%` expands to `C:\...\bin;`: a variable can
            // splice in one or more folders, each ending in a separator.
            let parts: Vec<&str> = target
                .split(sep())
                .map(str::trim)
                .filter(|p| !p.is_empty())
                .collect();
            let absolute = |p: &str| {
                Path::new(p).is_absolute() && !p.starts_with("\\\\") && !p.starts_with("//")
            };
            let checked =
                expanded.is_some() && !parts.is_empty() && parts.iter().all(|p| absolute(p));
            let mut duplicate = false;
            for p in &parts {
                let key = normalise(p);
                duplicate |= seen.contains(&key);
                seen.push(key);
            }
            let missing = checked && parts.iter().any(|p| !exists(Path::new(p)));
            PathEntry {
                expanded: expanded.filter(|e| e != value),
                checked,
                missing,
                duplicate,
                value: value.clone(),
            }
        })
        .collect()
}

/// Lay out your entries and the system entries in PATH order and check them
/// all, so duplicates are flagged on the later copy.
pub fn inspect_all(
    entries: &[String],
    base_at: usize,
    base: &[String],
    lookup: &dyn Fn(&str) -> Option<String>,
    exists: &dyn Fn(&Path) -> bool,
) -> (Vec<PathEntry>, Vec<PathEntry>) {
    let base_at = base_at.min(entries.len());
    let mut seen = Vec::new();
    let mut mine = inspect(&entries[..base_at], lookup, exists, &mut seen);
    let base = inspect(base, lookup, exists, &mut seen);
    mine.extend(inspect(&entries[base_at..], lookup, exists, &mut seen));
    (mine, base)
}

fn split(value: &str) -> Vec<String> {
    value
        .split(sep())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

impl EnvManager {
    fn lookup(&self) -> impl Fn(&str) -> Option<String> + '_ {
        move |name: &str| {
            if cfg!(windows) {
                return std::env::var(name).ok();
            }
            crate::manager::lock(&self.live)
                .get(name)
                .map(str::to_string)
                .or_else(|| std::env::var(name).ok())
        }
    }

    /// The syntax PATH is written in (Unix), from the first startup file.
    fn path_syntax(&self) -> Option<Syntax> {
        self.stores().iter().find_map(|s| match s.info() {
            StoreInfo::ShellFile { syntax, .. } => Some(syntax),
            StoreInfo::Registry { .. } => None,
        })
    }

    /// Your PATH entries plus the read-only system part, checked.
    pub fn path_info(&self) -> Result<PathInfo> {
        let stored = self
            .persistent()?
            .into_iter()
            .find(|v| v.key.eq_ignore_ascii_case(PATH_KEY) && (self.ci || v.key == PATH_KEY));
        let mut note = None;
        let (entries, base_at) = match (&stored, self.path_syntax()) {
            (None, _) => (Vec::new(), 0),
            (Some(v), _) if cfg!(windows) => (split(&v.value), 0),
            (Some(v), Some(syntax)) if v.kind == ValueKind::Expression => {
                match parse_expression(syntax, &v.value) {
                    Some(parsed) => parsed,
                    None => {
                        note = Some(format!(
                            "Your startup file sets PATH in a way the editor can't read ({}). Saving here replaces that line.",
                            v.value
                        ));
                        (Vec::new(), 0)
                    }
                }
            }
            (Some(v), _) => {
                let e = split(&v.value);
                note = Some(
                    "Your startup file sets PATH to a fixed list, which hides the system PATH. Saving here keeps your list and adds the system PATH after it.".into(),
                );
                let n = e.len();
                (e, n)
            }
        };
        let base = self.base_path(&entries);
        let lookup = self.lookup();
        let (entries, base) = inspect_all(&entries, base_at, &base, &lookup, &|p| p.exists());
        Ok(PathInfo {
            separator: sep(),
            entries,
            base_at,
            base_movable: !cfg!(windows),
            base,
            note,
        })
    }

    /// The read-only part: the system Path on Windows; elsewhere the PATH
    /// this app was started with, minus your own entries.
    fn base_path(&self, mine: &[String]) -> Vec<String> {
        #[cfg(windows)]
        {
            let _ = mine;
            crate::store::windows_registry::system_environment()
                .into_iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(PATH_KEY))
                .map(|(_, v)| split(&v))
                .unwrap_or_default()
        }
        #[cfg(not(windows))]
        {
            let lookup = self.lookup();
            let own: Vec<String> = mine
                .iter()
                .filter_map(|e| expand_entry(e, &lookup))
                .map(|e| normalise(&e))
                .collect();
            let live = crate::manager::lock(&self.live)
                .get(PATH_KEY)
                .map(split)
                .unwrap_or_default();
            let mut out: Vec<String> = Vec::new();
            for e in live {
                let n = normalise(&e);
                if !own.contains(&n) && !out.iter().any(|o| normalise(o) == n) {
                    out.push(e);
                }
            }
            out
        }
    }

    /// Check entries the user is typing, in the order they'd end up in.
    pub fn check_path_entries(&self, entries: &[String], base_at: usize) -> Result<PathInfo> {
        let mut info = self.path_info()?;
        let base: Vec<String> = info.base.iter().map(|e| e.value.clone()).collect();
        let lookup = self.lookup();
        let (mine, base) = inspect_all(entries, base_at, &base, &lookup, &|p| p.exists());
        info.entries = mine;
        info.base = base;
        info.base_at = base_at.min(entries.len());
        Ok(info)
    }

    /// Save your PATH entries. The system part is never written.
    pub fn set_user_path(&self, entries: &[String], base_at: usize) -> Result<Vec<BackupOutcome>> {
        validate_entries(entries)?;
        let reason = "Before editing PATH";
        if cfg!(windows) {
            let value = (!entries.is_empty()).then(|| entries.join(";"));
            return self.apply_changes(&[(PATH_KEY.to_string(), value)], Scope::User, reason);
        }
        // Render every line first, so an unsupported file stops the save
        // before anything is written.
        let mut lines = Vec::new();
        for store in self.stores() {
            let line = match store.info() {
                StoreInfo::ShellFile { syntax, .. } if !entries.is_empty() => {
                    Some(render_expression(syntax, entries, base_at)?)
                }
                _ => None,
            };
            lines.push((store, line));
        }
        let _guard = self.begin_write();
        let mut taken = Vec::new();
        for (store, line) in lines {
            taken.extend(store.set_expression(PATH_KEY, line, reason, &self.backups)?);
        }
        Ok(taken)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lookup(name: &str) -> Option<String> {
        match name {
            "HOME" => Some("/home/me".into()),
            "USERPROFILE" => Some(r"C:\Users\me".into()),
            _ => None,
        }
    }

    #[test]
    fn parses_posix_lines() {
        assert_eq!(
            parse_expression(Syntax::Posix, "\"$HOME/bin:/opt/x/bin:$PATH\""),
            Some((vec!["$HOME/bin".into(), "/opt/x/bin".into()], 2))
        );
        assert_eq!(
            parse_expression(Syntax::Posix, "\"/a:${PATH}:/z\""),
            Some((vec!["/a".into(), "/z".into()], 1))
        );
        // No inherited PATH, two of them, or a command: not editable.
        assert_eq!(parse_expression(Syntax::Posix, "\"/a:/b\""), None);
        assert_eq!(parse_expression(Syntax::Posix, "\"$PATH:/a:$PATH\""), None);
        assert_eq!(
            parse_expression(Syntax::Posix, "\"$(brew --prefix)/bin:$PATH\""),
            None
        );
    }

    #[test]
    fn parses_fish_lines() {
        assert_eq!(
            parse_expression(Syntax::Fish, "\"$HOME/bin\" /opt/bin $PATH"),
            Some((vec!["$HOME/bin".into(), "/opt/bin".into()], 2))
        );
        assert_eq!(
            parse_expression(Syntax::Fish, "(brew --prefix)/bin $PATH"),
            None
        );
    }

    #[test]
    fn render_round_trips() {
        let entries = vec![
            "~/bin".to_string(),
            "/opt/tool bin".to_string(),
            "/z".to_string(),
        ];
        let posix = render_expression(Syntax::Posix, &entries, 2).unwrap();
        assert_eq!(posix, "export PATH=\"$HOME/bin:/opt/tool bin:$PATH:/z\"");
        let fish = render_expression(Syntax::Fish, &entries, 0).unwrap();
        assert_eq!(
            fish,
            "set -gx PATH $PATH \"$HOME/bin\" \"/opt/tool bin\" \"/z\""
        );

        for syntax in [Syntax::Posix, Syntax::Fish] {
            let line = render_expression(syntax, &entries, 1).unwrap();
            let mut block = crate::managed_block::Block::default();
            block.set_expression(syntax, PATH_KEY, line);
            let (_, raw) = block.opaque_assignments().pop().unwrap();
            let (parsed, at) = parse_expression(syntax, &raw).unwrap();
            assert_eq!(parsed, ["$HOME/bin", "/opt/tool bin", "/z"], "{syntax:?}");
            assert_eq!(at, 1);
        }
        assert!(render_expression(Syntax::Nushell, &entries, 0).is_err());
    }

    #[test]
    fn rejects_entries_that_break_path() {
        assert!(validate_entries(&["/ok".into(), "$HOME/ok".into()]).is_ok());
        assert!(validate_entries(&["".into()]).is_err());
        assert!(validate_entries(&[" /a".into()]).is_err());
        assert!(validate_entries(&[format!("/a{}b", sep())]).is_err());
        assert!(validate_entries(&["/a\nb".into()]).is_err());
        if !cfg!(windows) {
            assert!(validate_entries(&["${HOME}/bin".into(), "$HOME/x_$USER".into()]).is_ok());
            for bad in ["$(id)/bin", "${PS1@P}", "${X:-/tmp}", "$", "${}", "$1/bin"] {
                assert!(validate_entries(&[bad.to_string()]).is_err(), "{bad}");
            }
        }
    }

    #[cfg(not(windows))]
    #[test]
    fn expands_unix_entries() {
        assert_eq!(
            expand_entry("~/bin", &lookup).as_deref(),
            Some("/home/me/bin")
        );
        assert_eq!(
            expand_entry("${HOME}/x:", &lookup).as_deref(),
            Some("/home/me/x:")
        );
        assert_eq!(expand_entry("$NOPE/bin", &lookup), None);
        assert_eq!(expand_entry("/plain", &lookup).as_deref(), Some("/plain"));
    }

    #[cfg(windows)]
    #[test]
    fn expands_windows_entries() {
        assert_eq!(
            expand_entry(r"%USERPROFILE%\bin", &lookup).as_deref(),
            Some(r"C:\Users\me\bin")
        );
        assert_eq!(expand_entry(r"%NOPE%\bin", &lookup), None);
        assert_eq!(expand_entry("100%", &lookup).as_deref(), Some("100%"));
    }

    #[test]
    fn flags_missing_and_duplicate_entries() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().to_string_lossy().into_owned();
        let gone = dir.path().join("gone").to_string_lossy().into_owned();
        let mine = vec![real.clone(), gone.clone(), "relative".to_string()];
        let base = vec![real.clone()];
        let (m, b) = inspect_all(&mine, 1, &base, &lookup, &|p| p.exists());
        assert!(m[0].checked && !m[0].missing && !m[0].duplicate);
        assert!(m[1].missing);
        assert!(!m[2].checked && !m[2].missing);
        // The system copy comes after yours, so it is the duplicate.
        assert!(b[0].duplicate);

        // A trailing separator (from a spliced-in variable) isn't part of the folder.
        let spliced = vec![format!("{real}{}", sep())];
        let (m, _) = inspect_all(&spliced, 0, &[], &lookup, &|p| p.exists());
        assert!(m[0].checked && !m[0].missing, "{:?}", m[0]);
    }
}
