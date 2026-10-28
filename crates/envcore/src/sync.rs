//! "Sync to terminals": pushing saved variables into terminals that are
//! already open.
//!
//! A running shell can't be changed from outside, so each supported shell
//! gets a tiny hook (installed into its profile on request) that runs before
//! every prompt:
//!
//! 1. read `sync/generation` (one small file);
//! 2. if it differs from the generation the shell last applied, source
//!    `sync/apply.<ext>` and remember the new generation.
//!
//! "Sync to terminals" writes the apply scripts first and bumps the
//! generation last, so a hook never sees a half-written state.
//!
//! Apply scripts always describe the *full* desired state (every synced
//! variable, every variable ever synced and since deleted, PATH entries to
//! add and to drop), so a terminal that missed several syncs still ends up
//! correct. PATH is merged entry by entry instead of overwritten, which
//! keeps things like an activated virtualenv intact.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{fsutil, Error, Result};

pub const GENERATION_FILE: &str = "generation";
const STATE_FILE: &str = "state.json";

/// Shell dialects that have a sync hook.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HookShell {
    /// bash and zsh (also Git Bash)
    Posix,
    Fish,
    PowerShell,
}

impl HookShell {
    pub const ALL: [HookShell; 3] = [HookShell::Posix, HookShell::Fish, HookShell::PowerShell];

    pub fn ext(self) -> &'static str {
        match self {
            HookShell::Posix => "sh",
            HookShell::Fish => "fish",
            HookShell::PowerShell => "ps1",
        }
    }
}

/// What was synced last time; persisted in `sync/state.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncState {
    pub generation: u64,
    pub synced_at: Option<DateTime<Utc>>,
    /// Variables pushed last time (PATH excluded).
    pub synced: BTreeMap<String, String>,
    /// Variables pushed at some point and deleted since; terminals unset them.
    pub removed: BTreeSet<String>,
    /// The user's own PATH entries pushed last time.
    pub path: Vec<String>,
    /// PATH entries pushed at some point and removed since.
    pub removed_path: BTreeSet<String>,
}

impl SyncState {
    pub fn load(dir: &Path) -> SyncState {
        fs::read(dir.join(STATE_FILE))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }
}

/// What terminals should end up with.
#[derive(Debug, Clone, Default)]
pub struct Desired {
    /// Variables to set (PATH excluded).
    pub vars: Vec<(String, String)>,
    /// The user's PATH entries, if the user store defines PATH.
    pub path: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub set: Vec<(String, String)>,
    /// Variables to remove. `Some(v)` means "fall back to the system-level
    /// value `v`" instead of removing (Windows machine variables).
    pub unset: Vec<(String, Option<String>)>,
    pub path_add: Vec<String>,
    pub path_remove: Vec<String>,
    pub state: SyncState,
}

/// Work out what the apply scripts must do. Pure, so it is easy to test.
pub fn plan(
    desired: &Desired,
    previous: &SyncState,
    system: &BTreeMap<String, String>,
    case_insensitive: bool,
) -> Plan {
    let norm = |k: &str| {
        if case_insensitive {
            k.to_ascii_uppercase()
        } else {
            k.to_string()
        }
    };
    let norm_path = |p: &str| {
        let p = p.trim().trim_end_matches(['/', '\\']);
        if case_insensitive {
            p.to_ascii_lowercase()
        } else {
            p.to_string()
        }
    };

    let wanted: BTreeSet<String> = desired.vars.iter().map(|(k, _)| norm(k)).collect();
    let mut removed: BTreeSet<String> = previous
        .removed
        .iter()
        .chain(previous.synced.keys())
        .filter(|k| !wanted.contains(&norm(k)))
        .cloned()
        .collect();
    // Keep one spelling per variable.
    let mut seen = BTreeSet::new();
    removed.retain(|k| seen.insert(norm(k)));

    let system_get = |k: &str| {
        system
            .iter()
            .find(|(sk, _)| norm(sk) == norm(k))
            .map(|(_, v)| v.clone())
    };
    let unset = removed.iter().map(|k| (k.clone(), system_get(k))).collect();

    let wanted_path: BTreeSet<String> = desired.path.iter().map(|p| norm_path(p)).collect();
    let mut path_remove: Vec<String> = Vec::new();
    for p in previous.removed_path.iter().chain(previous.path.iter()) {
        let n = norm_path(p);
        if !wanted_path.contains(&n) && !path_remove.iter().any(|x| norm_path(x) == n) {
            path_remove.push(p.clone());
        }
    }

    let state = SyncState {
        generation: previous.generation + 1,
        synced_at: Some(Utc::now()),
        synced: desired.vars.iter().cloned().collect(),
        removed,
        path: desired.path.clone(),
        removed_path: path_remove.iter().cloned().collect(),
    };
    Plan {
        set: desired.vars.clone(),
        unset,
        path_add: desired.path.clone(),
        path_remove,
        state,
    }
}

