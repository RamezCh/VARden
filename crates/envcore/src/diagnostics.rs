//! A cleaned copy of the log for problem reports.
//!
//! The log never contains values, but it does contain variable names and
//! paths with the user's name in them, and libraries VARden uses may log
//! URLs. Before anything is shared, this removes:
//!
//! * variable names, replaced with `VAR_1`, `VAR_2`… (the same name always
//!   gets the same number, so the log still reads coherently); a few
//!   well-known system names like `PATH` are kept because they help;
//! * the user's name and home folder in paths;
//! * anything that looks like a secret: known key formats, long random
//!   tokens, passwords in URLs and `name=value` pairs.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use regex::Regex;
use serde::Serialize;

/// Names that say nothing about the user and help when reading a report.
const KEEP_NAMES: &[&str] = &[
    "PATH",
    "PATHEXT",
    "HOME",
    "USERPROFILE",
    "APPDATA",
    "LOCALAPPDATA",
    "PROGRAMFILES",
    "SYSTEMROOT",
    "WINDIR",
    "TEMP",
    "TMP",
    "SHELL",
    "LANG",
    "PSMODULEPATH",
    "WSLENV",
    "XDG_CONFIG_HOME",
    "XDG_DATA_HOME",
];

/// Words in log lines that look like variable names but aren't.
const NOT_NAMES: &[&str] = &[
    "INFO",
    "WARN",
    "ERROR",
    "DEBUG",
    "TRACE",
    "HKCU",
    "HKLM",
    "HKEY_CURRENT_USER",
    "HKEY_LOCAL_MACHINE",
    "VARDEN",
    "ENVMANAGER",
    "BOM",
    "UTF",
    "CRLF",
    "URL",
    "JSON",
    "OK",
    "API",
    "PID",
    "UAC",
    "NSIS",
    "MSI",
    "WM_SETTINGCHANGE",
    "REG_SZ",
    "REG_EXPAND_SZ",
    "HTTP",
    "HTTPS",
    "TLS",
    "DNS",
    "GTK",
    "UI",
];

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sanitized {
    pub text: String,
    /// Distinct variable names replaced.
    pub names_hidden: usize,
    /// Places the user's name or home folder was replaced.
    pub user_hidden: usize,
    /// Secret-looking strings removed.
    pub secrets_hidden: usize,
    /// Lines included (the most recent ones).
    pub lines: usize,
}

pub struct Options<'a> {
    /// Variable names VARden knows about (they're hidden even when they
    /// don't look like the usual `UPPER_CASE`).
    pub known_names: &'a [String],
    pub home: Option<&'a Path>,
    pub user_name: Option<&'a str>,
    /// The user chose to keep variable names readable.
    pub keep_names: bool,
}

fn re(cell: &'static OnceLock<Regex>, pattern: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pattern).expect("valid pattern"))
}

/// Patterns for things that must never be shared, checked first.
fn secret_patterns() -> [&'static Regex; 4] {
    static URL_CREDS: OnceLock<Regex> = OnceLock::new();
    static KNOWN: OnceLock<Regex> = OnceLock::new();
    static PAIR: OnceLock<Regex> = OnceLock::new();
    static TOKEN: OnceLock<Regex> = OnceLock::new();
    [
        // scheme://user:password@host
        re(
            &URL_CREDS,
            r"(?P<keep>[A-Za-z][A-Za-z0-9+.-]*://)[^\s/@:]+:[^\s/@]+@",
        ),
        // Well-known key formats.
        re(
            &KNOWN,
            r"\b(?:sk-[A-Za-z0-9_-]{8,}|sk_(?:live|test)_[A-Za-z0-9]{8,}|gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|xox[abprs]-[A-Za-z0-9-]{10,}|AKIA[0-9A-Z]{16}|AIza[0-9A-Za-z_-]{30,}|eyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,})",
        ),
        // name=value or name: value where the name sounds secret.
        re(
            &PAIR,
            r"(?i)(?P<keep>\b[A-Za-z0-9_.-]*(?:key|token|secret|password|passwd|pwd|auth|credential|session|cookie)[A-Za-z0-9_.-]*\s*[=:]\s*)[^\s,;]+",
        ),
        // Long random-looking tokens (letters and digits mixed, 24+ chars).
        re(&TOKEN, r"\b[A-Za-z0-9+/_-]{24,}={0,2}"),
    ]
}

fn looks_random(s: &str) -> bool {
    let letters = s.chars().filter(|c| c.is_ascii_alphabetic()).count();
    let digits = s.chars().filter(|c| c.is_ascii_digit()).count();
    // Paths and words are mostly letters; tokens mix in many digits.
    letters > 0 && digits >= 4 && !s.contains("__")
}

