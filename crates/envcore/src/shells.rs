//! Auto-detection of installed shells and their config/profile files.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::managed_block::Syntax;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ShellKind {
    Bash,
    Zsh,
    Fish,
    Nushell,
    /// PowerShell 7+ (`pwsh`)
    PowerShell,
    /// Windows PowerShell 5.1 (`powershell.exe`)
    WindowsPowerShell,
    Cmd,
    GitBash,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedShell {
    pub kind: ShellKind,
    pub name: &'static str,
    pub executable: Option<PathBuf>,
    /// The file our managed block would live in. `None` for shells with no
    /// profile file (cmd.exe).
    pub config_path: Option<PathBuf>,
    pub config_exists: bool,
    pub syntax: Option<Syntax>,
    /// On Windows every shell inherits HKCU\Environment, so writing the
    /// registry already reaches it; the profile file is optional.
    pub inherits_user_env: bool,
    /// The user's login shell (Unix) or preferred PowerShell (Windows).
    pub is_default: bool,
}

/// Guess the assignment syntax from a file name, for custom profile paths.
pub fn syntax_for_path(path: &Path) -> Syntax {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("fish") => Syntax::Fish,
        Some("nu") => Syntax::Nushell,
        Some("ps1") => Syntax::PowerShell,
        _ => Syntax::Posix,
    }
}

fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

fn shell(
    kind: ShellKind,
    name: &'static str,
    executable: Option<PathBuf>,
    config_path: Option<PathBuf>,
    syntax: Option<Syntax>,
) -> DetectedShell {
    DetectedShell {
        kind,
        name,
        config_exists: config_path.as_deref().is_some_and(Path::exists),
        executable,
        config_path,
        syntax,
        inherits_user_env: cfg!(windows),
        is_default: false,
    }
}

/// Nushell keeps env setup in `env.nu` (older layout) or `config.nu`.
fn nushell_config() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.join("nushell").exists())
        .or_else(dirs::config_dir)?
        .join("nushell");
    let env = base.join("env.nu");
    Some(if env.exists() {
        env
    } else {
        base.join("config.nu")
    })
}

/// Scan for installed shells. Only shells whose executable is found (or
/// whose config file already exists) are returned.
pub fn detect() -> Vec<DetectedShell> {
    let mut found = if cfg!(windows) {
        detect_windows()
    } else {
        detect_unix()
    };
    found.retain(|s| s.executable.is_some() || s.config_exists);
    found
}

fn detect_unix() -> Vec<DetectedShell> {
    let home = home();
    let xdg = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".config"));
    let zdotdir = std::env::var_os("ZDOTDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.clone());

    // macOS Terminal starts login shells, which read .bash_profile, not .bashrc.
    let bash_rc = if cfg!(target_os = "macos") && home.join(".bash_profile").exists() {
        home.join(".bash_profile")
    } else {
        home.join(".bashrc")
    };

    let mut shells = vec![
        shell(
            ShellKind::Bash,
            "Bash",
            which::which("bash").ok(),
            Some(bash_rc),
            Some(Syntax::Posix),
        ),
        shell(
            ShellKind::Zsh,
            "Zsh",
            which::which("zsh").ok(),
            Some(zdotdir.join(".zshrc")),
            Some(Syntax::Posix),
        ),
        shell(
            ShellKind::Fish,
            "Fish",
            which::which("fish").ok(),
            Some(xdg.join("fish").join("config.fish")),
            Some(Syntax::Fish),
        ),
        shell(
            ShellKind::Nushell,
            "Nushell",
            which::which("nu").ok(),
            nushell_config(),
            Some(Syntax::Nushell),
        ),
        shell(
            ShellKind::PowerShell,
            "PowerShell",
            which::which("pwsh").ok(),
            Some(
                xdg.join("powershell")
                    .join("Microsoft.PowerShell_profile.ps1"),
            ),
            Some(Syntax::PowerShell),
        ),
    ];

    let login = std::env::var("SHELL").unwrap_or_default();
    let login_name = Path::new(&login)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    for s in &mut shells {
        let exe = match s.kind {
            ShellKind::Bash => "bash",
            ShellKind::Zsh => "zsh",
            ShellKind::Fish => "fish",
            ShellKind::Nushell => "nu",
            ShellKind::PowerShell => "pwsh",
            _ => "",
        };
        s.is_default = !exe.is_empty() && login_name == exe;
    }
    shells
}

fn detect_windows() -> Vec<DetectedShell> {
    let home = home();
    // dirs::document_dir() uses the Known Folder API, so OneDrive-redirected
    // Documents folders resolve correctly — which is where $PROFILE lives.
    let docs = dirs::document_dir().unwrap_or_else(|| home.join("Documents"));

    let comspec = std::env::var_os("ComSpec")
        .map(PathBuf::from)
        .or_else(|| which::which("cmd").ok());

    let mut shells = vec![
        shell(
            ShellKind::PowerShell,
            "PowerShell 7",
            which::which("pwsh").ok(),
            Some(
                docs.join("PowerShell")
                    .join("Microsoft.PowerShell_profile.ps1"),
            ),
            Some(Syntax::PowerShell),
        ),
        shell(
            ShellKind::WindowsPowerShell,
            "Windows PowerShell",
            which::which("powershell").ok(),
            Some(
                docs.join("WindowsPowerShell")
                    .join("Microsoft.PowerShell_profile.ps1"),
            ),
            Some(Syntax::PowerShell),
        ),
        shell(ShellKind::Cmd, "Command Prompt", comspec, None, None),
        shell(
            ShellKind::GitBash,
            "Git Bash",
            find_git_bash(),
            Some(home.join(".bashrc")),
            Some(Syntax::Posix),
        ),
        shell(
            ShellKind::Nushell,
            "Nushell",
            which::which("nu").ok(),
            nushell_config(),
            Some(Syntax::Nushell),
        ),
    ];

    let preferred = if shells[0].executable.is_some() { 0 } else { 1 };
    shells[preferred].is_default = true;
    shells
}

fn find_git_bash() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    for var in ["ProgramFiles", "ProgramW6432", "ProgramFiles(x86)"] {
        if let Some(p) = std::env::var_os(var) {
            candidates.push(PathBuf::from(p).join("Git").join("bin").join("bash.exe"));
        }
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        candidates.push(
            PathBuf::from(local)
                .join("Programs")
                .join("Git")
                .join("bin")
                .join("bash.exe"),
        );
    }
    // git.exe normally lives in <Git>\cmd\git.exe
    if let Ok(git) = which::which("git") {
        if let Some(root) = git.parent().and_then(Path::parent) {
            candidates.push(root.join("bin").join("bash.exe"));
        }
    }
    candidates.into_iter().find(|p| p.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn syntax_guess() {
        assert_eq!(syntax_for_path(Path::new("/x/config.fish")), Syntax::Fish);
        assert_eq!(
            syntax_for_path(Path::new("C:/p/Profile.PS1")),
            Syntax::PowerShell
        );
        assert_eq!(syntax_for_path(Path::new("/x/env.nu")), Syntax::Nushell);
        assert_eq!(syntax_for_path(Path::new("/x/.zshrc")), Syntax::Posix);
    }

    #[test]
    fn detect_does_not_panic() {
        let _ = detect();
    }
}
