// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Read-only loader for galion-knowledge cards.
//!
//! Card authoring is paused. This module reads cards. It does not write a
//! card, create a directory, or open `horbm-memory`. That crate's
//! `Stack::open` creates `revisions.sqlite`. The spoken match is the covering
//! lookup already used on the 7881 path: every content token of the query is
//! in the card, and the smallest unique haystack wins. A tie is not a guess.
//! That is the coverage gate `Stack::query` applies after its FTS5 leg. This
//! loader does not build that sqlite file.
//!
//! The field rules follow `v2/memory/src/card.rs` and `v2/data/schema.json`
//! in `galion-studio/galion-knowledge`. The optional manifest is
//! `galion-shard-manifest/1` with payload `card-bundle/1`, hashed the way
//! `v2/docs/SHARDS.md` describes.

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{CardHit, CardLookup};

/// Laptop knowledge directory. Used when it already exists and
/// `GALION_HORBM_KNOWLEDGE` is unset or not a directory. Nothing is created.
pub const WINDOWS_KNOWLEDGE_DIR: &str = r"G:\galion\horbm\knowledge";

const MANIFEST_SCHEMA: &str = "galion-shard-manifest/1";
const CARD_BUNDLE: &str = "card-bundle/1";
const MAX_CARD_BYTES: u64 = 256 * 1024;

/// `GALION_HORBM_KNOWLEDGE` when the value is a directory that already exists.
/// An empty or missing path is `None`. The directory is not created.
pub fn knowledge_dir_from_env(env_value: Option<&str>) -> Option<PathBuf> {
    let raw = env_value?.trim();
    if raw.is_empty() {
        return None;
    }
    let path = PathBuf::from(raw);
    if path.is_dir() {
        Some(path)
    } else {
        None
    }
}

/// What one load measured. Callers print [`LoadReport::log_line`] at startup.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadReport {
    pub cards: usize,
    pub elapsed: Duration,
    pub path: PathBuf,
    /// `absent`, `sha256 ok`, or `rejected`.
    pub manifest: String,
}

impl LoadReport {
    pub fn log_line(&self) -> String {
        let word = if self.cards == 1 { "card" } else { "cards" };
        format!(
            "horbm cards: loaded {} {word} in {:.3} ms from {} (lookup index, manifest {})",
            self.cards,
            self.elapsed.as_secs_f64() * 1000.0,
            self.path.display(),
            self.manifest
        )
    }
}

/// Cards plus the startup measurement.
#[derive(Clone, Debug)]
pub struct LoadedPack {
    pub cards: KnowledgeCards,
    pub report: LoadReport,
}

/// In-memory covering index. Built once at load. Not written back.
#[derive(Clone, Debug, Default)]
pub struct KnowledgeCards {
    cards: Vec<Indexed>,
    postings: HashMap<String, Vec<u32>>,
}

#[derive(Clone, Debug)]
struct Indexed {
    id: String,
    gist: String,
    n_tokens: usize,
}