// ---------- quoting ----------

pub(crate) fn q_posix(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}
pub(crate) fn q_fish(s: &str) -> String {
    format!("'{}'", s.replace('\\', r"\\").replace('\'', r"\'"))
}
pub(crate) fn q_ps(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

// ---------- apply scripts ----------

fn header(comment: &str, generation: u64) -> String {
    format!("{comment} Generated by Environment Manager (sync {generation}). Do not edit; it is rewritten on every sync.\n")
}

pub fn render_apply(shell: HookShell, plan: &Plan) -> String {
    let g = plan.state.generation;
    let mut out = String::new();
    match shell {
        HookShell::PowerShell => {
            out.push_str(&header("#", g));
            for (k, v) in &plan.set {
                out.push_str(&format!("$env:{k} = {}\n", q_ps(v)));
            }
            for (k, fallback) in &plan.unset {
                match fallback {
                    Some(v) => out.push_str(&format!("$env:{k} = {}\n", q_ps(v))),
                    None => out.push_str(&format!(
                        "Remove-Item -LiteralPath Env:\\{k} -ErrorAction SilentlyContinue\n"
                    )),
                }
            }
            if !plan.path_add.is_empty() || !plan.path_remove.is_empty() {
                let list = |v: &[String]| v.iter().map(|p| q_ps(p)).collect::<Vec<_>>().join(", ");
                out.push_str(&format!(
                    "$__emSep = [System.IO.Path]::PathSeparator\n\
                     $__emDrop = @({}) | ForEach-Object {{ $_.TrimEnd('\\', '/') }}\n\
                     $__emPath = [System.Collections.Generic.List[string]]::new()\n\
                     foreach ($__emP in ($env:PATH -split $__emSep)) {{\n\
                     \x20   if ($__emP -and -not ($__emDrop -contains $__emP.TrimEnd('\\', '/'))) {{ [void]$__emPath.Add($__emP) }}\n\
                     }}\n\
                     foreach ($__emP in @({})) {{\n\
                     \x20   $__emHave = $__emPath | ForEach-Object {{ $_.TrimEnd('\\', '/') }}\n\
                     \x20   if (-not ($__emHave -contains $__emP.TrimEnd('\\', '/'))) {{ [void]$__emPath.Add($__emP) }}\n\
                     }}\n\
                     $env:PATH = $__emPath -join $__emSep\n\
                     Remove-Variable __emSep, __emDrop, __emPath, __emP, __emHave -ErrorAction SilentlyContinue\n",
                    list(&plan.path_remove),
                    list(&plan.path_add),
                ));
            }
        }
        HookShell::Posix => {
            out.push_str(&header("#", g));
            for (k, v) in &plan.set {
                out.push_str(&format!("export {k}={}\n", q_posix(v)));
            }
            for (k, fallback) in &plan.unset {
                match fallback {
                    Some(v) => out.push_str(&format!("export {k}={}\n", q_posix(v))),
                    None => out.push_str(&format!("unset {k}\n")),
                }
            }
            if !plan.path_add.is_empty() || !plan.path_remove.is_empty() {
                out.push_str(POSIX_PATH_HELPERS);
                for p in &plan.path_remove {
                    out.push_str(&format!("__envmanager_path_remove {}\n", q_posix(p)));
                }
                for p in &plan.path_add {
                    out.push_str(&format!("__envmanager_path_add {}\n", q_posix(p)));
                }
                out.push_str(
                    "export PATH\nunset -f __envmanager_path_add __envmanager_path_remove\n",
                );
            }
        }
        HookShell::Fish => {
            out.push_str(&header("#", g));
            for (k, v) in &plan.set {
                out.push_str(&format!("set -gx {k} {}\n", q_fish(v)));
            }
            for (k, fallback) in &plan.unset {
                match fallback {
                    Some(v) => out.push_str(&format!("set -gx {k} {}\n", q_fish(v))),
                    None => out.push_str(&format!("set -e -g {k}\n")),
                }
            }
            for p in &plan.path_remove {
                out.push_str(&format!(
                    "while set -l __em_i (contains -i -- {q} $PATH); set -e PATH[$__em_i]; end\n",
                    q = q_fish(p)
                ));
            }
            for p in &plan.path_add {
                out.push_str(&format!(
                    "contains -- {q} $PATH; or set -gx PATH $PATH {q}\n",
                    q = q_fish(p)
                ));
            }
        }
    }
    out
}

/// PATH helpers for bash and zsh. Patterns use quoted "$1", which both
/// shells treat literally, so entries containing `*` or `?` are safe.
const POSIX_PATH_HELPERS: &str = r#"__envmanager_path_add() {
  case ":$PATH:" in *":$1:"*) ;; *) PATH="${PATH:+$PATH:}$1" ;; esac
}
__envmanager_path_remove() {
  __em_rest=":$PATH:"
  while :; do
    case "$__em_rest" in
      *":$1:"*) __em_rest="${__em_rest%%:"$1":*}:${__em_rest#*:"$1":}" ;;
      *) break ;;
    esac
  done
  __em_rest="${__em_rest#:}"
  PATH="${__em_rest%:}"
  unset __em_rest
}
"#;

