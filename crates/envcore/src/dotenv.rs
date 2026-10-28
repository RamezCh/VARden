//! Reading and writing `.env` files.
//!
//! Parsing follows what common dotenv loaders accept:
//! `KEY=value`, an optional `export ` prefix, `#` comments, single-quoted
//! (literal), double-quoted (with `\n`, `\t`, `\"`, `\\` escapes, may span
//! lines) and backtick-quoted values, and inline `# comments` after
//! unquoted values. `${VAR}` references are kept literally, not expanded.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::validate::validate_key;
use crate::{fsutil, Error, Result};

/// `.env` files larger than this are almost certainly not `.env` files.
const MAX_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvEntry {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedLine {
    /// 1-based line number.
    pub line: usize,
    pub text: String,
    pub reason: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedEnv {
    pub entries: Vec<EnvEntry>,
    pub skipped: Vec<SkippedLine>,
}

pub fn read_file(path: &Path) -> Result<ParsedEnv> {
    let meta = std::fs::metadata(path).map_err(|e| Error::io(path, e))?;
    if meta.len() > MAX_BYTES {
        return Err(Error::InvalidValue {
            key: path.display().to_string(),
            reason: "file is larger than 1 MB; is it really a .env file?".into(),
        });
    }
    let bytes = std::fs::read(path).map_err(|e| Error::io(path, e))?;
    let text = String::from_utf8(bytes).map_err(|_| Error::InvalidValue {
        key: path.display().to_string(),
        reason: "file is not UTF-8 text".into(),
    })?;
    Ok(parse(&text))
}

pub fn write_file(path: &Path, entries: &[EnvEntry]) -> Result<()> {
    fsutil::write_atomic(path, render(entries).as_bytes())
}

pub fn parse(input: &str) -> ParsedEnv {
    let text = input
        .strip_prefix('\u{feff}')
        .unwrap_or(input)
        .replace("\r\n", "\n");
    let lines: Vec<&str> = text.split('\n').collect();
    let mut out = ParsedEnv::default();
    let mut i = 0;

    while i < lines.len() {
        let line_no = i + 1;
        let raw = lines[i];
        i += 1;
        let t = raw.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let skip = |reason: &str| SkippedLine {
            line: line_no,
            text: raw.to_string(),
            reason: reason.to_string(),
        };

        let body = t.strip_prefix("export ").map(str::trim_start).unwrap_or(t);
        let Some((key, rest)) = body.split_once('=') else {
            out.skipped.push(skip("not a KEY=value line"));
            continue;
        };
        let key = key.trim();
        if validate_key(key).is_err() {
            out.skipped.push(skip("invalid variable name"));
            continue;
        }
        let rest = rest.trim_start();

        let value = match rest.chars().next() {
            Some(q @ ('"' | '\'' | '`')) => {
                // Quoted values may continue on following lines.
                let mut buf = rest[1..].to_string();
                let resume_at = i;
                let parsed = loop {
                    if let Some(v) = take_quoted(&buf, q) {
                        break Some(v);
                    }
                    if i >= lines.len() {
                        break None;
                    }
                    buf.push('\n');
                    buf.push_str(lines[i]);
                    i += 1;
                };
                match parsed {
                    Some((v, trailing)) if trailing.is_empty() || trailing.starts_with('#') => v,
                    Some(_) => {
                        out.skipped
                            .push(skip("unexpected text after the closing quote"));
                        continue;
                    }
                    None => {
                        // Report this line only and carry on with the next.
                        out.skipped.push(skip("quote is never closed"));
                        i = resume_at;
                        continue;
                    }
                }
            }
            _ => {
                // Unquoted: an inline comment needs whitespace before '#'.
                let end = rest
                    .char_indices()
                    .find(|&(idx, c)| c == '#' && rest[..idx].ends_with([' ', '\t']))
                    .map(|(idx, _)| idx)
                    .unwrap_or(rest.len());
                rest[..end].trim_end().to_string()
            }
        };

        out.entries.retain(|e| e.key != key);
        out.entries.push(EnvEntry {
            key: key.to_string(),
            value,
        });
    }
    out
}

/// Find the closing quote in `s` (text after the opening quote). Returns the
/// unescaped value and the trimmed text after the closing quote.
fn take_quoted(s: &str, quote: char) -> Option<(String, &str)> {
    let mut value = String::new();
    let mut chars = s.char_indices();
    while let Some((idx, c)) = chars.next() {
        if c == quote {
            return Some((value, s[idx + c.len_utf8()..].trim()));
        }
        if c == '\\' && quote == '"' {
            let (_, next) = chars.next()?;
            match next {
                'n' => value.push('\n'),
                'r' => value.push('\r'),
                't' => value.push('\t'),
                '"' | '\\' | '$' => value.push(next),
                other => {
                    value.push('\\');
                    value.push(other);
                }
            }
            continue;
        }
        value.push(c);
    }
    None
}

fn is_plain(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_-./:@%+,=~".contains(c))
}

pub fn render_entry(e: &EnvEntry) -> String {
    let v = &e.value;
    if v.is_empty() {
        format!("{}=", e.key)
    } else if is_plain(v) {
        format!("{}={v}", e.key)
    } else if !v.contains(['\'', '\n', '\r']) {
        format!("{}='{v}'", e.key)
    } else {
        let mut esc = String::with_capacity(v.len() + 8);
        for c in v.chars() {
            match c {
                '\\' => esc.push_str("\\\\"),
                '"' => esc.push_str("\\\""),
                '$' => esc.push_str("\\$"),
                '\n' => esc.push_str("\\n"),
                '\r' => esc.push_str("\\r"),
                c => esc.push(c),
            }
        }
        format!("{}=\"{esc}\"", e.key)
    }
}

pub fn render(entries: &[EnvEntry]) -> String {
    let mut out = format!(
        "# Exported by Environment Manager on {}\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M")
    );
    for e in entries {
        out.push_str(&render_entry(e));
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kv(p: &ParsedEnv) -> Vec<(&str, &str)> {
        p.entries
            .iter()
            .map(|e| (e.key.as_str(), e.value.as_str()))
            .collect()
    }

    #[test]
    fn parses_common_forms() {
        let p = parse(
            "\u{feff}# comment\r\n\
             PLAIN=hello\r\n\
             export EXPORTED=yes\n\
             SPACED = value with spaces   # trailing comment\n\
             HASH=abc#not-a-comment\n\
             EMPTY=\n\
             SINGLE='it is $literal \\n'\n\
             DOUBLE=\"line1\\nline2 \\\"q\\\" \\$HOME\"\n\
             BACKTICK=`has 'single' and \"double\"`\n\
             MULTI=\"first\n\
             second\"\n\
             URL=postgres://u:p@h:5432/db?ssl=true\n\
             PLAIN=overridden\n",
        );
        assert_eq!(
            kv(&p),
            vec![
                ("EXPORTED", "yes"),
                ("SPACED", "value with spaces"),
                ("HASH", "abc#not-a-comment"),
                ("EMPTY", ""),
                ("SINGLE", "it is $literal \\n"),
                ("DOUBLE", "line1\nline2 \"q\" $HOME"),
                ("BACKTICK", "has 'single' and \"double\""),
                ("MULTI", "first\nsecond"),
                ("URL", "postgres://u:p@h:5432/db?ssl=true"),
                ("PLAIN", "overridden"),
            ]
        );
        assert!(p.skipped.is_empty(), "{:?}", p.skipped);
    }

    #[test]
    fn reports_bad_lines_and_continues() {
        let p = parse("1BAD=x\nno equals here\nOPEN=\"never closed\nOK=1\nTRAIL='a' junk\n");
        let reasons: Vec<(usize, &str)> = p
            .skipped
            .iter()
            .map(|s| (s.line, s.reason.as_str()))
            .collect();
        assert_eq!(
            reasons,
            vec![
                (1, "invalid variable name"),
                (2, "not a KEY=value line"),
                (3, "quote is never closed"),
                (5, "unexpected text after the closing quote"),
            ]
        );
        // An unclosed quote doesn't swallow the lines after it.
        assert_eq!(kv(&p), vec![("OK", "1")]);
    }

    #[test]
    fn render_roundtrips() {
        let values = [
            "",
            "plain",
            "with space",
            "it's",
            "say \"hi\"",
            "$HOME and ${X}",
            "a#b",
            "two\nlines",
            "both ' and \" and \\",
            "postgres://u:p@h/db?x=1",
            "ünïcødé ✓",
        ];
        let entries: Vec<EnvEntry> = values
            .iter()
            .enumerate()
            .map(|(i, v)| EnvEntry {
                key: format!("K{i}"),
                value: v.to_string(),
            })
            .collect();
        let parsed = parse(&render(&entries));
        assert!(parsed.skipped.is_empty(), "{:?}", parsed.skipped);
        assert_eq!(parsed.entries, entries);
    }

    #[test]
    fn file_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".env");
        let entries = vec![EnvEntry {
            key: "A".into(),
            value: "1 2".into(),
        }];
        write_file(&path, &entries).unwrap();
        assert_eq!(read_file(&path).unwrap().entries, entries);
    }
}