impl KnowledgeCards {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.cards.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }

    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.cards.iter().map(|card| card.id.as_str())
    }

    /// Read `dir` if it is already a directory. Does not create it and does
    /// not write beside it. A manifest next to the pack is checked when one
    /// is present. JSON that is not a card (`manifest.json`, a catalog with
    /// no `id`) is not indexed.
    pub fn load(dir: &Path) -> Result<LoadedPack, CardError> {
        let started = Instant::now();
        if !dir.is_dir() {
            return Err(CardError::NotADirectory(dir.display().to_string()));
        }
        let mut files = Vec::new();
        collect_json(dir, dir, &mut files)?;
        files.sort_by(|a, b| a.rel.cmp(&b.rel));
        let manifest = read_manifest(dir)?;
        let manifest_state = match &manifest {
            Some(manifest) => {
                let (hash, bytes) = bundle_digest(&files);
                if hash != manifest.sha256 || bytes != manifest.bytes {
                    return Err(CardError::Invalid(format!(
                        "card-bundle sha256 mismatch for {}: got {hash} ({bytes} bytes), manifest {} ({} bytes)",
                        dir.display(),
                        manifest.sha256,
                        manifest.bytes
                    )));
                }
                "sha256 ok".to_string()
            }
            None => "absent".to_string(),
        };
        let mut parsed = Vec::new();
        for file in &files {
            parsed.extend(cards_in_file(file)?);
        }
        if let Some(manifest) = &manifest {
            if parsed.len() != manifest.card_count {
                return Err(CardError::Invalid(format!(
                    "manifest card_count is {}, loader indexed {}",
                    manifest.card_count,
                    parsed.len()
                )));
            }
        }
        let mut seen = HashMap::<String, String>::new();
        for card in &parsed {
            if let Some(prev) = seen.insert(card.id.clone(), card.origin.clone()) {
                return Err(CardError::Invalid(format!(
                    "duplicate card id {} in {} and {}",
                    card.id, prev, card.origin
                )));
            }
        }
        parsed.sort_by(|a, b| a.id.cmp(&b.id));
        let cards = KnowledgeCards::index(parsed);
        let report = LoadReport {
            cards: cards.len(),
            elapsed: started.elapsed(),
            path: dir.to_path_buf(),
            manifest: manifest_state,
        };
        Ok(LoadedPack { cards, report })
    }

    fn index(parsed: Vec<Parsed>) -> Self {
        let mut cards = Vec::with_capacity(parsed.len());
        let mut postings: HashMap<String, Vec<u32>> = HashMap::new();
        for card in parsed {
            let idx = u32::try_from(cards.len()).unwrap_or(u32::MAX);
            let tokens = haystack_tokens(&card);
            let n_tokens = tokens.len();
            for token in tokens {
                postings.entry(token).or_default().push(idx);
            }
            cards.push(Indexed {
                id: card.id,
                gist: card.gist,
                n_tokens,
            });
        }
        Self { cards, postings }
    }
}