// ---------- hook scripts ----------

/// The hook each profile sources. `sync_dir` is baked in.
pub fn render_hook(shell: HookShell, sync_dir: &Path) -> String {
    let dir = sync_dir.to_string_lossy();
    match shell {
        HookShell::PowerShell => format!(
            r#"# Environment Manager terminal sync hook (PowerShell). Generated; do not edit.
# Before each prompt, applies variables published by "Sync to terminals".
$global:__EnvManagerSyncDir = {dir}
$global:__EnvManagerGeneration = $null
$__emGenFile = Join-Path $global:__EnvManagerSyncDir 'generation'
if (Test-Path -LiteralPath $__emGenFile) {{
    # A new terminal already has fresh variables; only later syncs apply.
    $global:__EnvManagerGeneration = [System.IO.File]::ReadAllText($__emGenFile).Trim()
}}
Remove-Variable __emGenFile -ErrorAction SilentlyContinue

function global:__EnvManagerApply {{
    $genFile = Join-Path $global:__EnvManagerSyncDir 'generation'
    if (-not (Test-Path -LiteralPath $genFile)) {{ return }}
    $gen = [System.IO.File]::ReadAllText($genFile).Trim()
    if ($gen -eq $global:__EnvManagerGeneration) {{ return }}
    $apply = Join-Path $global:__EnvManagerSyncDir 'apply.ps1'
    if (Test-Path -LiteralPath $apply) {{ . $apply }}
    $global:__EnvManagerGeneration = $gen
}}

if (-not $global:__EnvManagerOriginalPrompt) {{
    $global:__EnvManagerOriginalPrompt = $function:prompt
    function global:prompt {{
        $code = $global:LASTEXITCODE
        try {{ __EnvManagerApply }} catch {{ }}
        $global:LASTEXITCODE = $code
        & $global:__EnvManagerOriginalPrompt
    }}
}}
"#,
            dir = q_ps(&dir)
        ),
        HookShell::Posix => format!(
            r#"# Environment Manager terminal sync hook (bash/zsh). Generated; do not edit.
# Before each prompt, applies variables published by "Sync to terminals".
__envmanager_dir={dir}
__envmanager_gen=
# A new terminal already has fresh variables; only later syncs apply.
[ -r "$__envmanager_dir/generation" ] && read -r __envmanager_gen < "$__envmanager_dir/generation"

__envmanager_apply() {{
  __envmanager_status=$?
  if [ -r "$__envmanager_dir/generation" ]; then
    read -r __envmanager_new < "$__envmanager_dir/generation"
    if [ "$__envmanager_new" != "$__envmanager_gen" ]; then
      [ -r "$__envmanager_dir/apply.sh" ] && . "$__envmanager_dir/apply.sh"
      __envmanager_gen=$__envmanager_new
    fi
  fi
  return $__envmanager_status
}}

if [ -n "${{ZSH_VERSION:-}}" ]; then
  autoload -Uz add-zsh-hook && add-zsh-hook precmd __envmanager_apply
elif [ -n "${{BASH_VERSION:-}}" ]; then
  case ";${{PROMPT_COMMAND:-}};" in
    *";__envmanager_apply;"*) ;;
    *) PROMPT_COMMAND="__envmanager_apply${{PROMPT_COMMAND:+;$PROMPT_COMMAND}}" ;;
  esac
fi
"#,
            dir = q_posix(&dir)
        ),
        HookShell::Fish => format!(
            r#"# Environment Manager terminal sync hook (fish). Generated; do not edit.
# Before each prompt, applies variables published by "Sync to terminals".
set -g __envmanager_dir {dir}
set -g __envmanager_gen ''
# A new terminal already has fresh variables; only later syncs apply.
test -r $__envmanager_dir/generation; and read __envmanager_gen < $__envmanager_dir/generation

function __envmanager_apply --on-event fish_prompt
    test -r $__envmanager_dir/generation; or return
    read -l gen < $__envmanager_dir/generation
    test "$gen" = "$__envmanager_gen"; and return
    test -r $__envmanager_dir/apply.fish; and source $__envmanager_dir/apply.fish
    set -g __envmanager_gen $gen
end
"#,
            dir = q_fish(&dir)
        ),
    }
}

