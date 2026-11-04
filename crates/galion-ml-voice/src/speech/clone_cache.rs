// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! On-disk cache of Chatterbox voice conditioning (owner item 10, 2026-11-04).
//!
//! The Nano worker used to recompute the conditioning (`model.conds`: speaker
//! embedding, prompt speech tokens, S3Gen prompt features) from the
//! reference wav at every worker start and kept it only in memory. This
//! module lets the clone slot save it once and reuse it. `.pt` (pickle) is
//! banned, so the format is **safetensors**: an 8-byte little-endian header
//! length, a JSON header, raw little-endian tensor bytes. No code runs on load.
//!
//! Layout, under the clip store (outside the git tree, like the refs):
//!
//! ```text
//! $GALION_CLIP_STORE/<persona>/cond/<model>-<key>.safetensors
//! $GALION_CLIP_STORE/<persona>/cond/<model>-<key>.safetensors.sha256
//! ```
//!
//! `<key>` is the first 32 hex of sha256 over [`CondKey::canonical`]:
//! format version, persona, model, reference wav sha256, a digest of the
//! weight pins (`scripts/clone/PINS.sha256` + `TOKENIZER.sha256`), and the
//! conditioning options. The same fields are written into the safetensors
//! `__metadata__` and checked again on load.
//!
//! Rules:
//! - **Write only after the consent gate.** [`plan_save`] needs the
//!   [`galion_consent::Approval`] for this persona *and* the same approval in
//!   the registry the gate writes; its sha256 must be the key's reference
//!   sha256. No approval, no file.
//! - **Invalidate on change.** A new reference sha256 or new pins give a new
//!   key, so the old file is never read. [`retire_stale`] renames the old
//!   entries to `*.off-<unix-seconds>` (nothing is deleted).
//! - **Verify before use.** [`lookup`] checks the sidecar sha256, parses the
//!   header with a strict reader (only F32 / I64 / I32, only the known tensor
//!   names, offsets contiguous and in bounds, no pickle) and compares the
//!   metadata with the key. A file that fails is renamed `*.off-<secs>` and
//!   the worker recomputes from the wav.
//! - The worker writes to a temp path the slot names. [`commit`] verifies it
//!   the same way, writes the sidecar, then renames it into place.

use super::sha256::{self, Sha256};
use galion_consent::{Approval, Registry, Verdict};
use std::path::{Path, PathBuf};

/// Bump when the tensor set or the metadata changes.
pub const FORMAT: &str = "galion-cond-v1";
/// Largest header accepted (the real one is about 1 KB).
pub const MAX_HEADER: u64 = 1 << 20;
/// Largest cache file accepted (the real one is a few hundred KB).
pub const MAX_FILE: u64 = 64 << 20;
/// Tensor names the Nano worker writes. Anything else is refused.
pub const TENSOR_NAMES: [&str; 10] = [
    "t3.speaker_emb",
    "t3.clap_emb",
    "t3.cond_prompt_speech_tokens",
    "t3.cond_prompt_speech_emb",
    "t3.emotion_adv",
    "gen.prompt_token",
    "gen.prompt_token_len",
    "gen.prompt_feat",
    "gen.prompt_feat_len",
    "gen.embedding",
];
/// Must be present for the conditioning to be usable.
pub const REQUIRED_TENSORS: [&str; 3] = ["t3.speaker_emb", "gen.prompt_feat", "gen.embedding"];

/// What identifies one conditioning result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CondKey {
    pub persona: &'static str,
    pub model: &'static str,
    pub ref_sha256: String,
    pub pins_sha256: String,
    /// `prepare_conditionals(exaggeration=…)` as the worker calls it.
    pub exaggeration: &'static str,
    pub norm_loudness: bool,
}

impl CondKey {
    /// The key the Nano worker uses today (`exaggeration=0.0`, `norm_loudness=True`).
    pub fn nano(persona: &'static str, ref_sha256: &str, pins_sha256: &str) -> Self {
        Self {
            persona,
            model: "nano",
            ref_sha256: ref_sha256.to_ascii_lowercase(),
            pins_sha256: pins_sha256.to_ascii_lowercase(),
            exaggeration: "0.0",
            norm_loudness: true,
        }
    }

    /// `(name, value)` pairs, written as safetensors `__metadata__`.
    pub fn metadata(&self) -> Vec<(&'static str, String)> {
        vec![
            ("format", FORMAT.to_string()),
            ("persona", self.persona.to_string()),
            ("model", self.model.to_string()),
            ("ref_sha256", self.ref_sha256.clone()),
            ("pins_sha256", self.pins_sha256.clone()),
            ("exaggeration", self.exaggeration.to_string()),
            ("norm_loudness", self.norm_loudness.to_string()),
        ]
    }

    pub fn canonical(&self) -> String {
        self.metadata().into_iter().map(|(k, v)| format!("{k}={v}\n")).collect()
    }

    pub fn id(&self) -> String {
        let mut h = Sha256::new();
        h.update(self.canonical().as_bytes());
        h.hex()[..32].to_string()
    }

    pub fn file_name(&self) -> String {
        format!("{}-{}.safetensors", self.model, self.id())
    }