impl CardLookup for KnowledgeCards {
    fn lookup(&self, query: &str) -> Option<CardHit> {
        let terms = index_tokens(query);
        if terms.is_empty() {
            return None;
        }
        let mut lists: Vec<&[u32]> = Vec::with_capacity(terms.len());
        for term in &terms {
            match self.postings.get(term) {
                Some(list) if !list.is_empty() => lists.push(list),
                _ => return None,
            }
        }
        lists.sort_by_key(|list| list.len());
        let mut hits = lists[0].to_vec();
        for list in lists.iter().skip(1) {
            hits.retain(|idx| list.binary_search(idx).is_ok());
            if hits.is_empty() {
                return None;
            }
        }
        hits.sort_by(|&a, &b| {
            let left = &self.cards[a as usize];
            let right = &self.cards[b as usize];
            left.n_tokens
                .cmp(&right.n_tokens)
                .then_with(|| left.id.cmp(&right.id))
        });
        if hits.len() >= 2 {
            let left = &self.cards[hits[0] as usize];
            let right = &self.cards[hits[1] as usize];
            if left.n_tokens == right.n_tokens {
                return None;
            }
        }
        let card = &self.cards[hits[0] as usize];
        if card.gist.is_empty() {
            return None;
        }
        Some(CardHit {
            id: card.id.clone(),
            gist: card.gist.clone(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CardError {
    NotADirectory(String),
    Io(String),
    Invalid(String),
}

impl std::fmt::Display for CardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CardError::NotADirectory(path) => write!(f, "knowledge dir is not a directory: {path}"),
            CardError::Io(msg) => write!(f, "card read: {msg}"),
            CardError::Invalid(msg) => write!(f, "card: {msg}"),
        }
    }
}

impl std::error::Error for CardError {}

struct RawFile {
    rel: String,
    bytes: Vec<u8>,
}

struct Manifest {
    sha256: String,
    bytes: u64,
    card_count: usize,
}

struct Parsed {
    id: String,
    gist: String,
    origin: String,
    hay: String,
}

fn collect_json(root: &Path, dir: &Path, out: &mut Vec<RawFile>) -> Result<(), CardError> {
    let entries = std::fs::read_dir(dir).map_err(|err| CardError::Io(err.to_string()))?;
    for entry in entries {
        let entry = entry.map_err(|err| CardError::Io(err.to_string()))?;
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            collect_json(root, &path, out)?;
            continue;
        }
        if !name.ends_with(".json") || skipped_sidecar(&name) {
            continue;
        }
        let meta = entry
            .metadata()
            .map_err(|err| CardError::Io(err.to_string()))?;
        if !meta.is_file() {
            continue;
        }
        if meta.len() > MAX_CARD_BYTES {
            return Err(CardError::Invalid(format!(
                "{} is larger than {MAX_CARD_BYTES} bytes",
                path.display()
            )));
        }
        let rel = rel_path(root, &path);
        let mut file = File::open(&path).map_err(|err| CardError::Io(err.to_string()))?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .map_err(|err| CardError::Io(err.to_string()))?;
        out.push(RawFile { rel, bytes });
    }
    Ok(())
}

fn skipped_sidecar(name: &str) -> bool {
    matches!(name, "manifest.json" | "catalog.json" | "schema.json")
}

fn rel_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

fn bundle_digest(files: &[RawFile]) -> (String, u64) {
    let mut hasher = Sha256::new();
    let mut bytes: u64 = 0;
    for file in files {
        let head = file.rel.as_bytes();
        hasher.update(head);
        hasher.update(b"\n");
        hasher.update(&file.bytes);
        hasher.update(b"\n");
        bytes = bytes
            .saturating_add(head.len() as u64)
            .saturating_add(1)
            .saturating_add(file.bytes.len() as u64)
            .saturating_add(1);
    }
    (hex_encode(&hasher.finalize()), bytes)
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

fn read_manifest(dir: &Path) -> Result<Option<Manifest>, CardError> {
    let candidates = [
        dir.join("manifest.json"),
        dir.join("..").join("manifest.json"),
        dir.join("..")
            .join("..")
            .join("shards")
            .join("manifest.json"),
    ];
    let Some(path) = candidates.into_iter().find(|path| path.is_file()) else {
        return Ok(None);
    };
    let mut file = File::open(&path).map_err(|err| CardError::Io(err.to_string()))?;
    let mut raw = String::new();
    file.read_to_string(&mut raw)
        .map_err(|err| CardError::Io(err.to_string()))?;
    let value: Value = serde_json::from_str(&raw)
        .map_err(|err| CardError::Invalid(format!("{}: {err}", path.display())))?;
    let obj = value.as_object().ok_or_else(|| {
        CardError::Invalid(format!("{}: manifest must be an object", path.display()))
    })?;
    let schema = obj.get("schema").and_then(|v| v.as_str()).unwrap_or("");
    if schema != MANIFEST_SCHEMA {
        return Err(CardError::Invalid(format!(
            "{}: schema is {schema:?}, expected {MANIFEST_SCHEMA}",
            path.display()
        )));
    }
    let core = obj
        .get("core_pack")
        .and_then(|v| v.as_object())
        .ok_or_else(|| {
            CardError::Invalid(format!("{}: manifest has no core_pack", path.display()))
        })?;
    let payload = core.get("payload").and_then(|v| v.as_str()).unwrap_or("");
    if payload != CARD_BUNDLE {
        return Err(CardError::Invalid(format!(
            "{}: core_pack.payload is {payload:?}, expected {CARD_BUNDLE}",
            path.display()
        )));
    }
    let sha256 = core
        .get("sha256")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    if sha256.len() != 64 || !sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(CardError::Invalid(format!(
            "{}: core_pack.sha256 is not 64 hex digits",
            path.display()
        )));
    }
    let bytes = core.get("bytes").and_then(|v| v.as_u64()).ok_or_else(|| {
        CardError::Invalid(format!("{}: core_pack.bytes is missing", path.display()))
    })?;
    let card_count = core
        .get("card_count")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| {
            CardError::Invalid(format!(
                "{}: core_pack.card_count is missing",
                path.display()
            ))
        })?;
    let card_count = usize::try_from(card_count).unwrap_or(usize::MAX);
    Ok(Some(Manifest {
        sha256,
        bytes,
        card_count,
    }))
}

fn cards_in_file(file: &RawFile) -> Result<Vec<Parsed>, CardError> {
    let text = std::str::from_utf8(&file.bytes)
        .map_err(|_| CardError::Invalid(format!("{}: card is not utf-8", file.rel)))?;
    let value: Value = serde_json::from_str(text)
        .map_err(|err| CardError::Invalid(format!("{}: {err}", file.rel)))?;
    let single = value
        .as_object()
        .is_some_and(|obj| obj.get("cards").is_none());
    let mut out = Vec::new();
    push_cards(&value, file, single, &mut out)?;
    Ok(out)
}

