//! The managed-block engine.
//!
//! All edits to shell config files happen strictly between these two marker
//! lines; everything outside them is left byte-for-byte untouched:
//!
//! ```bash
//! # >>> VARDEN MANAGED BLOCK >>>
//! export OPENAI_API_KEY='sk-...'
//! # <<< END VARDEN MANAGED BLOCK <<<
//! ```
//!
//! Values are always written *literally* (single-quoted where the shell
//! allows it), so `$`, backticks and `%` are never expanded by accident.
//! Lines inside the block that we didn't write and can't parse literally
//! (e.g. `export PATH="$HOME/bin:$PATH"`) are preserved as opaque lines.

use serde::{Deserialize, Serialize};

use crate::validate::validate_key;

pub const START_MARKER: &str = "# >>> VARDEN MANAGED BLOCK >>>";
pub const END_MARKER: &str = "# <<< END VARDEN MANAGED BLOCK <<<";

/// A pair of marker lines delimiting a section the app owns. Blocks written
/// before the rename to VARden used the `legacy` markers; they are still
/// recognised and are rewritten with the current markers on the next write.
#[derive(Debug, Clone, Copy)]
pub struct Markers {
    pub start: &'static str,
    pub end: &'static str,
    pub legacy: (&'static str, &'static str),
}

/// The variables block.
pub const VARIABLES: Markers = Markers {
    start: START_MARKER,
    end: END_MARKER,
    legacy: (
        "# >>> ENVMANAGER MANAGED BLOCK >>>",
        "# <<< END ENVMANAGER MANAGED BLOCK <<<",
    ),
};

/// The terminal-sync hook, kept separate so it can be installed and removed
/// on its own (and on Windows, where variables aren't written to profiles).
pub const HOOK: Markers = Markers {
    start: "# >>> VARDEN SHELL HOOK >>>",
    end: "# <<< END VARDEN SHELL HOOK <<<",
    legacy: (
        "# >>> ENVMANAGER SHELL HOOK >>>",
        "# <<< END ENVMANAGER SHELL HOOK <<<",
    ),
};
const NOTICE: &str = "# Managed by VARden. Manual edits inside this block may be overwritten.";
const LEGACY_NOTICE: &str =
    "# Managed by Environment Manager. Manual edits inside this block may be overwritten.";

/// Which assignment syntax a config file speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Syntax {
    /// bash, zsh, sh, Git Bash: `export KEY='value'`
    Posix,
    /// fish: `set -gx KEY 'value'`
    Fish,
    /// Nushell: `$env.KEY = "value"`
    Nushell,
    /// PowerShell: `$env:KEY = 'value'`
    PowerShell,
}