    /// The `cond_meta` JSON object the worker copies into `__metadata__`.
    pub fn metadata_json(&self) -> String {
        let mut s = String::from("{");
        for (i, (k, v)) in self.metadata().into_iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            s.push_str(&format!("\"{k}\":\"{}\"", json_escape(&v)));
        }
        s.push('}');
        s
    }
}

fn is_hex64(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// sha256 over the weight pin tables, so new weights or a new tokenizer
/// invalidate every cached conditioning. A missing table hashes as `absent`.
pub fn pins_digest(repo: &Path) -> String {
    let mut h = Sha256::new();
    for name in ["PINS.sha256", "TOKENIZER.sha256"] {
        h.update(name.as_bytes());
        h.update(b"\n");
        match std::fs::read(repo.join("scripts/clone").join(name)) {
            Ok(bytes) => h.update(&bytes),
            Err(_) => h.update(b"absent"),
        }
        h.update(b"\n");
    }
    h.hex()
}

pub fn cond_dir(store: &Path, persona: &str) -> PathBuf {
    store.join(persona).join("cond")
}

pub fn entry_path(store: &Path, key: &CondKey) -> PathBuf {
    cond_dir(store, key.persona).join(key.file_name())
}

fn sidecar(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(".sha256");
    PathBuf::from(s)
}

fn unix_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Rename `path` (and its sidecar) to `*.off-<secs>`. Never deletes.
pub fn quarantine(path: &Path) -> Option<PathBuf> {
    let stamp = unix_secs();
    let mut n = 0u32;
    loop {
        let mut off = path.as_os_str().to_owned();
        off.push(if n == 0 { format!(".off-{stamp}") } else { format!(".off-{stamp}-{n}") });
        let off = PathBuf::from(off);
        if !off.exists() {
            std::fs::rename(path, &off).ok()?;
            let side = sidecar(path);
            if side.exists() {
                let mut so = off.as_os_str().to_owned();
                so.push(".sha256");
                let _ = std::fs::rename(side, PathBuf::from(so));
            }
            return Some(off);
        }
        n += 1;
    }
}

/// Rename every cached entry in the persona's cond dir that is not `key`'s
/// file (old reference sha256, old pins, old format). Returns the new names.
pub fn retire_stale(store: &Path, key: &CondKey) -> Vec<PathBuf> {
    let keep = key.file_name();
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(cond_dir(store, key.persona)) else { return out };
    let mut names: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
    names.sort();
    for p in names {
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
        if name.ends_with(".safetensors") && name != keep {
            if let Some(off) = quarantine(&p) {
                out.push(off);
            }
        }
    }
    out
}

#[derive(Clone, Debug, PartialEq)]
pub enum Lookup {
    /// Verified; send this path as `cond_load`.
    Hit(PathBuf),
    /// No file for this key.
    Miss,
    /// A file was there but failed a check; it was renamed aside.
    Invalid(String),
}

/// Find and verify the cached conditioning for `key`.
pub fn lookup(store: &Path, key: &CondKey) -> Lookup {
    let path = entry_path(store, key);
    if !path.is_file() {
        return Lookup::Miss;
    }
    match verify_file(&path, key, true) {
        Ok(()) => Lookup::Hit(path),
        Err(why) => {
            let _ = quarantine(&path);
            Lookup::Invalid(why)
        }
    }
}

/// Where the worker may write, for one approved key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaveTicket {
    pub key: CondKey,
    pub tmp: PathBuf,
    pub dest: PathBuf,
}

/// Name a temp path for the worker to write, only for a voice that passed
/// the consent gate with exactly this reference.
pub fn plan_save(store: &Path, key: &CondKey, approval: &Approval, reg: &Registry) -> Result<SaveTicket, String> {
    if approval.persona != key.persona {
        return Err(format!("approval is for {}, not {}", approval.persona, key.persona));
    }
    if !is_hex64(&key.ref_sha256) || approval.sha256.to_ascii_lowercase() != key.ref_sha256 {
        return Err("approval does not cover this reference sha256".into());
    }
    match reg.lookup(key.persona) {
        Some(Verdict::Approved(a)) if a == *approval => {}
        Some(Verdict::Approved(_)) => return Err("registry approval differs from the one presented".into()),
        Some(Verdict::Refused(r)) => return Err(format!("consent gate refused {} ({r})", key.persona)),
        None => return Err(format!("consent gate has not run for {}", key.persona)),
    }
    if !is_hex64(&key.pins_sha256) {
        return Err("pins digest is not a sha256".into());
    }
    let dir = cond_dir(store, key.persona);
    std::fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
    }
    let dest = dir.join(key.file_name());
    let tmp = dir.join(format!("{}.tmp-{}", key.file_name(), std::process::id()));
    Ok(SaveTicket { key: key.clone(), tmp, dest })
}

