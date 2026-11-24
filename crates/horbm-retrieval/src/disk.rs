// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Read-only stamps. Phase 2 never creates a database file.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::error::{Error, Result};

/// What happened to a path this call was not allowed to create.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiskStatus {
    /// The caller did not name a path.
    Unset,
    /// The path was absent before the call and is still absent.
    NotCreated,
    /// The path already existed and its length and mtime did not change.
    Unchanged,
}

impl DiskStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unset => "unset",
            Self::NotCreated => "not created",
            Self::Unchanged => "unchanged",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Stamp {
    path: PathBuf,
    len: Option<u64>,
    modified: Option<SystemTime>,
}

impl Stamp {
    pub(crate) fn capture(path: &Path) -> Self {
        match fs::metadata(path) {
            Ok(meta) => Self {
                path: path.to_path_buf(),
                len: Some(meta.len()),
                modified: meta.modified().ok(),
            },
            Err(_) => Self {
                path: path.to_path_buf(),
                len: None,
                modified: None,
            },
        }
    }

    pub(crate) fn check(&self) -> Result<DiskStatus> {
        let after = Stamp::capture(&self.path);
        match (self.len, after.len) {
            (None, None) => Ok(DiskStatus::NotCreated),
            (Some(before_len), Some(after_len))
                if before_len == after_len && self.modified == after.modified =>
            {
                Ok(DiskStatus::Unchanged)
            }
            _ => Err(Error::message(format!(
                "path changed during retrieval: {}",
                self.path.display()
            ))),
        }
    }
}

pub(crate) fn is_sqlite_artifact(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n == "revisions.sqlite"
        || n.ends_with(".sqlite")
        || n.ends_with(".sqlite-wal")
        || n.ends_with(".sqlite-shm")
        || n.ends_with(".db")
        || n.ends_with(".db-wal")
        || n.ends_with(".db-shm")
}

/// Relative paths of sqlite artifacts under `dir`. Read only.
pub(crate) fn artifact_rel_paths(dir: &Path) -> Result<Vec<String>> {
    let mut out = Vec::new();
    if dir.is_dir() {
        walk(dir, dir, &mut out)?;
    }
    out.sort();
    Ok(out)
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) -> Result<()> {
    let entries = fs::read_dir(dir)?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            walk(root, &path, out)?;
            continue;
        }
        if is_sqlite_artifact(&name) {
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            out.push(rel);
        }
    }
    Ok(())
}

pub(crate) fn artifacts_unchanged(dir: &Path, before: &[String]) -> Result<()> {
    let after = artifact_rel_paths(dir)?;
    if after.as_slice() == before {
        Ok(())
    } else {
        Err(Error::message(format!(
            "sqlite artifacts under {} changed from {before:?} to {after:?}",
            dir.display()
        )))
    }
}