/// One line inside the block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockLine {
    /// A literal assignment we fully understand.
    Entry { key: String, value: String },
    /// Something we must keep verbatim. `key` is set when we could at least
    /// tell which variable it assigns, so a later `set` can replace it.
    Opaque {
        key: Option<String>,
        raw_value: Option<String>,
        text: String,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Block {
    pub lines: Vec<BlockLine>,
}

impl Block {
    /// Literal entries, last assignment wins (matching shell semantics).
    pub fn entries(&self) -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = Vec::new();
        for line in &self.lines {
            if let BlockLine::Entry { key, value } = line {
                out.retain(|(k, _)| k != key);
                out.push((key.clone(), value.clone()));
            }
        }
        out
    }

    /// Opaque assignments whose key we recognised, with their raw right-hand side.
    pub fn opaque_assignments(&self) -> Vec<(String, String)> {
        self.lines
            .iter()
            .filter_map(|l| match l {
                BlockLine::Opaque {
                    key: Some(k),
                    raw_value: Some(v),
                    ..
                } => Some((k.clone(), v.clone())),
                _ => None,
            })
            .collect()
    }

    /// Insert or replace `key`, keeping its original position if it existed.
    pub fn set(&mut self, key: &str, value: &str) {
        self.set_line(
            key,
            BlockLine::Entry {
                key: key.to_string(),
                value: value.to_string(),
            },
        );
    }

    /// Write a raw shell expression for `key` (kept verbatim), replacing any
    /// earlier assignment in place.
    pub fn set_expression(&mut self, syntax: Syntax, key: &str, text: String) {
        let line = parse_line(&text, syntax);
        self.set_line(key, line);
    }

    fn set_line(&mut self, key: &str, new: BlockLine) {
        let mut placed = false;
        self.lines.retain_mut(|line| {
            let matches = match line {
                BlockLine::Entry { key: k, .. } => k == key,
                BlockLine::Opaque { key: Some(k), .. } => k == key,
                _ => false,
            };
            if !matches {
                return true;
            }
            if placed {
                false
            } else {
                *line = new.clone();
                placed = true;
                true
            }
        });
        if !placed {
            self.lines.push(new);
        }
    }

    /// Remove every assignment of `key`. Returns whether anything was removed.
    pub fn remove(&mut self, key: &str) -> bool {
        let before = self.lines.len();
        self.lines.retain(|line| match line {
            BlockLine::Entry { key: k, .. } => k != key,
            BlockLine::Opaque { key: Some(k), .. } => k != key,
            _ => true,
        });
        before != self.lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
}

/// Where the block sits in the file, as line indices (inclusive).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Span {
    start: usize,
    end: usize,
}

fn locate(lines: &[&str], m: Markers) -> Result<Option<Span>, String> {
    let is = |i: usize, current: &str, legacy: &str| {
        let t = lines[i].trim();
        t == current || t == legacy
    };
    let starts: Vec<usize> = (0..lines.len())
        .filter(|&i| is(i, m.start, m.legacy.0))
        .collect();
    let ends: Vec<usize> = (0..lines.len())
        .filter(|&i| is(i, m.end, m.legacy.1))
        .collect();
    match (starts.as_slice(), ends.as_slice()) {
        ([], []) => Ok(None),
        ([s], [e]) if s < e => Ok(Some(Span { start: *s, end: *e })),
        ([_], [_]) => Err("end marker appears before start marker".into()),
        ([], _) => Err("end marker without a start marker".into()),
        (_, []) => Err("start marker without an end marker".into()),
        _ => Err("more than one managed block".into()),
    }
}

/// Parse the managed block out of `content` (already `\n`-normalised).
pub fn read_block(content: &str, syntax: Syntax) -> Result<Option<Block>, String> {
    let lines: Vec<&str> = content.lines().collect();
    let Some(span) = locate(&lines, VARIABLES)? else {
        return Ok(None);
    };
    let mut block = Block::default();
    for raw in &lines[span.start + 1..span.end] {
        let t = raw.trim();
        if t.is_empty() || t == NOTICE || t == LEGACY_NOTICE {
            continue;
        }
        block.lines.push(parse_line(raw, syntax));
    }
    Ok(Some(block))
}

/// Return `content` with the managed block replaced by `block`.
///
/// * No block yet → appended at the end, separated by a blank line.
/// * Empty `block` → the block (markers included) is removed.
pub fn write_block(content: &str, syntax: Syntax, block: &Block) -> Result<String, String> {
    replace_section(content, VARIABLES, &render_block(syntax, block))
}

/// Whether `content` contains the section delimited by `m`.
pub fn has_section(content: &str, m: Markers) -> Result<bool, String> {
    let lines: Vec<&str> = content.lines().collect();
    Ok(locate(&lines, m)?.is_some())
}