fn push_cards(
    value: &Value,
    file: &RawFile,
    single: bool,
    out: &mut Vec<Parsed>,
) -> Result<(), CardError> {
    match value {
        Value::Array(items) => {
            for item in items {
                if let Some(card) = one_card(item, file, false)? {
                    out.push(card);
                }
            }
            Ok(())
        }
        Value::Object(map) => {
            if let Some(cards) = map.get("cards") {
                return push_cards(cards, file, false, out);
            }
            if let Some(card) = one_card(value, file, single)? {
                out.push(card);
            }
            Ok(())
        }
        _ => Err(CardError::Invalid(format!(
            "{}: card JSON must be an object or array",
            file.rel
        ))),
    }
}

fn one_card(value: &Value, file: &RawFile, check_name: bool) -> Result<Option<Parsed>, CardError> {
    let Some(obj) = value.as_object() else {
        return Err(CardError::Invalid(format!(
            "{}: expected a card object",
            file.rel
        )));
    };
    let Some(id) = first_str(obj, &["id", "card_id"]).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    if !valid_id(&id) {
        return Err(CardError::Invalid(format!(
            "{}: card id {id} is not a lowercase slug",
            file.rel
        )));
    }
    if check_name {
        let stem = file
            .rel
            .rsplit('/')
            .next()
            .unwrap_or(file.rel.as_str())
            .trim_end_matches(".json");
        if stem != id {
            return Err(CardError::Invalid(format!(
                "{}: file name does not match card id {id}",
                file.rel
            )));
        }
    }
    let title = first_str(obj, &["title"]).unwrap_or_default();
    if title.is_empty() {
        return Err(CardError::Invalid(format!("{id}: title is empty")));
    }
    let body = first_str(obj, &["body", "text"]).unwrap_or_default();
    if body.is_empty() {
        return Err(CardError::Invalid(format!("{id}: body is empty")));
    }
    let summary = first_str(obj, &["summary", "gist"]).unwrap_or_default();
    let section = resolve_section(obj, &file.rel).ok_or_else(|| {
        CardError::Invalid(format!("{id}: section must be world, horbm, or diary"))
    })?;
    if !matches!(section.as_str(), "world" | "horbm" | "diary") {
        return Err(CardError::Invalid(format!(
            "{id}: section must be world, horbm, or diary"
        )));
    }
    let category = first_str(obj, &["category"]).unwrap_or_default();
    if category.is_empty() {
        return Err(CardError::Invalid(format!("{id}: category is empty")));
    }
    let claim = map_claim(first_str(obj, &["claim_type", "epistemic"]).unwrap_or_default());
    if !matches!(
        claim.as_str(),
        "fact" | "interpretation" | "opinion" | "contested"
    ) {
        return Err(CardError::Invalid(format!(
            "{id}: claim_type {claim:?} is not fact, interpretation, opinion, or contested"
        )));
    }
    let evidence =
        map_evidence(first_str(obj, &["evidence_strength", "evidence"]).unwrap_or_default());
    if !matches!(evidence.as_str(), "none" | "weak" | "moderate" | "strong") {
        return Err(CardError::Invalid(format!(
            "{id}: evidence_strength {evidence:?} is not none, weak, moderate, or strong"
        )));
    }
    if obj.get("said_by").is_none() && obj.get("speaker").is_none() && obj.get("author").is_none() {
        return Err(CardError::Invalid(format!("{id}: said_by is missing")));
    }
    let as_of = first_str(obj, &["as_of_date", "date", "said_at"]).unwrap_or_default();
    if obj.get("as_of_date").is_none() && obj.get("date").is_none() && obj.get("said_at").is_none()
    {
        return Err(CardError::Invalid(format!("{id}: as_of_date is missing")));
    }
    if !valid_as_of(&as_of) {
        return Err(CardError::Invalid(format!(
            "{id}: as_of_date {as_of:?} is not a date"
        )));
    }
    let status = first_str(obj, &["status"]).unwrap_or_default();
    if status.is_empty() {
        return Err(CardError::Invalid(format!("{id}: status is empty")));
    }
    validate_sources(obj.get("sources"), &id)?;
    let views = validate_views(obj.get("contested_views"), &id)?;
    validate_strings(obj.get("related"), &id, "related")?;
    let aliases = string_list(obj.get("aliases"));
    let tags = string_list(obj.get("tags"));
    if let Some(value) = obj.get("read_time_min") {
        if value.as_u64().filter(|n| *n >= 1).is_none() {
            return Err(CardError::Invalid(format!(
                "{id}: read_time_min must be a positive integer"
            )));
        }
    }
    let gist = if !summary.is_empty() {
        summary.clone()
    } else {
        summary_from_body(&body)
    };
    let mut hay = format!("{id}\n{title}\n{summary}\n{body}");
    for alias in &aliases {
        hay.push('\n');
        hay.push_str(alias);
    }
    for tag in &tags {
        hay.push('\n');
        hay.push_str(tag);
    }
    for (label, view_summary) in &views {
        hay.push('\n');
        hay.push_str(label);
        hay.push('\n');
        hay.push_str(view_summary);
    }
    Ok(Some(Parsed {
        id,
        gist,
        origin: file.rel.clone(),
        hay,
    }))
}