/// Verify the worker's temp file, write the sidecar, move it into place, and
/// retire stale entries. A file that fails is renamed aside, not used.
pub fn commit(ticket: &SaveTicket) -> Result<PathBuf, String> {
    if let Err(why) = verify_file(&ticket.tmp, &ticket.key, false) {
        let _ = quarantine(&ticket.tmp);
        return Err(why);
    }
    let hex = sha256::file_hex(&ticket.tmp).map_err(|e| format!("hash: {e}"))?;
    if ticket.dest.exists() {
        let _ = quarantine(&ticket.dest);
    }
    std::fs::rename(&ticket.tmp, &ticket.dest).map_err(|e| format!("rename: {e}"))?;
    let name = ticket.dest.file_name().and_then(|n| n.to_str()).unwrap_or("");
    std::fs::write(sidecar(&ticket.dest), format!("{hex}  {name}\n")).map_err(|e| format!("sidecar: {e}"))?;
    let store = ticket.dest.parent().and_then(Path::parent).and_then(Path::parent).unwrap_or(Path::new("."));
    retire_stale(store, &ticket.key);
    Ok(ticket.dest.clone())
}

/// Per-slot cache context: where the store is, the weight-pin digest, and
/// the approvals `galion_consent::clone_gate` returned at clone time.
#[derive(Clone, Debug, PartialEq)]
pub struct CondCtx {
    pub store: PathBuf,
    pub pins_sha256: String,
    pub approvals: Vec<Approval>,
}

/// What to ask the worker for on one request.
#[derive(Clone, Debug, PartialEq)]
pub enum CondPlan {
    /// Use the verified cached conditioning.
    Load { path: PathBuf, key: CondKey },
    /// Compute from the wav, then write to the ticket's temp path.
    Save(SaveTicket),
    /// Compute from the wav as before; nothing is written. Why, for the log.
    Skip(String),
}

/// `GALION_CLONE_COND_CACHE=0|off|false` turns the cache off.
pub fn cache_enabled(raw: Option<&str>) -> bool {
    !matches!(raw.map(|s| s.trim().to_ascii_lowercase()).as_deref(), Some("0") | Some("off") | Some("false"))
}

impl CondCtx {
    /// `None` when the cache is off, the clip store is unset, or no voice
    /// passed the gate.
    pub fn new(store: Option<&Path>, enabled: bool, repo: &Path, approvals: Vec<Approval>) -> Option<Self> {
        let store = store?;
        if !enabled || approvals.is_empty() {
            return None;
        }
        Some(Self { store: store.to_path_buf(), pins_sha256: pins_digest(repo), approvals })
    }

    pub fn plan(&self, model: &'static str, persona: &'static str, ref_sha256: &str, reg: &Registry) -> CondPlan {
        if model != "nano" {
            return CondPlan::Skip(format!("{model} conditioning is not cached"));
        }
        let key = CondKey::nano(persona, ref_sha256, &self.pins_sha256);
        let Some(approval) = self.approvals.iter().find(|a| a.persona == persona) else {
            return CondPlan::Skip(format!("{persona} did not pass the consent gate"));
        };
        match lookup(&self.store, &key) {
            Lookup::Hit(path) => return CondPlan::Load { path, key },
            Lookup::Miss => {}
            Lookup::Invalid(why) => eprintln!("galion-clone-cond: invalid {persona} ({why}); renamed aside, recomputing"),
        }
        match plan_save(&self.store, &key, approval, reg) {
            Ok(t) => CondPlan::Save(t),
            Err(why) => CondPlan::Skip(why),
        }
    }
}

/// Extra JSON fields for the worker request line (leading comma).
pub fn request_fields(plan: &CondPlan) -> String {
    let (field, path, key) = match plan {
        CondPlan::Load { path, key } => ("cond_load", path, key),
        CondPlan::Save(t) => ("cond_save", &t.tmp, &t.key),
        CondPlan::Skip(_) => return String::new(),
    };
    format!(
        ",\"{field}\":\"{}\",\"cond_meta\":{}",
        json_escape(&path.to_string_lossy()),
        key.metadata_json()
    )
}

/// Did the worker report writing this ticket's temp file?
pub fn saved_in(log: &str, ticket: &SaveTicket) -> bool {
    let name = ticket.tmp.file_name().and_then(|n| n.to_str()).unwrap_or("\u{0}");
    log.lines().any(|l| l.trim().strip_prefix("galion-clone-cond: saved ").is_some_and(|rest| rest.trim() == name))
}