/// Replace the section delimited by `m` with `rendered` (which includes its
/// own marker lines). An empty `rendered` removes the section. Content
/// outside the section is never touched.
pub fn replace_section(content: &str, m: Markers, rendered: &[String]) -> Result<String, String> {
    let lines: Vec<&str> = content.lines().collect();
    let span = locate(&lines, m)?;
    let rendered = rendered.to_vec();
    let removing = rendered.is_empty();

    let mut out: Vec<String> = Vec::with_capacity(lines.len() + rendered.len() + 1);
    match span {
        Some(span) => {
            out.extend(lines[..span.start].iter().map(|s| s.to_string()));
            out.extend(rendered);
            out.extend(lines[span.end + 1..].iter().map(|s| s.to_string()));
            if removing {
                // Don't leave a dangling blank separator at the end of the file.
                while out.last().is_some_and(|l| l.trim().is_empty()) {
                    out.pop();
                }
            }
        }
        None if removing => return Ok(content.to_string()),
        None => {
            out.extend(lines.iter().map(|s| s.to_string()));
            while out.last().is_some_and(|l| l.trim().is_empty()) {
                out.pop();
            }
            if !out.is_empty() {
                out.push(String::new());
            }
            out.extend(rendered);
        }
    }

    let mut s = out.join("\n");
    if !s.is_empty() {
        s.push('\n');
    }
    Ok(s)
}

fn render_block(syntax: Syntax, block: &Block) -> Vec<String> {
    if block.is_empty() {
        return Vec::new();
    }
    let mut out = vec![START_MARKER.to_string(), NOTICE.to_string()];
    for line in &block.lines {
        out.push(match line {
            BlockLine::Entry { key, value } => render_assignment(syntax, key, value),
            BlockLine::Opaque { text, .. } => text.clone(),
        });
    }
    out.push(END_MARKER.to_string());
    out
}

pub fn render_assignment(syntax: Syntax, key: &str, value: &str) -> String {
    match syntax {
        Syntax::Posix => format!("export {key}='{}'", value.replace('\'', r"'\''")),
        Syntax::Fish => format!(
            "set -gx {key} '{}'",
            value.replace('\\', r"\\").replace('\'', r"\'")
        ),
        Syntax::Nushell => {
            let mut esc = String::with_capacity(value.len());
            for c in value.chars() {
                match c {
                    '\\' => esc.push_str(r"\\"),
                    '"' => esc.push_str("\\\""),
                    '\n' => esc.push_str(r"\n"),
                    '\r' => esc.push_str(r"\r"),
                    '\t' => esc.push_str(r"\t"),
                    c => esc.push(c),
                }
            }
            format!("$env.{key} = \"{esc}\"")
        }
        Syntax::PowerShell => format!("$env:{key} = '{}'", value.replace('\'', "''")),
    }
}

fn opaque(text: &str, key: Option<&str>, raw_value: Option<&str>) -> BlockLine {
    BlockLine::Opaque {
        key: key.filter(|k| validate_key(k).is_ok()).map(str::to_string),
        raw_value: raw_value.map(|v| v.trim().to_string()),
        text: text.to_string(),
    }
}

fn parse_line(raw: &str, syntax: Syntax) -> BlockLine {
    let t = raw.trim();
    if t.starts_with('#') {
        return opaque(raw, None, None);
    }
    let parsed = match syntax {
        Syntax::Posix => parse_posix(t),
        Syntax::Fish => parse_fish(t),
        Syntax::Nushell => parse_nushell(t),
        Syntax::PowerShell => parse_powershell(t),
    };
    match parsed {
        Parsed::Literal(key, value) if validate_key(&key).is_ok() => {
            BlockLine::Entry { key, value }
        }
        Parsed::Literal(key, _) => opaque(raw, Some(&key), None),
        Parsed::KeyOnly(key, rhs) => opaque(raw, Some(&key), Some(&rhs)),
        Parsed::Unknown => opaque(raw, None, None),
    }
}

enum Parsed {
    Literal(String, String),
    /// We know the key but the value uses expansion/escapes we won't evaluate.
    KeyOnly(String, String),
    Unknown,
}

// ---------- POSIX (bash / zsh / sh) ----------

