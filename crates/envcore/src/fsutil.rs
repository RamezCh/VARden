//! Small helpers for editing user config files without surprising anyone.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::{Error, Result};

const UTF8_BOM: &str = "\u{feff}";

/// A text file's content plus the formatting details we must preserve
/// when writing it back (Windows PowerShell 5.1 profiles often carry a BOM
/// and CRLF line endings).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextFile {
    /// Content with the BOM stripped and line endings normalised to `\n`.
    pub content: String,
    pub bom: bool,
    pub crlf: bool,
}

impl TextFile {
    pub fn parse(raw: &str) -> Self {
        let (bom, body) = match raw.strip_prefix(UTF8_BOM) {
            Some(rest) => (true, rest),
            None => (false, raw),
        };
        let crlf = body.contains("\r\n");
        TextFile {
            content: if crlf {
                body.replace("\r\n", "\n")
            } else {
                body.to_string()
            },
            bom,
            crlf,
        }
    }

    pub fn render(&self) -> String {
        let mut out = String::with_capacity(self.content.len() + 8);
        if self.bom {
            out.push_str(UTF8_BOM);
        }
        if self.crlf {
            out.push_str(&self.content.replace('\n', "\r\n"));
        } else {
            out.push_str(&self.content);
        }
        out
    }
}

/// Read a text file. A missing file is treated as empty (we may be creating
/// `~/.zshrc` for the first time), using the platform's native line ending.
pub fn read_text(path: &Path) -> Result<TextFile> {
    match fs::read_to_string(path) {
        Ok(raw) => Ok(TextFile::parse(&raw)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(TextFile {
            content: String::new(),
            bom: false,
            crlf: cfg!(windows),
        }),
        Err(e) => Err(Error::io(path, e)),
    }
}

/// Follow symlinks so that dotfile managers (stow, chezmoi, yadm…) keep
/// working: we must edit the link *target*, never replace the link itself.
pub fn resolve_target(path: &Path) -> PathBuf {
    match fs::canonicalize(path) {
        Ok(p) => p,
        Err(_) => path.to_path_buf(), // doesn't exist yet
    }
}

/// Write via a sibling temp file + rename so a crash never leaves a
/// half-written rc file behind.
pub fn write_atomic(path: &Path, data: &[u8]) -> Result<()> {
    let target = resolve_target(path);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
    }
    let file_name = target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".into());
    let tmp = target.with_file_name(format!(".{file_name}.varden-tmp"));

    let result = (|| {
        let mut f = fs::File::create(&tmp).map_err(|e| Error::io(&tmp, e))?;
        f.write_all(data).map_err(|e| Error::io(&tmp, e))?;
        f.sync_all().map_err(|e| Error::io(&tmp, e))?;
        drop(f);
        // Keep the original's permissions (e.g. 0600 on a secrets file).
        if let Ok(meta) = fs::metadata(&target) {
            let _ = fs::set_permissions(&tmp, meta.permissions());
        }
        fs::rename(&tmp, &target).map_err(|e| Error::io(&target, e))
    })();

    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_bom_crlf() {
        let raw = "\u{feff}a\r\nb\r\n";
        let tf = TextFile::parse(raw);
        assert!(tf.bom && tf.crlf);
        assert_eq!(tf.content, "a\nb\n");
        assert_eq!(tf.render(), raw);
    }

    #[cfg(unix)]
    #[test]
    fn writes_through_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real_zshrc");
        let link = dir.path().join(".zshrc");
        fs::write(&real, "old").unwrap();
        std::os::unix::fs::symlink(&real, &link).unwrap();
        write_atomic(&link, b"new").unwrap();
        assert!(fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(fs::read_to_string(&real).unwrap(), "new");
    }
}