fn verify_file(path: &Path, key: &CondKey, need_sidecar: bool) -> Result<(), String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("stat: {e}"))?;
    if meta.len() > MAX_FILE {
        return Err("cache file too large".into());
    }
    let bytes = std::fs::read(path).map_err(|e| format!("read: {e}"))?;
    if need_sidecar {
        let want = std::fs::read_to_string(sidecar(path)).unwrap_or_default();
        let want = want.split_whitespace().next().unwrap_or("").to_ascii_lowercase();
        let mut h = Sha256::new();
        h.update(&bytes);
        if !is_hex64(&want) || h.hex() != want {
            return Err("sidecar sha256 does not match the cache file".into());
        }
    }
    let header = parse_header(&bytes)?;
    for (k, v) in key.metadata() {
        match header.metadata.iter().find(|(mk, _)| mk == k) {
            Some((_, mv)) if *mv == v => {}
            Some((_, mv)) => return Err(format!("metadata {k} is {mv:?}, want {v:?}")),
            None => return Err(format!("metadata {k} missing")),
        }
    }
    for req in REQUIRED_TENSORS {
        if !header.tensors.iter().any(|t| t.name == req) {
            return Err(format!("tensor {req} missing"));
        }
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq)]
pub struct TensorInfo {
    pub name: String,
    pub dtype: String,
    pub shape: Vec<u64>,
    pub start: u64,
    pub end: u64,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Header {
    pub tensors: Vec<TensorInfo>,
    pub metadata: Vec<(String, String)>,
}

fn dtype_size(d: &str) -> Option<u64> {
    match d {
        "F32" | "I32" => Some(4),
        "I64" => Some(8),
        _ => None,
    }
}

/// Strict safetensors reader for the cache subset.
pub fn parse_header(bytes: &[u8]) -> Result<Header, String> {
    if bytes.starts_with(b"PK\x03\x04") || bytes.first() == Some(&0x80) {
        return Err("pickle / torch.save file, not safetensors".into());
    }
    if bytes.len() < 8 {
        return Err("short file".into());
    }
    let n = u64::from_le_bytes(bytes[..8].try_into().unwrap());
    if n == 0 || n > MAX_HEADER || 8 + n > bytes.len() as u64 {
        return Err("bad header length".into());
    }
    let text = std::str::from_utf8(&bytes[8..8 + n as usize]).map_err(|_| "header is not utf-8".to_string())?;
    let data_len = bytes.len() as u64 - 8 - n;
    let root = json::parse(text)?;
    let json::Value::Obj(entries) = root else { return Err("header is not an object".into()) };
    let mut h = Header::default();
    for (name, v) in entries {
        if name == "__metadata__" {
            let json::Value::Obj(m) = v else { return Err("__metadata__ is not an object".into()) };
            for (k, mv) in m {
                let json::Value::Str(s) = mv else { return Err("metadata values must be strings".into()) };
                h.metadata.push((k, s));
            }
            continue;
        }
        if !TENSOR_NAMES.contains(&name.as_str()) {
            return Err(format!("unexpected tensor {name}"));
        }
        if h.tensors.iter().any(|t| t.name == name) {
            return Err(format!("duplicate tensor {name}"));
        }
        let json::Value::Obj(f) = v else { return Err(format!("{name}: not an object")) };
        let get = |k: &str| f.iter().find(|(fk, _)| fk == k).map(|(_, v)| v);
        let Some(json::Value::Str(dtype)) = get("dtype") else { return Err(format!("{name}: dtype")) };
        let size = dtype_size(dtype).ok_or_else(|| format!("{name}: dtype {dtype} refused (F32, I64, I32 only)"))?;
        let Some(json::Value::Arr(shape)) = get("shape") else { return Err(format!("{name}: shape")) };
        let shape: Vec<u64> = shape.iter().map(|d| d.as_u64()).collect::<Option<_>>().ok_or(format!("{name}: shape"))?;
        let Some(json::Value::Arr(off)) = get("data_offsets") else { return Err(format!("{name}: data_offsets")) };
        let (Some(start), Some(end)) = (off.first().and_then(json::Value::as_u64), off.get(1).and_then(json::Value::as_u64)) else {
            return Err(format!("{name}: data_offsets"));
        };
        if off.len() != 2 || end < start || end > data_len {
            return Err(format!("{name}: offsets out of bounds"));
        }
        let count = shape.iter().try_fold(1u64, |a, &d| a.checked_mul(d)).ok_or(format!("{name}: shape overflow"))?;
        if count.checked_mul(size) != Some(end - start) {
            return Err(format!("{name}: byte size does not match shape"));
        }
        h.tensors.push(TensorInfo { name, dtype: dtype.clone(), shape, start, end });
    }
    // Offsets must tile the data area exactly: no gaps, no overlaps.
    let mut spans: Vec<(u64, u64)> = h.tensors.iter().map(|t| (t.start, t.end)).collect();
    spans.sort();
    let mut at = 0u64;
    for (s, e) in spans {
        if s != at {
            return Err("tensor data has gaps or overlaps".into());
        }
        at = e;
    }
    if at != data_len {
        return Err("trailing bytes after tensor data".into());
    }
    Ok(h)
}

/// Writer for the same subset (tests, and a Rust-side producer later).
/// `tensors`: `(name, dtype, shape, little-endian bytes)`.
pub fn write_safetensors(tensors: &[(&str, &str, Vec<u64>, Vec<u8>)], metadata: &[(&str, String)]) -> Vec<u8> {
    let mut header = String::from("{\"__metadata__\":{");
    for (i, (k, v)) in metadata.iter().enumerate() {
        if i > 0 {
            header.push(',');
        }
        header.push_str(&format!("\"{}\":\"{}\"", json_escape(k), json_escape(v)));
    }
    header.push('}');
    let mut at = 0u64;
    let mut data = Vec::new();
    for (name, dtype, shape, bytes) in tensors {
        let dims: Vec<String> = shape.iter().map(u64::to_string).collect();
        header.push_str(&format!(
            ",\"{name}\":{{\"dtype\":\"{dtype}\",\"shape\":[{}],\"data_offsets\":[{at},{}]}}",
            dims.join(","),
            at + bytes.len() as u64
        ));
        at += bytes.len() as u64;
        data.extend_from_slice(bytes);
    }
    header.push('}');
    while header.len() % 8 != 0 {
        header.push(' ');
    }
    let mut out = (header.len() as u64).to_le_bytes().to_vec();
    out.extend_from_slice(header.as_bytes());
    out.extend_from_slice(&data);
    out
}

fn json_escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o
}