fn resolve_section(obj: &serde_json::Map<String, Value>, path: &str) -> Option<String> {
    let written = first_str(obj, &["section", "scope"]).unwrap_or_default();
    if matches!(written.as_str(), "world" | "horbm" | "diary") {
        return Some(written);
    }
    for part in path.split(['/', '\\']) {
        if matches!(part, "world" | "horbm" | "diary") {
            return Some(part.to_string());
        }
    }
    None
}

fn validate_sources(value: Option<&Value>, id: &str) -> Result<(), CardError> {
    let Some(Value::Array(items)) = value else {
        return Err(CardError::Invalid(format!(
            "{id}: sources must be an array"
        )));
    };
    for (index, item) in items.iter().enumerate() {
        let Some(obj) = item.as_object() else {
            return Err(CardError::Invalid(format!(
                "{id}: sources[{index}] must be an object"
            )));
        };
        for key in ["title", "url", "author", "date", "accessed"] {
            if obj.get(key).and_then(|v| v.as_str()).is_none() {
                return Err(CardError::Invalid(format!(
                    "{id}: sources[{index}].{key} must be a string"
                )));
            }
        }
        let title = obj.get("title").and_then(|v| v.as_str()).unwrap_or("");
        if title.trim().is_empty() {
            return Err(CardError::Invalid(format!(
                "{id}: sources[{index}].title is empty"
            )));
        }
        let url = obj.get("url").and_then(|v| v.as_str()).unwrap_or("");
        if !valid_url(url.trim()) {
            return Err(CardError::Invalid(format!(
                "{id}: sources[{index}].url is not an http(s) URL"
            )));
        }
        let accessed = obj.get("accessed").and_then(|v| v.as_str()).unwrap_or("");
        if !valid_accessed(accessed.trim()) {
            return Err(CardError::Invalid(format!(
                "{id}: sources[{index}].accessed is not a date"
            )));
        }
    }
    Ok(())
}

fn validate_views(value: Option<&Value>, id: &str) -> Result<Vec<(String, String)>, CardError> {
    let Some(Value::Array(items)) = value else {
        return Err(CardError::Invalid(format!(
            "{id}: contested_views must be an array"
        )));
    };
    let mut out = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let Some(obj) = item.as_object() else {
            return Err(CardError::Invalid(format!(
                "{id}: contested_views[{index}] must be an object"
            )));
        };
        let label = first_str(obj, &["label", "position"]).unwrap_or_default();
        let summary = first_str(obj, &["summary"]).unwrap_or_default();
        let status = first_str(obj, &["status"]).unwrap_or_default();
        if label.is_empty() || summary.is_empty() || status.is_empty() {
            return Err(CardError::Invalid(format!(
                "{id}: contested_views[{index}] needs label, summary, and status"
            )));
        }
        if obj.get("sources").is_some() {
            validate_sources(obj.get("sources"), id)?;
        }
        out.push((label, summary));
    }
    Ok(out)
}

fn validate_strings(value: Option<&Value>, id: &str, field: &str) -> Result<(), CardError> {
    let Some(Value::Array(items)) = value else {
        return Err(CardError::Invalid(format!(
            "{id}: {field} must be an array"
        )));
    };
    for (index, item) in items.iter().enumerate() {
        match item.as_str() {
            Some(text) if !text.trim().is_empty() => {}
            _ => {
                return Err(CardError::Invalid(format!(
                    "{id}: {field}[{index}] must be a non-empty string"
                )));
            }
        }
    }
    Ok(())
}

fn string_list(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.trim().to_string()))
            .filter(|s| !s.is_empty())
            .collect(),
        _ => Vec::new(),
    }
}

fn summary_from_body(body: &str) -> String {
    let trimmed = body.trim();
    let count = trimmed.chars().count();
    if count <= 480 {
        trimmed.to_string()
    } else {
        let truncated: String = trimmed.chars().take(480).collect();
        format!("{truncated}…")
    }
}

