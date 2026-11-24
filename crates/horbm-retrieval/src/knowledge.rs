// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Read-only loader for the galion-knowledge card pack.
//!
//! Layout written by `scripts/windows/Sync-HorbmKnowledge.ps1`: `manifest.json`
//! plus `world/`, `horbm/`, and `diary/` card files. The manifest is
//! `galion-shard-manifest/1`. `core_pack` carries payload `card-bundle/1`.
//! The digest is the one in `v2/docs/SHARDS.md` and `horbm` `cards.rs`:
//! for each `*.json` except `manifest.json`, `catalog.json`, and `schema.json`,
//! in lexicographic relative-path order, hash
//! `<relative path>\n<raw bytes>\n`.
//!
//! This module does not create directories and does not open `Stack`.
//! `Stack::open` would create `revisions.sqlite`.

use std::collections::HashSet;
use std::fs::File;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::error::{Error, Result};
use crate::fts::{row_from_card, FtsIndex};

const MANIFEST_SCHEMA: &str = "galion-shard-manifest/1";
const CARD_BUNDLE: &str = "card-bundle/1";
const MAX_CARD_BYTES: u64 = 256 * 1024;

/// Laptop knowledge directory. Used only when that directory already exists.
pub const WINDOWS_KNOWLEDGE_DIR: &str = r"G:\galion\horbm\knowledge";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadReport {
    pub cards: usize,
    /// `absent` or `sha256 ok`.
    pub manifest: String,
    pub path: PathBuf,
}

struct RawFile {
    rel: String,
    bytes: Vec<u8>,
}

pub(crate) fn load_into(index: &mut FtsIndex, dir: &Path) -> Result<LoadReport> {
    if !dir.is_dir() {
        return Err(Error::message(format!(
            "knowledge directory is missing: {}",
            dir.display()
        )));
    }
    let files = collect(dir)?;
    let (hash, nbytes) = bundle_digest(&files);
    let mut rows = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for file in &files {
        let text = String::from_utf8(file.bytes.clone())
            .map_err(|_| Error::message(format!("{} is not utf-8", file.rel)))?;
        let value: Value = serde_json::from_str(&text)?;
        let cards = horbm_memory::parse_cards(&value, &file.rel)?;
        for card in cards {
            if !seen.insert(card.id.clone()) {
                return Err(Error::message(format!(
                    "duplicate card id {} in {}",
                    card.id, file.rel
                )));
            }
            rows.push(row_from_card(&card));
        }
    }
    let manifest = check_manifest(dir, &hash, nbytes, rows.len())?;
    if rows.is_empty() {
        return Err(Error::message(format!("no cards in {}", dir.display())));
    }
    index.insert_rows(&rows)?;
    Ok(LoadReport {
        cards: rows.len(),
        manifest,
        path: dir.to_path_buf(),
    })
}

/// Where the pack lives. A set variable that is not a directory is an error,
/// not a fallback. The Windows path is used only when it is already a directory.
pub fn resolve_knowledge_home(get: &dyn Fn(&str) -> Option<String>) -> Result<Option<PathBuf>> {
    for key in ["GALION_KNOWLEDGE_HOME", "GALION_HORBM_KNOWLEDGE"] {
        if let Some(raw) = get(key) {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                continue;
            }
            let path = PathBuf::from(trimmed);
            if path.is_dir() {
                return Ok(Some(path));
            }
            return Err(Error::message(format!(
                "{key} is not a directory: {trimmed}"
            )));
        }
    }
    let windows = PathBuf::from(WINDOWS_KNOWLEDGE_DIR);
    if windows.is_dir() {
        return Ok(Some(windows));
    }
    Ok(None)
}

pub fn memory_db_path(get: &dyn Fn(&str) -> Option<String>) -> Option<PathBuf> {
    let raw = get("HORBM_MEMORY_DB")?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(PathBuf::from(trimmed))
    }
}