/// The line placed (inside the HOOK markers) in a shell profile.
pub fn profile_line(shell: HookShell, hook_file: &Path) -> String {
    let p = hook_file.to_string_lossy();
    match shell {
        HookShell::PowerShell => {
            format!("if (Test-Path -LiteralPath {q}) {{ . {q} }}", q = q_ps(&p))
        }
        HookShell::Posix => format!("[ -r {q} ] && . {q}", q = q_posix(&p)),
        HookShell::Fish => format!("test -r {q}; and source {q}", q = q_fish(&p)),
    }
}

pub fn hook_file(hooks_dir: &Path, shell: HookShell) -> PathBuf {
    hooks_dir.join(format!("hook.{}", shell.ext()))
}

/// (Re)write all hook scripts. Cheap; done on startup and before installs.
pub fn write_hooks(hooks_dir: &Path, sync_dir: &Path) -> Result<()> {
    for shell in HookShell::ALL {
        fsutil::write_atomic(
            &hook_file(hooks_dir, shell),
            &script_bytes(shell, &render_hook(shell, sync_dir)),
        )?;
    }
    Ok(())
}

/// Windows PowerShell 5.1 reads BOM-less files as ANSI, which garbles any
/// non-ASCII value; a UTF-8 BOM makes every PowerShell version read UTF-8.
fn script_bytes(shell: HookShell, text: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() + 3);
    if shell == HookShell::PowerShell {
        out.extend_from_slice("\u{feff}".as_bytes());
    }
    out.extend_from_slice(text.as_bytes());
    out
}