fn map_claim(raw: String) -> String {
    match raw.as_str() {
        "said" => "fact".to_string(),
        "inferred" => "interpretation".to_string(),
        other => other.to_string(),
    }
}

fn map_evidence(raw: String) -> String {
    match raw.as_str() {
        "unsourced" => "none".to_string(),
        "limited" | "design" => "weak".to_string(),
        "supported" | "not-settled" => "moderate".to_string(),
        "established" => "strong".to_string(),
        other => other.to_string(),
    }
}

fn first_str(obj: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(Value::String(s)) = obj.get(*key) {
            return Some(s.trim().to_string());
        }
    }
    None
}

fn valid_id(id: &str) -> bool {
    let bytes = id.as_bytes();
    if bytes.is_empty() {
        return false;
    }
    let mut after_sep = true;
    for &byte in bytes {
        let alnum = byte.is_ascii_lowercase() || byte.is_ascii_digit();
        if alnum {
            after_sep = false;
        } else if (byte == b'.' || byte == b'-') && !after_sep {
            after_sep = true;
        } else {
            return false;
        }
    }
    !after_sep
}

fn valid_as_of(value: &str) -> bool {
    if value.is_empty() {
        return true;
    }
    let value = value.strip_prefix('-').unwrap_or(value);
    let mut parts = value.split('-');
    let Some(year) = parts.next() else {
        return false;
    };
    if year.is_empty() || year.len() > 6 || !year.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    let mut extra = 0;
    for part in parts {
        extra += 1;
        if extra > 2 || part.len() != 2 || !part.bytes().all(|b| b.is_ascii_digit()) {
            return false;
        }
    }
    true
}

fn valid_accessed(value: &str) -> bool {
    if value.is_empty() {
        return true;
    }
    let bytes = value.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[..4].iter().all(|b| b.is_ascii_digit())
        && bytes[5..7].iter().all(|b| b.is_ascii_digit())
        && bytes[8..].iter().all(|b| b.is_ascii_digit())
}

fn valid_url(value: &str) -> bool {
    if value.is_empty() {
        return true;
    }
    let Some(rest) = value
        .strip_prefix("https://")
        .or_else(|| value.strip_prefix("http://"))
    else {
        return false;
    };
    !rest.is_empty() && !rest.chars().any(char::is_whitespace)
}

fn haystack_tokens(card: &Parsed) -> std::collections::BTreeSet<String> {
    index_tokens(&card.hay).into_iter().collect()
}

fn index_tokens(raw: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in raw.chars() {
        if ch.is_alphanumeric() {
            for lower in ch.to_lowercase() {
                cur.push(lower);
            }
        } else if !cur.is_empty() {
            push_token(&mut out, &mut cur);
        }
    }
    if !cur.is_empty() {
        push_token(&mut out, &mut cur);
    }
    out.into_iter()
        .filter(|token| !is_stopword(token))
        .collect()
}

fn push_token(out: &mut Vec<String>, cur: &mut String) {
    if cur.len() <= 64 {
        out.push(std::mem::take(cur));
    } else {
        cur.clear();
    }
}