fn hide_secrets(line: &str, count: &mut usize) -> String {
    let [url, known, pair, token] = secret_patterns();
    let mut s = url
        .replace_all(line, |c: &regex::Captures| {
            *count += 1;
            format!("{}[hidden]@", &c["keep"])
        })
        .into_owned();
    s = known
        .replace_all(&s, |_: &regex::Captures| {
            *count += 1;
            "[hidden]".to_string()
        })
        .into_owned();
    s = pair
        .replace_all(&s, |c: &regex::Captures| {
            *count += 1;
            format!("{}[hidden]", &c["keep"])
        })
        .into_owned();
    token
        .replace_all(&s, |c: &regex::Captures| {
            if looks_random(&c[0]) {
                *count += 1;
                "[hidden]".to_string()
            } else {
                c[0].to_string()
            }
        })
        .into_owned()
}

fn hide_user(line: &str, opts: &Options, count: &mut usize) -> String {
    let mut s = line.to_string();
    if let Some(home) = opts.home.map(|h| h.to_string_lossy().into_owned())
        && home.len() > 3
    {
        let n = s.matches(&home).count();
        if n > 0 {
            *count += n;
            s = s.replace(&home, "~");
        }
    }
    if let Some(user) = opts.user_name.filter(|u| u.len() >= 3) {
        let pattern = format!(r"(?i)\b{}\b", regex::escape(user));
        if let Ok(r) = Regex::new(&pattern) {
            s = r
                .replace_all(&s, |_: &regex::Captures| {
                    *count += 1;
                    "<you>"
                })
                .into_owned();
        }
    }
    s
}

fn hide_names(line: &str, opts: &Options, map: &mut BTreeMap<String, String>) -> String {
    static UPPER: OnceLock<Regex> = OnceLock::new();
    // UPPER_CASE words of 3+ characters, the usual shape of variable names.
    let upper = re(
        &UPPER,
        r"\b[A-Z][A-Z0-9]*(?:_[A-Z0-9]+)+\b|\b[A-Z][A-Z0-9]{2,}\b",
    );
    let keep = |w: &str| {
        let u = w.to_ascii_uppercase();
        KEEP_NAMES.contains(&u.as_str()) || NOT_NAMES.contains(&u.as_str())
    };
    let name_for = |w: &str, map: &mut BTreeMap<String, String>| {
        let n = map.len() + 1;
        map.entry(w.to_string())
            .or_insert_with(|| format!("VAR_{n}"))
            .clone()
    };
    // Known names first (any casing, e.g. `Path`, `my_token`).
    let mut s = line.to_string();
    for name in opts.known_names {
        if name.len() < 2 || keep(name) {
            continue;
        }
        let pattern = format!(r"\b{}\b", regex::escape(name));
        if let Ok(r) = Regex::new(&pattern)
            && r.is_match(&s)
        {
            let alias = name_for(name, map);
            s = r.replace_all(&s, alias.as_str()).into_owned();
        }
    }
    upper
        .replace_all(&s, |c: &regex::Captures| {
            let w = &c[0];
            if keep(w) || w.starts_with("VAR_") {
                w.to_string()
            } else {
                name_for(w, map)
            }
        })
        .into_owned()
}

/// Clean `log` (most recent `max_lines` lines) for sharing.
pub fn sanitize(log: &str, opts: &Options, max_lines: usize) -> Sanitized {
    let lines: Vec<&str> = log.lines().collect();
    let start = lines.len().saturating_sub(max_lines);
    let mut out = Sanitized::default();
    let mut names = BTreeMap::new();
    let mut text = Vec::with_capacity(lines.len() - start);
    for line in &lines[start..] {
        let mut l = hide_secrets(line, &mut out.secrets_hidden);
        l = hide_user(&l, opts, &mut out.user_hidden);
        if !opts.keep_names {
            l = hide_names(&l, opts, &mut names);
        }
        text.push(l);
    }
    out.names_hidden = names.len();
    out.lines = text.len();
    out.text = text.join("\n");
    out
}

/// The log files, newest last, so the most recent lines end the report.
pub fn read_recent_logs(dir: &Path, max_files: usize) -> String {
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "log"))
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    files.sort();
    let skip = files.len().saturating_sub(max_files);
    files
        .into_iter()
        .skip(skip)
        .filter_map(|(_, p)| std::fs::read(&p).ok())
        .map(|b| String::from_utf8_lossy(&b).into_owned())
        .collect::<Vec<_>>()
        .join("\n")
}