fn parse_posix(t: &str) -> Parsed {
    let rest = t.strip_prefix("export ").map(str::trim_start).unwrap_or(t);
    let Some((key, rhs)) = rest.split_once('=') else {
        return Parsed::Unknown;
    };
    if key.contains(char::is_whitespace) {
        return Parsed::Unknown;
    }
    match posix_unquote(rhs) {
        Some(v) => Parsed::Literal(key.to_string(), v),
        None => Parsed::KeyOnly(key.to_string(), rhs.to_string()),
    }
}

/// Unquote a single POSIX shell word. Returns `None` if the word relies on
/// expansion (`$`, backticks, globbing) or contains more than one word.
fn posix_unquote(word: &str) -> Option<String> {
    let mut out = String::new();
    let mut chars = word.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\'' => loop {
                match chars.next()? {
                    '\'' => break,
                    c => out.push(c),
                }
            },
            '"' => loop {
                match chars.next()? {
                    '"' => break,
                    '$' | '`' => return None,
                    '\\' => match chars.next()? {
                        c @ ('$' | '`' | '"' | '\\') => out.push(c),
                        c => {
                            out.push('\\');
                            out.push(c);
                        }
                    },
                    c => out.push(c),
                }
            },
            '\\' => out.push(chars.next()?),
            '$' | '`' | '*' | '?' | '[' | '~' | ';' | '&' | '|' | '<' | '>' | '(' | ')' => {
                return None
            }
            c if c.is_whitespace() => {
                // Allow a trailing comment, nothing else.
                let rest: String = chars.collect();
                let rest = rest.trim_start();
                return (rest.is_empty() || rest.starts_with('#')).then_some(out);
            }
            c => out.push(c),
        }
    }
    Some(out)
}

// ---------- fish ----------

fn parse_fish(t: &str) -> Parsed {
    let Some(rest) = t.strip_prefix("set ") else {
        return Parsed::Unknown;
    };
    let mut rest = rest.trim_start();
    let mut exported = false;
    while let Some(flag) = rest.strip_prefix('-') {
        let (f, r) = flag.split_once(char::is_whitespace).unwrap_or((flag, ""));
        if f.contains('x') || f == "-export" {
            exported = true;
        }
        rest = r.trim_start();
    }
    let (key, rhs) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
    if !exported || key.is_empty() {
        return Parsed::Unknown;
    }
    match fish_unquote(rhs.trim()) {
        Some(v) => Parsed::Literal(key.to_string(), v),
        None => Parsed::KeyOnly(key.to_string(), rhs.to_string()),
    }
}

fn fish_unquote(word: &str) -> Option<String> {
    let inner = word.strip_prefix('\'')?.strip_suffix('\'')?;
    let mut out = String::new();
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next()? {
                c @ ('\\' | '\'') => out.push(c),
                c => {
                    out.push('\\');
                    out.push(c);
                }
            },
            '\'' => return None, // unescaped quote → multiple words
            c => out.push(c),
        }
    }
    Some(out)
}

// ---------- Nushell ----------

fn parse_nushell(t: &str) -> Parsed {
    let Some(rest) = t.strip_prefix("$env.") else {
        return Parsed::Unknown;
    };
    let Some((key, rhs)) = rest.split_once('=') else {
        return Parsed::Unknown;
    };
    let (key, rhs) = (key.trim(), rhs.trim());
    let lit = if let Some(inner) = rhs.strip_prefix('"').and_then(|r| r.strip_suffix('"')) {
        nu_unescape(inner)
    } else if let Some(inner) = rhs.strip_prefix('\'').and_then(|r| r.strip_suffix('\'')) {
        (!inner.contains('\'')).then(|| inner.to_string())
    } else {
        None
    };
    match lit {
        Some(v) => Parsed::Literal(key.to_string(), v),
        None => Parsed::KeyOnly(key.to_string(), rhs.to_string()),
    }
}