/// Minimal JSON reader (objects, arrays, strings, non-negative integers,
/// true/false/null). Enough for a safetensors header; nothing is evaluated.
mod json {
    #[derive(Clone, Debug, PartialEq)]
    pub enum Value {
        Obj(Vec<(String, Value)>),
        Arr(Vec<Value>),
        Str(String),
        Num(u64),
        Bool(bool),
        Null,
    }

    impl Value {
        pub fn as_u64(&self) -> Option<u64> {
            match self {
                Value::Num(n) => Some(*n),
                _ => None,
            }
        }
    }

    pub fn parse(s: &str) -> Result<Value, String> {
        let b = s.as_bytes();
        let mut i = 0;
        let v = value(b, &mut i, 0)?;
        ws(b, &mut i);
        if i != b.len() {
            return Err("trailing data in header".into());
        }
        Ok(v)
    }

    fn ws(b: &[u8], i: &mut usize) {
        while *i < b.len() && matches!(b[*i], b' ' | b'\n' | b'\r' | b'\t') {
            *i += 1;
        }
    }

    fn value(b: &[u8], i: &mut usize, depth: u32) -> Result<Value, String> {
        if depth > 8 {
            return Err("header nests too deep".into());
        }
        ws(b, i);
        match b.get(*i) {
            Some(b'{') => {
                *i += 1;
                let mut out = Vec::new();
                ws(b, i);
                if b.get(*i) == Some(&b'}') {
                    *i += 1;
                    return Ok(Value::Obj(out));
                }
                loop {
                    ws(b, i);
                    let Value::Str(k) = string(b, i)? else { unreachable!() };
                    ws(b, i);
                    if b.get(*i) != Some(&b':') {
                        return Err("expected ':'".into());
                    }
                    *i += 1;
                    let v = value(b, i, depth + 1)?;
                    if out.iter().any(|(ok, _): &(String, Value)| *ok == k) {
                        return Err(format!("duplicate key {k}"));
                    }
                    out.push((k, v));
                    ws(b, i);
                    match b.get(*i) {
                        Some(b',') => *i += 1,
                        Some(b'}') => {
                            *i += 1;
                            return Ok(Value::Obj(out));
                        }
                        _ => return Err("expected ',' or '}'".into()),
                    }
                }
            }
            Some(b'[') => {
                *i += 1;
                let mut out = Vec::new();
                ws(b, i);
                if b.get(*i) == Some(&b']') {
                    *i += 1;
                    return Ok(Value::Arr(out));
                }
                loop {
                    out.push(value(b, i, depth + 1)?);
                    ws(b, i);
                    match b.get(*i) {
                        Some(b',') => *i += 1,
                        Some(b']') => {
                            *i += 1;
                            return Ok(Value::Arr(out));
                        }
                        _ => return Err("expected ',' or ']'".into()),
                    }
                }
            }
            Some(b'"') => string(b, i),
            Some(c) if c.is_ascii_digit() => {
                let s = *i;
                while *i < b.len() && b[*i].is_ascii_digit() {
                    *i += 1;
                }
                std::str::from_utf8(&b[s..*i]).unwrap().parse().map(Value::Num).map_err(|_| "number".into())
            }
            _ if b[*i..].starts_with(b"true") => {
                *i += 4;
                Ok(Value::Bool(true))
            }
            _ if b[*i..].starts_with(b"false") => {
                *i += 5;
                Ok(Value::Bool(false))
            }
            _ if b[*i..].starts_with(b"null") => {
                *i += 4;
                Ok(Value::Null)
            }
            _ => Err("unexpected byte in header".into()),
        }
    }