impl crate::manager::EnvManager {
    /// Every variable name VARden knows about: saved, temporary, in profiles,
    /// and in the environment it started with.
    pub fn known_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .list(&[])
            .map(|rows| rows.into_iter().map(|r| r.key).collect())
            .unwrap_or_default();
        for p in self.profiles().profiles {
            names.extend(p.vars.into_iter().map(|v| v.key));
        }
        names.extend(std::env::vars_os().map(|(k, _)| k.to_string_lossy().into_owned()));
        // Longest first, so `API_KEY_OLD` is replaced before `API_KEY`.
        names.sort_by(|a, b| b.len().cmp(&a.len()).then(a.cmp(b)));
        names.dedup();
        names
    }
}

/// The current user's login name, if the OS tells us.
pub fn user_name() -> Option<String> {
    std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .ok()
        .filter(|u| !u.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts<'a>(known: &'a [String], keep: bool) -> Options<'a> {
        Options {
            known_names: known,
            home: Some(Path::new(r"C:\Users\ramiz")),
            user_name: Some("ramiz"),
            keep_names: keep,
        }
    }

    #[test]
    fn hides_names_consistently_but_keeps_system_ones() {
        let known = vec!["Path".to_string(), "my_token".to_string()];
        let log = "[INFO] Set OPENAI_API_KEY (User)\n[INFO] Deleted OPENAI_API_KEY, STRIPE_KEY (User)\n[INFO] Set Path\n[INFO] Set my_token\n[INFO] Saved PATH (3 entries)";
        let s = sanitize(log, &opts(&known, false), 100);
        assert!(
            !s.text.contains("OPENAI")
                && !s.text.contains("STRIPE")
                && !s.text.contains("my_token")
        );
        // The same name gets the same alias everywhere.
        let first = s.text.lines().next().unwrap().to_string();
        let alias = first.split_whitespace().nth(2).unwrap().to_string();
        assert!(
            s.text.lines().nth(1).unwrap().contains(&alias),
            "{}",
            s.text
        );
        // PATH is a system name in any casing; it helps and says nothing personal.
        assert!(
            s.text.contains("[INFO]")
                && s.text.contains("Saved PATH")
                && s.text.contains("Set Path")
        );
        assert_eq!(s.names_hidden, 3);

        let kept = sanitize(log, &opts(&known, true), 100);
        assert!(kept.text.contains("OPENAI_API_KEY"));
        assert_eq!(kept.names_hidden, 0);
    }

    #[test]
    fn hides_the_user_and_home_folder() {
        let log = r"[INFO] Updated terminal sync in C:\Users\ramiz\Documents\PowerShell\profile.ps1 for Ramiz";
        let s = sanitize(log, &opts(&[], true), 100);
        assert_eq!(
            s.text,
            r"[INFO] Updated terminal sync in ~\Documents\PowerShell\profile.ps1 for <you>"
        );
        assert_eq!(s.user_hidden, 2);
    }

    #[test]
    fn removes_anything_that_looks_secret() {
        let log = [
            "proxy http://bob:hunter2@10.0.0.1:8080/ intercepts",
            "token sk-proj-Xk2v9QmL0aP7abcd leaked",
            "header Authorization: Bearer-abc api_key=s3cr3t-value",
            "id 9f8e7d6c5b4a39281706f5e4d3c2b1a0ffee",
            "jwt eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIn0.c2lnbmF0dXJlMTIz",
        ]
        .join("\n");
        let s = sanitize(&log, &opts(&[], true), 100);
        for leak in ["hunter2", "bob:", "sk-proj", "s3cr3t", "9f8e7d6c", "eyJhbG"] {
            assert!(!s.text.contains(leak), "{leak} in {}", s.text);
        }
        assert!(s.text.contains("http://[hidden]@10.0.0.1"));
        assert!(s.secrets_hidden >= 5);
    }

    #[test]
    fn leaves_ordinary_lines_alone() {
        let log = "[2026-10-28][20:26:55][varden_lib][INFO] VARden 0.1.0 starting on windows\nWatching HKCU\\Environment for outside changes\nBackup 20261028T202701.798Z restored";
        let s = sanitize(log, &opts(&[], false), 100);
        assert_eq!(s.text, log);
        assert_eq!((s.names_hidden, s.secrets_hidden), (0, 0));
    }

    #[test]
    fn keeps_only_the_latest_lines() {
        let log = (1..=10)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let s = sanitize(&log, &opts(&[], true), 3);
        assert_eq!(s.text, "line 8\nline 9\nline 10");
    }
}