fn check_manifest(dir: &Path, hash: &str, nbytes: u64, cards: usize) -> Result<String> {
    let path = dir.join("manifest.json");
    if !path.is_file() {
        return Ok("absent".to_string());
    }
    let mut file = File::open(&path)?;
    let mut raw = String::new();
    file.read_to_string(&mut raw)?;
    let value: Value = serde_json::from_str(&raw)?;
    let obj = value
        .as_object()
        .ok_or_else(|| Error::message(format!("{}: manifest must be an object", path.display())))?;
    let schema = obj.get("schema").and_then(|v| v.as_str()).unwrap_or("");
    if schema != MANIFEST_SCHEMA {
        return Err(Error::message(format!(
            "{}: schema is {schema:?}, expected {MANIFEST_SCHEMA}",
            path.display()
        )));
    }
    let core = obj
        .get("core_pack")
        .and_then(|v| v.as_object())
        .ok_or_else(|| Error::message(format!("{}: manifest has no core_pack", path.display())))?;
    let payload = core.get("payload").and_then(|v| v.as_str()).unwrap_or("");
    if payload != CARD_BUNDLE {
        return Err(Error::message(format!(
            "{}: core_pack.payload is {payload:?}, expected {CARD_BUNDLE}",
            path.display()
        )));
    }
    let sha = core
        .get("sha256")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    let bytes = core
        .get("bytes")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| Error::message(format!("{}: core_pack.bytes is missing", path.display())))?;
    let card_count = core
        .get("card_count")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| {
            Error::message(format!(
                "{}: core_pack.card_count is missing",
                path.display()
            ))
        })?;
    if sha != hash || bytes != nbytes {
        return Err(Error::message(format!(
            "card-bundle sha256 mismatch for {}: got {hash} ({nbytes} bytes), manifest {sha} ({bytes} bytes)",
            dir.display()
        )));
    }
    if card_count as usize != cards {
        return Err(Error::message(format!(
            "manifest card_count is {card_count}, loader indexed {cards}"
        )));
    }
    Ok("sha256 ok".to_string())
}

fn bundle_digest(files: &[RawFile]) -> (String, u64) {
    let mut hasher = Sha256::new();
    let mut nbytes: u64 = 0;
    for file in files {
        let head = file.rel.as_bytes();
        hasher.update(head);
        hasher.update(b"\n");
        hasher.update(&file.bytes);
        hasher.update(b"\n");
        nbytes = nbytes
            .saturating_add(head.len() as u64)
            .saturating_add(1)
            .saturating_add(file.bytes.len() as u64)
            .saturating_add(1);
    }
    (hex_encode(&hasher.finalize()), nbytes)
}

fn hex_encode(digest: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0xf) as usize] as char);
    }
    out
}

fn collect(dir: &Path) -> Result<Vec<RawFile>> {
    let mut files = Vec::new();
    collect_dir(dir, dir, &mut files)?;
    files.sort_by(|a, b| a.rel.cmp(&b.rel));
    Ok(files)
}

fn collect_dir(root: &Path, dir: &Path, out: &mut Vec<RawFile>) -> Result<()> {
    let entries = std::fs::read_dir(dir)?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            collect_dir(root, &path, out)?;
            continue;
        }
        if !name.ends_with(".json") || skipped_sidecar(&name) {
            continue;
        }
        let meta = entry.metadata()?;
        if !meta.is_file() {
            continue;
        }
        if meta.len() > MAX_CARD_BYTES {
            return Err(Error::message(format!(
                "{} is larger than {MAX_CARD_BYTES} bytes",
                path.display()
            )));
        }
        let rel = rel_path(root, &path)?;
        let mut file = File::open(&path)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        out.push(RawFile { rel, bytes });
    }
    Ok(())
}

fn skipped_sidecar(name: &str) -> bool {
    matches!(name, "manifest.json" | "catalog.json" | "schema.json")
}

fn rel_path(root: &Path, path: &Path) -> Result<String> {
    let rel = path.strip_prefix(root).map_err(|_| {
        Error::message(format!(
            "path {} is outside {}",
            path.display(),
            root.display()
        ))
    })?;
    let mut parts = Vec::new();
    for component in rel.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_string_lossy().into_owned()),
            Component::CurDir => {}
            _ => {
                return Err(Error::message(format!(
                    "bad relative path {}",
                    path.display()
                )))
            }
        }
    }
    Ok(parts.join("/"))
}
