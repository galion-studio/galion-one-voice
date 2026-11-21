// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Paths. On the owner's Windows laptop the only drive is G:.

use std::path::PathBuf;

/// Home used when `GALION_HOME` is unset on Windows.
pub const WINDOWS_GALION_HOME: &str = "G:\\galion";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathReject {
    Empty,
    NotG,
}

impl std::fmt::Display for PathReject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "empty path"),
            Self::NotG => write!(f, "windows path must be on G:"),
        }
    }
}

impl std::error::Error for PathReject {}

pub fn windows_galion_home() -> &'static str {
    WINDOWS_GALION_HOME
}

/// A Windows path with a drive letter is accepted only on `G:`.
/// A path with no drive letter is left to the caller.
pub fn require_g_drive(path: &str) -> Result<(), PathReject> {
    let b = path.as_bytes();
    if b.is_empty() {
        return Err(PathReject::Empty);
    }
    if b.len() < 3 || b[1] != b':' || !matches!(b[2], b'\\' | b'/') {
        return Err(PathReject::NotG);
    }
    if !b[0].eq_ignore_ascii_case(&b'G') {
        return Err(PathReject::NotG);
    }
    reject_foreign_windows_drive(path)
}

/// Reject a `C:` path anywhere in the string. A `G:` path is kept.
/// A path with no drive letter passes.
pub fn reject_foreign_windows_drive(path: &str) -> Result<(), PathReject> {
    if path.is_empty() {
        return Err(PathReject::Empty);
    }
    let b = path.as_bytes();
    if b.len() >= 2
        && b[1] == b':'
        && b[0].is_ascii_alphabetic()
        && !b[0].eq_ignore_ascii_case(&b'G')
    {
        return Err(PathReject::NotG);
    }
    let lower = path.to_ascii_lowercase();
    let c_back = format!("{}{}", "c:", "\\");
    let c_fwd = format!("{}{}", "c:", "/");
    if lower.contains(&c_back) || lower.contains(&c_fwd) {
        return Err(PathReject::NotG);
    }
    Ok(())
}

/// `GALION_HOME` when it is set and, on Windows or when it uses a drive
/// letter, only if it is on `G:`. Otherwise the Windows constant, or the
/// XDG / home data directory on other hosts. This function does not create
/// the directory and does not read it.
pub fn galion_home() -> Result<PathBuf, PathReject> {
    if let Ok(value) = std::env::var("GALION_HOME") {
        if !value.is_empty() {
            if cfg!(windows) {
                require_g_drive(&value)?;
            } else {
                reject_foreign_windows_drive(&value)?;
            }
            return Ok(PathBuf::from(value));
        }
    }
    if cfg!(windows) {
        Ok(PathBuf::from(WINDOWS_GALION_HOME))
    } else {
        Ok(unix_home())
    }
}

fn unix_home() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        if !xdg.is_empty() {
            return PathBuf::from(xdg).join("galion");
        }
    }
    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() => PathBuf::from(home).join(".local/share/galion"),
        _ => PathBuf::from(".local/share/galion"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    fn drive(letter: u8) -> String {
        format!("{}:\\galion", letter as char)
    }

    #[test]
    fn the_owner_windows_home_is_g_and_other_drives_are_refused() {
        let home = windows_galion_home();
        assert!(require_g_drive(home).is_ok());
        assert!(home.starts_with("G:\\"));
        assert!(require_g_drive("G:/galion/memory").is_ok());
        assert!(require_g_drive(&drive(b'G')).is_ok());
        assert!(require_g_drive(&drive(b'C')).is_err());
        assert!(require_g_drive(&drive(b'D')).is_err());
        assert_eq!(require_g_drive(""), Err(PathReject::Empty));
        assert!(reject_foreign_windows_drive(&drive(b'C')).is_err());
    }

    #[test]
    fn crate_texts_do_not_name_a_foreign_drive() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut bad = Vec::new();
        visit(root, &mut bad);
        assert!(bad.is_empty(), "{bad:?}");
    }

    fn visit(dir: &Path, bad: &mut Vec<String>) {
        let entries =
            fs::read_dir(dir).unwrap_or_else(|err| panic!("read {}: {err}", dir.display()));
        for entry in entries {
            let entry = entry.unwrap();
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name == "target" || name.starts_with('.') {
                continue;
            }
            if path.is_dir() {
                visit(&path, bad);
                continue;
            }
            let keep = matches!(
                path.extension().and_then(|e| e.to_str()),
                Some("rs" | "md" | "toml" | "tsv" | "yml")
            );
            if !keep {
                continue;
            }
            let text = fs::read_to_string(&path).unwrap_or_default();
            if reject_foreign_windows_drive(&text).is_err() && text_has_foreign(&text) {
                bad.push(path.display().to_string());
            }
        }
    }

    fn text_has_foreign(text: &str) -> bool {
        let lower = text.to_ascii_lowercase();
        let c_back = format!("{}{}", "c:", "\\");
        let c_fwd = format!("{}{}", "c:", "/");
        lower.contains(&c_back) || lower.contains(&c_fwd)
    }
}