fn nu_unescape(inner: &str) -> Option<String> {
    let mut out = String::new();
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => out.push(match chars.next()? {
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                c @ ('\\' | '"' | '\'' | '/') => c,
                _ => return None,
            }),
            '"' => return None,
            c => out.push(c),
        }
    }
    Some(out)
}

// ---------- PowerShell ----------

fn parse_powershell(t: &str) -> Parsed {
    if t.len() < 5 || !t[..5].eq_ignore_ascii_case("$env:") {
        return Parsed::Unknown;
    }
    let Some((key, rhs)) = t[5..].split_once('=') else {
        return Parsed::Unknown;
    };
    let (key, rhs) = (key.trim(), rhs.trim());
    let lit = if let Some(inner) = rhs.strip_prefix('\'').and_then(|r| r.strip_suffix('\'')) {
        // '' is an escaped quote; a lone ' would end the string early.
        let unescaped = inner.replace("''", "\u{0}");
        (!unescaped.contains('\'')).then(|| unescaped.replace('\u{0}', "'"))
    } else if let Some(inner) = rhs.strip_prefix('"').and_then(|r| r.strip_suffix('"')) {
        (!inner.contains(['$', '`', '"'])).then(|| inner.to_string())
    } else {
        None
    };
    match lit {
        Some(v) => Parsed::Literal(key.to_string(), v),
        None => Parsed::KeyOnly(key.to_string(), rhs.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Syntax; 4] = [
        Syntax::Posix,
        Syntax::Fish,
        Syntax::Nushell,
        Syntax::PowerShell,
    ];
    const NASTY: &[&str] = &[
        "",
        "plain",
        "with space",
        "it's",
        "''",
        r"back\slash\\",
        "$HOME `whoami` %USERPROFILE%",
        "postgres://u:p@h:5432/db?ssl=true#x",
        "\"double\"",
        "semi;colon&amp|pipe",
        "unicode ✓ ünïcødé",
    ];

    #[test]
    fn every_value_roundtrips_in_every_syntax() {
        for syntax in ALL {
            for v in NASTY {
                let line = render_assignment(syntax, "K", v);
                match parse_line(&line, syntax) {
                    BlockLine::Entry { key, value } => {
                        assert_eq!(key, "K");
                        assert_eq!(&value, v, "{syntax:?}: {line}");
                    }
                    other => panic!("{syntax:?} did not parse {line:?}: {other:?}"),
                }
            }
        }
    }

    #[test]
    fn appends_block_and_leaves_rest_untouched() {
        let original = "alias ll='ls -la'\nexport EDITOR=vim\n";
        let mut b = Block::default();
        b.set("OPENAI_API_KEY", "sk-123");
        let out = write_block(original, Syntax::Posix, &b).unwrap();
        assert!(out.starts_with(original));
        assert_eq!(
            out,
            format!("{original}\n{START_MARKER}\n{NOTICE}\nexport OPENAI_API_KEY='sk-123'\n{END_MARKER}\n")
        );
    }

    #[test]
    fn update_is_idempotent_and_in_place() {
        let mut b = Block::default();
        b.set("A", "1");
        b.set("B", "2");
        let first = write_block("before\n", Syntax::Posix, &b).unwrap();
        let first = format!("{first}after\n");

        let mut parsed = read_block(&first, Syntax::Posix).unwrap().unwrap();
        assert_eq!(
            parsed.entries(),
            vec![("A".into(), "1".into()), ("B".into(), "2".into())]
        );
        parsed.set("A", "changed");
        let second = write_block(&first, Syntax::Posix, &parsed).unwrap();
        assert!(second.starts_with("before\n\n"));
        assert!(second.ends_with(&format!("{END_MARKER}\nafter\n")));
        assert!(second.contains("export A='changed'\nexport B='2'"));

        // Writing the same block again changes nothing.
        let again = read_block(&second, Syntax::Posix).unwrap().unwrap();
        assert_eq!(write_block(&second, Syntax::Posix, &again).unwrap(), second);
    }

    #[test]
    fn removing_last_entry_removes_block() {
        let mut b = Block::default();
        b.set("A", "1");
        let with = write_block("keep me\n", Syntax::Posix, &b).unwrap();
        let mut parsed = read_block(&with, Syntax::Posix).unwrap().unwrap();
        assert!(parsed.remove("A"));
        assert_eq!(
            write_block(&with, Syntax::Posix, &parsed).unwrap(),
            "keep me\n"
        );
    }

    #[test]
    fn opaque_lines_survive_and_can_be_overridden() {
        let content = format!(
            "{START_MARKER}\nexport PATH=\"$HOME/bin:$PATH\"\nexport X=1\n# my note\n{END_MARKER}\n"
        );
        let mut b = read_block(&content, Syntax::Posix).unwrap().unwrap();
        assert_eq!(b.entries(), vec![("X".into(), "1".into())]);
        assert_eq!(
            b.opaque_assignments(),
            vec![("PATH".into(), "\"$HOME/bin:$PATH\"".into())]
        );

        b.set("X", "2");
        let out = write_block(&content, Syntax::Posix, &b).unwrap();
        assert!(out.contains("export PATH=\"$HOME/bin:$PATH\"\nexport X='2'\n# my note\n"));

        b.set("PATH", "/only/this");
        let out = write_block(&out, Syntax::Posix, &b).unwrap();
        assert!(out.contains("export PATH='/only/this'"));
        assert!(!out.contains("$HOME/bin"));
    }

    #[test]
    fn malformed_blocks_are_refused() {
        let b = Block::default();
        for bad in [
            format!("{START_MARKER}\nexport A=1\n"),
            format!("export A=1\n{END_MARKER}\n"),
            format!("{END_MARKER}\n{START_MARKER}\n"),
            format!("{START_MARKER}\n{END_MARKER}\n{START_MARKER}\n{END_MARKER}\n"),
        ] {
            assert!(read_block(&bad, Syntax::Posix).is_err(), "{bad}");
            assert!(write_block(&bad, Syntax::Posix, &b).is_err(), "{bad}");
        }
    }

    #[test]
    fn reads_hand_written_double_quotes() {
        let content =
            format!("{START_MARKER}\nexport DATABASE_URL=\"postgres://x\"\n{END_MARKER}\n");
        let b = read_block(&content, Syntax::Posix).unwrap().unwrap();
        assert_eq!(
            b.entries(),
            vec![("DATABASE_URL".into(), "postgres://x".into())]
        );
    }

    #[test]
    fn indented_markers_are_found() {
        let content = format!("  {START_MARKER}\n  export A='1'\n  {END_MARKER}\n");
        let b = read_block(&content, Syntax::Posix).unwrap().unwrap();
        assert_eq!(b.entries(), vec![("A".into(), "1".into())]);
    }

    #[test]
    fn legacy_blocks_are_read_and_upgraded() {
        let legacy = "keep\n\n# >>> ENVMANAGER MANAGED BLOCK >>>\n# Managed by Environment Manager. Manual edits inside this block may be overwritten.\nexport A='1'\n# <<< END ENVMANAGER MANAGED BLOCK <<<\n";
        let mut block = read_block(legacy, Syntax::Posix).unwrap().unwrap();
        assert_eq!(block.entries(), vec![("A".into(), "1".into())]);
        block.set("B", "2");
        let out = write_block(legacy, Syntax::Posix, &block).unwrap();
        assert_eq!(
            out,
            format!("keep\n\n{START_MARKER}\n{NOTICE}\nexport A='1'\nexport B='2'\n{END_MARKER}\n")
        );
        // An old and a new block in one file is ambiguous: refuse to touch it.
        let both = format!("{legacy}{START_MARKER}\n{END_MARKER}\n");
        assert!(read_block(&both, Syntax::Posix).is_err());
    }
}
