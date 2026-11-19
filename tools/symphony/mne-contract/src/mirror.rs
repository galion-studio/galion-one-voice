// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! JSON and TOML mirrors. Lossless on the decoded document.
//! Whitespace and key order are not part of the contract.

use crate::binary;
use crate::model::Document;
use crate::validate;
use std::path::Path;

pub fn from_json(bytes: &[u8]) -> Result<Document, String> {
    serde_json::from_slice(bytes).map_err(|e| format!("json: {e}"))
}

pub fn to_json(doc: &Document) -> Result<Vec<u8>, String> {
    validate(doc)?;
    let mut bytes = serde_json::to_vec_pretty(doc).map_err(|e| format!("json: {e}"))?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub fn from_toml(bytes: &[u8]) -> Result<Document, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "toml is not utf-8".to_string())?;
    toml::from_str(text).map_err(|e| format!("toml: {e}"))
}

pub fn to_toml(doc: &Document) -> Result<Vec<u8>, String> {
    validate(doc)?;
    let text = toml::to_string_pretty(doc).map_err(|e| format!("toml: {e}"))?;
    Ok(text.into_bytes())
}

pub fn load_path(path: &Path) -> Result<Document, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let doc = parse_bytes(path, &bytes)?;
    validate(&doc)?;
    Ok(doc)
}

pub fn parse_bytes(path: &Path, bytes: &[u8]) -> Result<Document, String> {
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
    match ext {
        "json" => from_json(bytes),
        "toml" => from_toml(bytes),
        "mne" | "bin" => binary::decode(bytes),
        _ if bytes.starts_with(b"GMN1") => binary::decode(bytes),
        _ if bytes.first() == Some(&b'{') => from_json(bytes),
        _ => from_toml(bytes),
    }
}

pub fn store_path(path: &Path, doc: &Document) -> Result<(), String> {
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
    let bytes = match ext {
        "json" => to_json(doc)?,
        "toml" => to_toml(doc)?,
        "mne" | "bin" => binary::encode(doc)?,
        _ => return Err(format!("unknown output extension .{ext}")),
    };
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
        }
    }
    std::fs::write(path, bytes).map_err(|e| format!("write {}: {e}", path.display()))
}

pub fn load_dir(dir: &Path) -> Result<Vec<(std::path::PathBuf, Document)>, String> {
    let mut paths = Vec::new();
    let rd = std::fs::read_dir(dir).map_err(|e| format!("read dir {}: {e}", dir.display()))?;
    for ent in rd {
        let ent = ent.map_err(|e| format!("read dir {}: {e}", dir.display()))?;
        let path = ent.path();
        if path.extension().and_then(|s| s.to_str()) == Some("toml") {
            paths.push(path);
        }
    }
    paths.sort();
    let mut out = Vec::with_capacity(paths.len());
    for path in paths {
        let doc = load_path(&path)?;
        out.push((path, doc));
    }
    Ok(out)
}