    fn string(b: &[u8], i: &mut usize) -> Result<Value, String> {
        if b.get(*i) != Some(&b'"') {
            return Err("expected string".into());
        }
        *i += 1;
        let mut out = String::new();
        loop {
            match b.get(*i) {
                None => return Err("unterminated string".into()),
                Some(b'"') => {
                    *i += 1;
                    return Ok(Value::Str(out));
                }
                Some(b'\\') => {
                    let c = *b.get(*i + 1).ok_or("bad escape")?;
                    *i += 2;
                    match c {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'n' => out.push('\n'),
                        b't' => out.push('\t'),
                        b'r' => out.push('\r'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'u' => {
                            let h = b.get(*i..*i + 4).ok_or("bad \\u")?;
                            let n = u32::from_str_radix(std::str::from_utf8(h).map_err(|_| "bad \\u")?, 16).map_err(|_| "bad \\u")?;
                            out.push(char::from_u32(n).unwrap_or('\u{fffd}'));
                            *i += 4;
                        }
                        _ => return Err("bad escape".into()),
                    }
                }
                Some(_) => {
                    let rest = std::str::from_utf8(&b[*i..]).map_err(|_| "utf-8")?;
                    let ch = rest.chars().next().unwrap();
                    out.push(ch);
                    *i += ch.len_utf8();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REF: &str = "1779e9f3c5c470b236f26608f862b75958c8b8676e3faf8e47be6de3b79ab8ac";
    const PINS: &str = "aa00000000000000000000000000000000000000000000000000000000000000";

    struct Store(PathBuf);
    impl Drop for Store {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn store(tag: &str) -> Store {
        let p = std::env::temp_dir().join(format!("galion-cond-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        Store(p)
    }

    fn f32s(v: &[f32]) -> Vec<u8> {
        v.iter().flat_map(|x| x.to_le_bytes()).collect()
    }
    fn i64s(v: &[i64]) -> Vec<u8> {
        v.iter().flat_map(|x| x.to_le_bytes()).collect()
    }

    fn fixture(key: &CondKey) -> Vec<u8> {
        let meta: Vec<(&str, String)> = key.metadata();
        write_safetensors(
            &[
                ("t3.speaker_emb", "F32", vec![1, 4], f32s(&[0.1, 0.2, 0.3, 0.4])),
                ("t3.cond_prompt_speech_tokens", "I64", vec![1, 3], i64s(&[5, 6, 7])),
                ("t3.emotion_adv", "F32", vec![1, 1, 1], f32s(&[0.0])),
                ("gen.prompt_token", "I64", vec![1, 2], i64s(&[1, 2])),
                ("gen.prompt_token_len", "I64", vec![1], i64s(&[2])),
                ("gen.prompt_feat", "F32", vec![1, 2, 2], f32s(&[1.0, 2.0, 3.0, 4.0])),
                ("gen.embedding", "F32", vec![1, 2], f32s(&[0.5, 0.6])),
            ],
            &meta,
        )
    }

    fn approve(reg: &Registry, persona: &'static str, sha: &str) -> Approval {
        let a = Approval { persona, seconds: 9.6, public: false, wav: PathBuf::from("/x/reference.wav"), sha256: sha.into() };
        reg.record(persona, Verdict::Approved(a.clone()));
        a
    }

    #[test]
    fn key_changes_with_ref_pins_and_options() {
        let k = CondKey::nano("samantha", REF, PINS);
        assert_eq!(k.id(), CondKey::nano("samantha", &REF.to_uppercase(), PINS).id());
        assert_eq!(k.id().len(), 32);
        assert_ne!(k.id(), CondKey::nano("jarvis", REF, PINS).id());
        assert_ne!(k.id(), CondKey::nano("samantha", &REF.replace('1', "2"), PINS).id());
        assert_ne!(k.id(), CondKey::nano("samantha", REF, &PINS.replace('a', "b")).id());
        let mut k2 = k.clone();
        k2.exaggeration = "0.5";
        assert_ne!(k.id(), k2.id());
        assert!(k.file_name().starts_with("nano-") && k.file_name().ends_with(".safetensors"));
        assert!(k.metadata_json().contains("\"format\":\"galion-cond-v1\""));
    }

    #[test]
    fn pins_digest_moves_when_a_pin_table_changes() {
        let s = store("pins");
        let d = s.0.join("scripts/clone");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("PINS.sha256"), "a 1 2\n").unwrap();
        let a = pins_digest(&s.0);
        std::fs::write(d.join("TOKENIZER.sha256"), "tokenizer/vocab.json 1 2\n").unwrap();
        let b = pins_digest(&s.0);
        std::fs::write(d.join("PINS.sha256"), "a 1 3\n").unwrap();
        let c = pins_digest(&s.0);
        assert!(is_hex64(&a) && a != b && b != c);
        assert_eq!(c, pins_digest(&s.0));
    }

    #[test]
    fn no_write_without_the_consent_gate() {
        let s = store("gate");
        let key = CondKey::nano("samantha", REF, PINS);
        let reg = Registry::new();
        let forged = Approval { persona: "samantha", seconds: 9.6, public: false, wav: PathBuf::new(), sha256: REF.into() };
        // Gate never ran: refused, and nothing is created.
        assert!(plan_save(&s.0, &key, &forged, &reg).unwrap_err().contains("has not run"));
        assert!(!cond_dir(&s.0, "samantha").exists());
        // Gate refused the voice.
        reg.record("samantha", Verdict::Refused("consent"));
        assert!(plan_save(&s.0, &key, &forged, &reg).unwrap_err().contains("refused"));
        // Approved, but for another wav.
        let other = approve(&reg, "samantha", &REF.replace('1', "3"));
        assert!(plan_save(&s.0, &key, &other, &reg).is_err());
        // Approval presented differs from the registry's.
        let _ = approve(&reg, "samantha", REF);
        let mut wrong = forged.clone();
        wrong.seconds = 11.0;
        assert!(plan_save(&s.0, &key, &wrong, &reg).unwrap_err().contains("differs"));
        // Persona mismatch.
        let jar = CondKey::nano("jarvis", REF, PINS);
        let sam = approve(&reg, "samantha", REF);
        assert!(plan_save(&s.0, &jar, &sam, &reg).is_err());
        // The real thing.
        let t = plan_save(&s.0, &key, &sam, &reg).unwrap();
        assert!(t.tmp.starts_with(cond_dir(&s.0, "samantha")));
        assert_eq!(t.dest, entry_path(&s.0, &key));
    }

    #[test]
    fn save_commit_lookup_round_trip() {
        let s = store("rt");
        let key = CondKey::nano("samantha", REF, PINS);
        let reg = Registry::new();
        let a = approve(&reg, "samantha", REF);
        assert_eq!(lookup(&s.0, &key), Lookup::Miss);
        let t = plan_save(&s.0, &key, &a, &reg).unwrap();
        std::fs::write(&t.tmp, fixture(&key)).unwrap();
        let dest = commit(&t).unwrap();
        assert!(!t.tmp.exists());
        assert_eq!(lookup(&s.0, &key), Lookup::Hit(dest.clone()));
        let side = std::fs::read_to_string(sidecar(&dest)).unwrap();
        assert!(side.starts_with(&sha256::file_hex(&dest).unwrap()));
        let h = parse_header(&std::fs::read(&dest).unwrap()).unwrap();
        assert_eq!(h.tensors.len(), 7);
    }

    #[test]
    fn new_reference_or_pins_never_read_the_old_file_and_retire_it() {
        let s = store("stale");
        let reg = Registry::new();
        let old = CondKey::nano("jarvis", REF, PINS);
        let a = approve(&reg, "jarvis", REF);
        let t = plan_save(&s.0, &old, &a, &reg).unwrap();
        std::fs::write(&t.tmp, fixture(&old)).unwrap();
        let old_path = commit(&t).unwrap();
        // Re-cut reference: new sha256, new key.
        let new_ref = REF.replace('1', "4");
        let new = CondKey::nano("jarvis", &new_ref, PINS);
        assert_eq!(lookup(&s.0, &new), Lookup::Miss);
        // Even a file copied under the new name fails on metadata.
        std::fs::copy(&old_path, entry_path(&s.0, &new)).unwrap();
        std::fs::copy(sidecar(&old_path), sidecar(&entry_path(&s.0, &new))).unwrap();
        match lookup(&s.0, &new) {
            Lookup::Invalid(why) => assert!(why.contains("sidecar") || why.contains("ref_sha256"), "{why}"),
            other => panic!("{other:?}"),
        }
        assert!(!entry_path(&s.0, &new).exists(), "renamed aside");
        // Saving the new key retires the old entry (renamed, not deleted).
        let a2 = approve(&reg, "jarvis", &new_ref);
        let t2 = plan_save(&s.0, &new, &a2, &reg).unwrap();
        std::fs::write(&t2.tmp, fixture(&new)).unwrap();
        commit(&t2).unwrap();
        assert!(!old_path.exists());
        let offs: Vec<_> = std::fs::read_dir(cond_dir(&s.0, "jarvis"))
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().contains(".off-"))
            .collect();
        assert!(offs.len() >= 2, "old + copied entries kept as .off");
        // Pins change: new key again.
        let p2 = CondKey::nano("jarvis", &new_ref, &PINS.replace('a', "c"));
        assert_eq!(lookup(&s.0, &p2), Lookup::Miss);
    }

    #[test]
    fn tampered_or_foreign_files_are_refused() {
        let s = store("tamper");
        let key = CondKey::nano("samantha", REF, PINS);
        let reg = Registry::new();
        let a = approve(&reg, "samantha", REF);
        let t = plan_save(&s.0, &key, &a, &reg).unwrap();
        std::fs::write(&t.tmp, fixture(&key)).unwrap();
        let dest = commit(&t).unwrap();
        // Flip one data byte: sidecar check fails, file is renamed aside.
        let mut b = std::fs::read(&dest).unwrap();
        let last = b.len() - 1;
        b[last] ^= 1;
        std::fs::write(&dest, &b).unwrap();
        assert!(matches!(lookup(&s.0, &key), Lookup::Invalid(_)));
        assert!(!dest.exists());
        // Worker output that is a torch.save zip or pickle never commits.
        for bad in [b"PK\x03\x04rest".to_vec(), vec![0x80, 0x04, 0x95]] {
            let t = plan_save(&s.0, &key, &a, &reg).unwrap();
            std::fs::write(&t.tmp, bad).unwrap();
            assert!(commit(&t).unwrap_err().contains("pickle"));
            assert!(!t.dest.exists());
        }
        // Wrong metadata (another persona's conditioning).
        let t = plan_save(&s.0, &key, &a, &reg).unwrap();
        std::fs::write(&t.tmp, fixture(&CondKey::nano("jarvis", REF, PINS))).unwrap();
        assert!(commit(&t).unwrap_err().contains("persona"));
    }

    #[test]
    fn strict_header_reader() {
        let key = CondKey::nano("samantha", REF, PINS);
        let meta = key.metadata();
        let ok = parse_header(&fixture(&key)).unwrap();
        assert_eq!(ok.metadata.len(), 7);
        let bad_dtype = write_safetensors(&[("t3.speaker_emb", "F16", vec![2], vec![0; 4])], &meta);
        assert!(parse_header(&bad_dtype).unwrap_err().contains("F16"));
        let bad_name = write_safetensors(&[("evil", "F32", vec![1], vec![0; 4])], &meta);
        assert!(parse_header(&bad_name).unwrap_err().contains("unexpected"));
        let bad_size = write_safetensors(&[("t3.speaker_emb", "F32", vec![3], vec![0; 8])], &meta);
        assert!(parse_header(&bad_size).unwrap_err().contains("shape"));
        let mut trailing = fixture(&key);
        trailing.extend_from_slice(&[0; 4]);
        assert!(parse_header(&trailing).is_err());
        // Overlapping offsets, hand-written.
        let hdr = br#"{"t3.speaker_emb":{"dtype":"F32","shape":[1],"data_offsets":[0,4]},"gen.embedding":{"dtype":"F32","shape":[1],"data_offsets":[0,4]}}"#;
        let mut f = (hdr.len() as u64).to_le_bytes().to_vec();
        f.extend_from_slice(hdr);
        f.extend_from_slice(&[0; 4]);
        assert!(parse_header(&f).unwrap_err().contains("overlap"));
        // Header length beyond the file.
        let mut huge = u64::MAX.to_le_bytes().to_vec();
        huge.extend_from_slice(b"{}");
        assert!(parse_header(&huge).is_err());
    }

    #[test]
    fn ctx_plans_save_then_load_and_request_fields() {
        let s = store("ctx");
        let reg = Registry::new();
        let a = approve(&reg, "samantha", REF);
        assert!(CondCtx::new(None, true, &s.0, vec![a.clone()]).is_none());
        assert!(CondCtx::new(Some(&s.0), false, &s.0, vec![a.clone()]).is_none());
        assert!(CondCtx::new(Some(&s.0), true, &s.0, vec![]).is_none());
        assert!(!cache_enabled(Some("0")) && !cache_enabled(Some("OFF")) && cache_enabled(None) && cache_enabled(Some("1")));
        let ctx = CondCtx::new(Some(&s.0), true, &s.0, vec![a]).unwrap();
        assert!(matches!(ctx.plan("turbo", "samantha", REF, &reg), CondPlan::Skip(_)));
        assert!(matches!(ctx.plan("nano", "jarvis", REF, &reg), CondPlan::Skip(_)));
        let CondPlan::Save(t) = ctx.plan("nano", "samantha", REF, &reg) else { panic!() };
        let f = request_fields(&CondPlan::Save(t.clone()));
        assert!(f.starts_with(",\"cond_save\":\"") && f.contains("\"ref_sha256\":\"1779e9f3"));
        let name = t.tmp.file_name().unwrap().to_str().unwrap().to_string();
        assert!(!saved_in("galion-clone-cond: saved other.tmp-1\n", &t));
        assert!(saved_in(&format!("x\ngalion-clone-cond: saved {name}\n"), &t));
        std::fs::write(&t.tmp, fixture(&t.key)).unwrap();
        commit(&t).unwrap();
        let CondPlan::Load { path, .. } = ctx.plan("nano", "samantha", REF, &reg) else { panic!() };
        assert_eq!(path, t.dest);
        // Registry cleared (gate not run in this process): no save planned.
        let reg2 = Registry::new();
        assert!(matches!(ctx.plan("nano", "samantha", &REF.replace('1', "5"), &reg2), CondPlan::Skip(_)));
    }

    /// Cross-check with the Python writer (`chatterbox_worker.save_conds`,
    /// real `safetensors.torch.save_file`). Run by hand:
    /// `GALION_COND_XCHECK=<file>:<ref_sha256>:<pins_sha256> cargo test … -- --ignored python_written`.
    #[test]
    #[ignore]
    fn python_written_file_verifies() {
        let raw = std::env::var("GALION_COND_XCHECK").expect("GALION_COND_XCHECK");
        let mut it = raw.rsplitn(3, ':');
        let (pins, r, file) = (it.next().unwrap(), it.next().unwrap(), it.next().unwrap());
        let key = CondKey::nano("samantha", r, pins);
        let h = parse_header(&std::fs::read(file).unwrap()).unwrap();
        eprintln!("tensors: {:?}", h.tensors.iter().map(|t| (&t.name, &t.dtype, &t.shape)).collect::<Vec<_>>());
        verify_file(Path::new(file), &key, false).unwrap();
    }

    #[test]
    fn json_reader_basics() {
        let v = json::parse(r#" {"a":[1,2,{"b":"x\"y\u0041"}],"t":true,"n":null} "#).unwrap();
        let json::Value::Obj(o) = v else { panic!() };
        assert_eq!(o.len(), 3);
        assert!(json::parse(r#"{"a":1,"a":2}"#).is_err());
        assert!(json::parse(r#"{"a":1} x"#).is_err());
        assert!(json::parse(&"[".repeat(20)).is_err());
    }
}