fn is_stopword(term: &str) -> bool {
    matches!(
        term,
        "a" | "an"
            | "the"
            | "and"
            | "or"
            | "of"
            | "to"
            | "in"
            | "on"
            | "for"
            | "with"
            | "at"
            | "by"
            | "from"
            | "as"
            | "is"
            | "are"
            | "was"
            | "were"
            | "be"
            | "been"
            | "being"
            | "it"
            | "its"
            | "this"
            | "that"
            | "these"
            | "those"
            | "not"
            | "but"
            | "if"
            | "into"
            | "than"
            | "then"
            | "so"
            | "such"
            | "no"
            | "nor"
            | "too"
            | "very"
            | "can"
            | "will"
            | "just"
            | "about"
            | "over"
            | "after"
            | "before"
            | "between"
            | "out"
            | "up"
            | "down"
            | "off"
            | "own"
            | "same"
            | "other"
            | "which"
            | "who"
            | "whom"
            | "what"
            | "when"
            | "where"
            | "how"
            | "all"
            | "any"
            | "both"
            | "each"
            | "few"
            | "more"
            | "most"
            | "some"
            | "only"
            | "do"
            | "does"
            | "did"
            | "doing"
            | "have"
            | "has"
            | "had"
            | "having"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn real_card(id: &str, summary: &str, body: &str) -> String {
        format!(
            r#"{{
  "id": "{id}",
  "title": "{summary}",
  "section": "horbm",
  "category": "fixture",
  "summary": "{summary}",
  "body": "{body}",
  "claim_type": "fact",
  "sources": [{{
    "title": "Galion knowledge fixture",
    "url": "https://github.com/galion-studio/galion-knowledge",
    "author": "Galion Labs",
    "date": "2026-11-18",
    "accessed": "2026-11-22"
  }}],
  "evidence_strength": "strong",
  "said_by": "Galion Labs",
  "as_of_date": "2026-11-18",
  "status": "draft",
  "contested_views": [],
  "related": []
}}
"#
        )
    }

    fn temp_dir(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "horbm-cards-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::create_dir_all(path.join("horbm")).unwrap();
        path
    }

    #[test]
    fn missing_dir_is_not_created() {
        let missing = std::env::temp_dir().join(format!(
            "horbm-cards-missing-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        assert!(!missing.exists());
        let err = KnowledgeCards::load(&missing).unwrap_err();
        assert!(matches!(err, CardError::NotADirectory(_)));
        assert!(!missing.exists());
    }

    #[test]
    fn load_reads_real_cards_and_does_not_write() {
        let dir = temp_dir("ok");
        std::fs::write(
            dir.join("horbm").join("fixture-kettle.json"),
            real_card(
                "fixture-kettle",
                "The kettle is on the hob.",
                "The kettle is on the hob.",
            ),
        )
        .unwrap();
        std::fs::write(
            dir.join("horbm").join("fixture-gate.json"),
            real_card(
                "fixture-gate",
                "The gate latch sticks.",
                "The gate latch sticks on the left.",
            ),
        )
        .unwrap();
        let before = std::fs::read_dir(dir.join("horbm")).unwrap().count();
        let loaded = KnowledgeCards::load(&dir).unwrap();
        let after = std::fs::read_dir(dir.join("horbm")).unwrap().count();
        assert_eq!(before, after);
        assert!(!dir.join("revisions.sqlite").exists());
        assert_eq!(loaded.cards.len(), 2);
        assert_eq!(loaded.report.manifest, "absent");
        assert!(loaded.report.log_line().contains("loaded 2 cards"));
        assert!(loaded.report.log_line().contains("lookup index"));
        let hit = loaded.cards.lookup("where is the kettle").unwrap();
        assert_eq!(hit.id, "fixture-kettle");
        assert_eq!(hit.gist, "The kettle is on the hob.");
        assert!(loaded.cards.lookup("who won the match").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_bad_id_and_a_hash_mismatch_are_refused() {
        let dir = temp_dir("bad");
        std::fs::write(
            dir.join("horbm").join("Fixture-Kettle.json"),
            real_card(
                "Fixture-Kettle",
                "The kettle is on the hob.",
                "The kettle is on the hob.",
            ),
        )
        .unwrap();
        let err = KnowledgeCards::load(&dir).unwrap_err();
        assert!(err.to_string().contains("lowercase") || err.to_string().contains("file name"));
        let _ = std::fs::remove_dir_all(&dir);

        let dir = temp_dir("hash");
        let body = real_card(
            "fixture-kettle",
            "The kettle is on the hob.",
            "The kettle is on the hob.",
        );
        std::fs::write(dir.join("horbm").join("fixture-kettle.json"), &body).unwrap();
        std::fs::write(
            dir.join("manifest.json"),
            r#"{"schema":"galion-shard-manifest/1","core_pack":{"sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","bytes":1,"card_count":1,"payload":"card-bundle/1"}}"#,
        )
        .unwrap();
        let err = KnowledgeCards::load(&dir).unwrap_err();
        assert!(err.to_string().contains("sha256"));
        assert!(!dir.join("revisions.sqlite").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn env_dir_must_already_exist() {
        assert!(knowledge_dir_from_env(None).is_none());
        assert!(knowledge_dir_from_env(Some("  ")).is_none());
        assert!(knowledge_dir_from_env(Some("/no/such/horbm/knowledge")).is_none());
        let dir = temp_dir("env");
        let found = knowledge_dir_from_env(Some(dir.to_str().unwrap())).unwrap();
        assert_eq!(found, dir);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