/// Publish a plan: apply scripts first, then state, then the generation
/// number that hooks watch.
pub fn publish(sync_dir: &Path, plan: &Plan) -> Result<()> {
    fs::create_dir_all(sync_dir).map_err(|e| Error::io(sync_dir, e))?;
    for shell in HookShell::ALL {
        let path = sync_dir.join(format!("apply.{}", shell.ext()));
        fsutil::write_atomic(&path, &script_bytes(shell, &render_apply(shell, plan)))?;
    }
    fsutil::write_atomic(
        &sync_dir.join(STATE_FILE),
        &serde_json::to_vec_pretty(&plan.state)?,
    )?;
    fsutil::write_atomic(
        &sync_dir.join(GENERATION_FILE),
        format!("{}\n", plan.state.generation).as_bytes(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn desired(vars: &[(&str, &str)], path: &[&str]) -> Desired {
        Desired {
            vars: vars
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            path: path.iter().map(|p| p.to_string()).collect(),
        }
    }

    #[test]
    fn removed_variables_and_path_entries_accumulate() {
        let none = BTreeMap::new();
        let p1 = plan(
            &desired(&[("A", "1"), ("B", "2")], &["/x", "/y"]),
            &SyncState::default(),
            &none,
            false,
        );
        assert_eq!(p1.state.generation, 1);
        assert!(p1.unset.is_empty() && p1.path_remove.is_empty());

        let p2 = plan(&desired(&[("A", "1")], &["/x"]), &p1.state, &none, false);
        assert_eq!(p2.unset, vec![("B".to_string(), None)]);
        assert_eq!(p2.path_remove, vec!["/y".to_string()]);

        // A terminal that missed sync 2 must still drop B and /y at sync 3.
        let p3 = plan(
            &desired(&[("A", "1"), ("C", "3")], &["/x"]),
            &p2.state,
            &none,
            false,
        );
        assert_eq!(p3.unset, vec![("B".to_string(), None)]);
        assert_eq!(p3.path_remove, vec!["/y".to_string()]);

        // Re-adding clears it from the removed lists.
        let p4 = plan(&desired(&[("B", "back")], &["/y"]), &p3.state, &none, false);
        assert!(p4.unset.iter().all(|(k, _)| k != "B"));
        assert!(p4.path_remove.iter().all(|p| p != "/y"));
    }

    #[test]
    fn windows_semantics() {
        let mut system = BTreeMap::new();
        system.insert("TEMP".to_string(), r"C:\Windows\Temp".to_string());
        let p1 = plan(
            &desired(&[("temp", r"C:\Users\me\tmp")], &[r"C:\Tools\"]),
            &SyncState::default(),
            &system,
            true,
        );
        let p2 = plan(&desired(&[], &[r"c:\tools"]), &p1.state, &system, true);
        // Deleting a user variable falls back to the machine value.
        assert_eq!(
            p2.unset,
            vec![("temp".to_string(), Some(r"C:\Windows\Temp".to_string()))]
        );
        // Same PATH entry differing only in case / trailing slash isn't removed.
        assert!(p2.path_remove.is_empty());
    }

    #[test]
    fn publish_writes_generation_last_and_state_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let p = plan(
            &desired(&[("A", "it's")], &["/x"]),
            &SyncState::default(),
            &BTreeMap::new(),
            false,
        );
        publish(dir.path(), &p).unwrap();
        assert_eq!(
            fs::read_to_string(dir.path().join("generation")).unwrap(),
            "1\n"
        );
        assert_eq!(SyncState::load(dir.path()), p.state);
        for ext in ["sh", "fish", "ps1"] {
            assert!(dir.path().join(format!("apply.{ext}")).is_file());
        }
        let ps = fs::read(dir.path().join("apply.ps1")).unwrap();
        assert!(
            ps.starts_with("\u{feff}".as_bytes()),
            "PowerShell scripts carry a UTF-8 BOM"
        );
        assert!(!fs::read(dir.path().join("apply.sh"))
            .unwrap()
            .starts_with(b"\xef"));
    }

    // ---- end-to-end: run the real hook + apply scripts in installed shells ----

    const TRICKY: &str = r#"it's $HOME `x` \ "q" %PATH% ;"#;

    /// Sync 1 is live when the shell starts; sync 2 is published while it
    /// runs. Returns (sync_dir, stage_dir with sync 2, hooks_dir).
    fn scenario(root: &Path, sep: char) -> (PathBuf, PathBuf, PathBuf) {
        let (sync_dir, stage, hooks) = (root.join("sync"), root.join("stage"), root.join("shell"));
        let none = BTreeMap::new();
        let p1 = plan(
            &desired(&[("A", "1"), ("B", "b")], &["/x"]),
            &SyncState::default(),
            &none,
            sep == ';',
        );
        publish(&sync_dir, &p1).unwrap();
        let p2 = plan(
            &desired(&[("A", TRICKY)], &["/y"]),
            &p1.state,
            &none,
            sep == ';',
        );
        publish(&stage, &p2).unwrap();
        write_hooks(&hooks, &sync_dir).unwrap();
        (sync_dir, stage, hooks)
    }

    fn run(cmd: &mut std::process::Command) -> Vec<String> {
        let out = cmd.output().expect("shell runs");
        assert!(
            out.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(str::to_string)
            .collect()
    }

    fn expect(lines: Vec<String>, base_path: &str, sep: char) {
        assert_eq!(
            lines,
            vec![
                "A=<unset>".to_string(),
                "B=b".to_string(),
                format!("A={TRICKY}"),
                "B=<unset>".to_string(),
                format!("P={base_path}{sep}/y"),
            ]
        );
    }

    #[cfg(unix)]
    #[test]
    fn hook_works_in_bash_and_zsh() {
        const SCRIPT: &str = r#". "$HOOK"
printf 'A=%s\nB=%s\n' "${A-<unset>}" "${B-<unset>}"
cp "$STAGE"/* "$SYNC"/
__envmanager_apply
printf 'A=%s\nB=%s\nP=%s\n' "${A-<unset>}" "${B-<unset>}" "$PATH"
"#;
        for (shell, args) in [("bash", vec!["--norc", "-c"]), ("zsh", vec!["-f", "-c"])] {
            let Ok(exe) = which::which(shell) else {
                continue;
            };
            let dir = tempfile::tempdir().unwrap();
            let (sync_dir, stage, hooks) = scenario(dir.path(), ':');
            let lines = run(std::process::Command::new(exe)
                .args(&args)
                .arg(SCRIPT)
                .env_clear()
                .env("PATH", "/usr/bin:/bin:/x:/x")
                .env("B", "b")
                .env("HOOK", hook_file(&hooks, HookShell::Posix))
                .env("STAGE", &stage)
                .env("SYNC", &sync_dir));
            expect(lines, "/usr/bin:/bin", ':');
        }
    }

    #[cfg(unix)]
    #[test]
    fn hook_works_in_fish() {
        let Ok(exe) = which::which("fish") else {
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        let (sync_dir, stage, hooks) = scenario(dir.path(), ':');
        let script = r#"source $HOOK
set -q A; and echo "A=$A"; or echo "A=<unset>"
set -q B; and echo "B=$B"; or echo "B=<unset>"
cp $STAGE/* $SYNC/
emit fish_prompt
set -q A; and echo "A=$A"; or echo "A=<unset>"
set -q B; and echo "B=$B"; or echo "B=<unset>"
echo "P="(string join : $PATH)
"#;
        let lines = run(std::process::Command::new(exe)
            .args(["--no-config", "-c", script])
            .env_clear()
            .env("PATH", "/usr/bin:/bin:/x:/x")
            .env("B", "b")
            .env("HOME", dir.path())
            .env("HOOK", hook_file(&hooks, HookShell::Fish))
            .env("STAGE", &stage)
            .env("SYNC", &sync_dir));
        expect(lines, "/usr/bin:/bin", ':');
    }

    #[test]
    fn hook_works_in_powershell() {
        let Some(exe) = ["pwsh", "powershell"]
            .iter()
            .find_map(|s| which::which(s).ok())
        else {
            return;
        };
        let sep = if cfg!(windows) { ';' } else { ':' };
        let dir = tempfile::tempdir().unwrap();
        let (sync_dir, stage, hooks) = scenario(dir.path(), sep);
        let script = r#". $env:HOOK
function Show($name) { if (Test-Path "Env:\$name") { "$name=" + (Get-Item "Env:\$name").Value } else { "$name=<unset>" } }
Show A; Show B
Copy-Item -Path (Join-Path $env:STAGE '*') -Destination $env:SYNC -Force
$null = prompt
Show A; Show B
"P=" + $env:PATH
"#;
        let base = std::env::var("PATH").unwrap_or_default();
        let initial = format!("{base}{sep}/x{sep}/x");
        let lines = run(std::process::Command::new(exe)
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .env("PATH", &initial)
            .env("B", "b")
            .env_remove("A")
            .env("HOOK", hook_file(&hooks, HookShell::PowerShell))
            .env("STAGE", &stage)
            .env("SYNC", &sync_dir));
        // pwsh may put its own folder in front of PATH at startup, so check
        // the PATH line by its entries: no /x left, /y appended once at the end.
        let (head, path_line) = lines.split_at(4);
        expect(
            [head.to_vec(), vec![format!("P={base}{sep}/y")]].concat(),
            &base,
            sep,
        );
        let entries: Vec<&str> = path_line[0].trim_start_matches("P=").split(sep).collect();
        assert!(!entries.contains(&"/x"), "{entries:?}");
        assert_eq!(entries.iter().filter(|e| **e == "/y").count(), 1);
        assert_eq!(entries.last(), Some(&"/y"));
        for e in base.split(sep).filter(|e| !e.is_empty()) {
            assert!(entries.contains(&e), "lost {e}");
        }
    }
}
