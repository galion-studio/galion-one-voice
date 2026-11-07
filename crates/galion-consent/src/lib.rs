// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! Clone-time consent gate for one Samantha or Jarvis reference clip.
//!
//! Owner decision (2026-11-04 02:39 CET): the consent check runs only when a
//! voice is cloned, not on every turn. This crate is that check, in Rust, in
//! process. It replaces `scripts/clone/consent.py` (kept as
//! `tools/legacy/consent.py.bak`). Nothing here spawns a process.
//!
//! [`evaluate`] has the same order, the same reasons and the same record
//! formats as `consent.py evaluate()`:
//!
//! 1. `persona` — the voice id is `samantha` or `jarvis`.
//! 2. `store` — `GALION_CLIP_STORE` is set (no default path).
//! 3. `inside_repo` — the store is outside the git tree.
//! 4. `wav` — `<store>/<persona>/reference.wav` is a file outside the tree
//!    and parses as a RIFF/WAVE PCM file (Python `wave` rules).
//! 5. `mono` — one channel, 16-bit samples.
//! 6. `duration` — 8–12 s.
//! 7. `consent` — `consent.txt` beside the wav has every [`REQUIRED`] field
//!    filled (no placeholder, no `[`/`]`, at most 400 chars), its `persona`
//!    names this voice, `scope` is [`SCOPE`], `public_demo` is yes/no, `date`
//!    carries `CET` or `Zagreb`, and `duration_s` is a number.
//! 8. `duration` — the declared `duration_s` is within 0.25 s of the wav.
//! 9. `sha` — the declared `sha256` is 64 hex chars and equals both the
//!    `reference.wav.sha256` sidecar and the wav bytes (tamper check).
//! 10. `lock` — `reference_clips = true` inside
//!     `[formats.exceptions.v2s14_chatterbox]` of `LOCKS.toml`
//!     (`GALION_LOCKS_FILE` overrides the path). Missing means false.
//! 11. `worker` — `scripts/clone/chatterbox_worker.py` is in the tree.
//! 12. `weights` — every pinned clone weight is on disk at its pinned size
//!     (`scripts/clone/PINS.sha256`, root `GALION_CLONE_WEIGHTS`, model
//!     `GALION_CLONE_MODEL`).
//!
//! Two refusals are stricter than the Python file, at the owner's request:
//!
//! - `celebrity` — LOCKS: "No celebrity clone". A voice id, a consent record
//!   (`persona`, `performer`, `work`, `grantor`) or a reference path naming a
//!   blocked voice ([`BLOCKED_VOICES`]: Armstrong) is refused. Python refused
//!   the Armstrong id too, as `persona`.
//! - `expired` — an `expiry` that starts with an ISO date (`YYYY-MM-DD`)
//!   earlier than today (UTC) is refused. Python only required the field to be
//!   filled. Non-date expiry text (for example "on withdrawal") is unchanged.
//!
//! A pass is recorded in the in-memory [`Registry`]. The per-turn path only
//! looks there ([`Registry::lookup`]); it never evaluates and never spawns.

#![forbid(unsafe_code)]

pub mod recut;
pub mod sha256;

use std::fmt;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

/// Consent fields, in the order `consent.py fields` printed them.
pub const REQUIRED: [&str; 15] = [
    "date",
    "persona",
    "work",
    "performer",
    "grantor",
    "instrument",
    "grantee",
    "scope",
    "public_demo",
    "expiry",
    "operator",
    "withdrawal",
    "counsel",
    "sha256",
    "duration_s",
];

pub const SCOPE: &str = "local-zero-shot-conditioning";
/// Owner-widened scope (t662s3211, 2026-11-05 10:32 CET): fine-tune a Piper
/// voice for the Samantha / Jarvis CC0 donors only. Recorded in a separate
/// addendum next to `consent.txt`; the original consent record is never edited.
pub const SCOPE_PIPER_TRAINING: &str = "piper-voice-training";
/// Filename of the addendum beside `consent.txt`.
pub const PIPER_TRAINING_ADDENDUM: &str = "consent-addendum-piper-training.txt";
pub const DURATION_MIN: f64 = 8.0;
pub const DURATION_MAX: f64 = 12.0;
pub const DURATION_TOLERANCE: f64 = 0.25;
pub const PLACEHOLDERS: [&str; 10] = [
    "todo",
    "field",
    "placeholder",
    "unknown",
    "tbd",
    "n/a",
    "na",
    "none",
    "null",
    "fixme",
];
pub const TOKENIZER: [&str; 5] = [
    "tokenizer/added_tokens.json",
    "tokenizer/merges.txt",
    "tokenizer/special_tokens_map.json",
    "tokenizer/tokenizer_config.json",
    "tokenizer/vocab.json",
];
pub const PERSONAS: [&str; 2] = ["samantha", "jarvis"];
/// LOCKS `[formats.exceptions.v2s14_chatterbox]` notes: "No celebrity clone."
/// Lowercase substrings. Armstrong stays blocked (ASR/listening sample only).
pub const BLOCKED_VOICES: [&str; 1] = ["armstrong"];

static EVALUATIONS: AtomicUsize = AtomicUsize::new(0);

/// How many times [`evaluate`] has run in this process. Tests use it to show
/// a live turn makes no consent call.
pub fn evaluations() -> usize {
    EVALUATIONS.load(Ordering::SeqCst)
}

/// Where the gate looks. [`Env::from_process`] reads the same variables as
/// `consent.py`; tests build one by hand.
#[derive(Clone, Debug)]
pub struct Env {
    /// The git tree (`REPO` in consent.py).
    pub repo: PathBuf,
    /// `GALION_CLIP_STORE`, trimmed. `None` when unset or empty.
    pub store: Option<PathBuf>,
    /// `GALION_LOCKS_FILE` or `<repo>/LOCKS.toml`.
    pub locks: PathBuf,
    /// `GALION_CLONE_WEIGHTS` or `<repo>/weights/clone`.
    pub weights: PathBuf,
    /// `GALION_CLONE_MODEL` (`nano` unless `turbo`).
    pub model: &'static str,
    /// `<repo>/scripts/clone/PINS.sha256`.
    pub pins: PathBuf,
    /// `<repo>/scripts/clone/chatterbox_worker.py`.
    pub worker: PathBuf,
    /// Civil date for the expiry check. `None` is today (UTC).
    pub today: Option<(i32, u32, u32)>,
}

fn env_trim(key: &str) -> String {
    std::env::var(key).map(|v| v.trim().to_string()).unwrap_or_default()
}

/// The repository this crate was built from.
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

impl Env {
    /// Same variables as consent.py, rooted at `repo`.
    pub fn from_process(repo: &Path) -> Self {
        let store = env_trim("GALION_CLIP_STORE");
        let locks = env_trim("GALION_LOCKS_FILE");
        let weights = env_trim("GALION_CLONE_WEIGHTS");
        let model = env_trim("GALION_CLONE_MODEL").to_lowercase();
        let mut env = Self::at(repo);
        env.store = (!store.is_empty()).then(|| PathBuf::from(store));
        if !locks.is_empty() {
            env.locks = PathBuf::from(locks);
        }
        if !weights.is_empty() {
            env.weights = PathBuf::from(weights);
        }
        env.model = if model == "turbo" { "turbo" } else { "nano" };
        env
    }

    /// Defaults only, no environment read. No store.
    pub fn at(repo: &Path) -> Self {
        Self {
            repo: repo.to_path_buf(),
            store: None,
            locks: repo.join("LOCKS.toml"),
            weights: repo.join("weights").join("clone"),
            model: "nano",
            pins: repo.join("scripts").join("clone").join("PINS.sha256"),
            worker: repo.join("scripts").join("clone").join("chatterbox_worker.py"),
            today: None,
        }
    }

    /// [`Env::from_process`] at [`repo_root`].
    pub fn process() -> Self {
        Self::from_process(&repo_root())
    }
}

/// One gate result. `reason` is `ready` on a pass.
#[derive(Clone, Debug, PartialEq)]
pub struct Gate {
    pub ok: bool,
    pub reason: &'static str,
    pub seconds: Option<f64>,
    pub public: bool,
    /// The consent record's instrument / grantor is CC0 or a public-domain
    /// dedication (no individually signed release). Using such a voice needs
    /// the caller's CC0 licence acceptance ([`cc0_use_gate`]).
    pub cc0: bool,
    pub wav: Option<PathBuf>,
    pub sha256: Option<String>,
}

impl Gate {
    fn absent(reason: &'static str) -> Self {
        Self { ok: false, reason, seconds: None, public: false, cc0: false, wav: None, sha256: None }
    }
}

/// Human text for a refusal reason.
pub fn reason_message(reason: &str) -> &'static str {
    match reason {
        "persona" => "the voice id is not samantha or jarvis",
        "celebrity" => "celebrity voices are blocked by LOCKS.toml (no celebrity clone)",
        "store" => "GALION_CLIP_STORE is unset; there is no default clip store",
        "inside_repo" => "the clip store is inside the git tree",
        "wav" => "reference.wav is missing, inside the tree, or not a PCM wav",
        "mono" => "the reference is not mono 16-bit PCM",
        "duration" => "the reference is not 8-12 s or does not match the declared duration_s",
        "consent" => "consent.txt is missing, incomplete, or does not match this voice",
        "expired" => "the consent record has expired",
        "sha" => "the reference sha256 does not match the consent record and sidecar (tampered or re-cut)",
        "lock" => "LOCKS.toml does not allow reference clips (reference_clips is not true)",
        "worker" => "the clone worker is not in the tree",
        "weights" => "the pinned clone weights are not on disk at their pinned sizes",
        "cc0_licence" => "this is a CC0 voice and the caller has not accepted the CC0 1.0 licence for this session",
        "ready" => "consent is valid",
        _ => "refused",
    }
}

/// A clone refused at the gate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CloneRefused {
    pub persona: String,
    pub reason: &'static str,
}

impl fmt::Display for CloneRefused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "clone refused for {}: {} (consent gate: {})",
            if self.persona.is_empty() { "persona" } else { &self.persona },
            reason_message(self.reason),
            self.reason
        )
    }
}

impl std::error::Error for CloneRefused {}

/// A voice that passed the gate when it was cloned.
#[derive(Clone, Debug, PartialEq)]
pub struct Approval {
    pub persona: &'static str,
    pub seconds: f64,
    pub public: bool,
    pub cc0: bool,
    pub wav: PathBuf,
    pub sha256: String,
}

/// What the clone gate decided for one voice.
#[derive(Clone, Debug, PartialEq)]
pub enum Verdict {
    Approved(Approval),
    Refused(&'static str),
}

/// In-memory clone decisions. Written at clone time, read per turn.
#[derive(Debug, Default)]
pub struct Registry {
    slots: Mutex<Vec<(&'static str, Verdict)>>,
}

impl Registry {
    pub const fn new() -> Self {
        Self { slots: Mutex::new(Vec::new()) }
    }

    pub fn record(&self, persona: &'static str, verdict: Verdict) {
        let mut slots = self.slots.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(slot) = slots.iter_mut().find(|(p, _)| *p == persona) {
            slot.1 = verdict;
        } else {
            slots.push((persona, verdict));
        }
    }

    /// Per-turn read. No file I/O, no evaluation, no process.
    pub fn lookup(&self, persona: &str) -> Option<Verdict> {
        let key = persona.trim().to_ascii_lowercase();
        let slots = self.slots.lock().unwrap_or_else(|e| e.into_inner());
        slots.iter().find(|(p, _)| *p == key).map(|(_, v)| v.clone())
    }

    pub fn approved(&self, persona: &str) -> bool {
        matches!(self.lookup(persona), Some(Verdict::Approved(_)))
    }

    pub fn clear(&self) {
        self.slots.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }
}

/// The process registry the clone slot writes and the persona floor reads.
pub fn global() -> &'static Registry {
    static GLOBAL: OnceLock<Registry> = OnceLock::new();
    GLOBAL.get_or_init(Registry::new)
}

/// Clone-time gate: evaluate, record the verdict in `reg`, and refuse with a
/// clear error when consent is not valid.
pub fn clone_gate_into(reg: &Registry, persona: &str, env: &Env) -> Result<Approval, CloneRefused> {
    let gate = evaluate(persona, env, true);
    let name = persona_of(persona);
    if gate.ok {
        let approval = Approval {
            persona: name.unwrap_or("samantha"),
            seconds: gate.seconds.unwrap_or(0.0),
            public: gate.public,
            cc0: gate.cc0,
            wav: gate.wav.clone().unwrap_or_default(),
            sha256: gate.sha256.clone().unwrap_or_default(),
        };
        if let Some(name) = name {
            reg.record(name, Verdict::Approved(approval.clone()));
        }
        return Ok(approval);
    }
    if let Some(name) = name {
        reg.record(name, Verdict::Refused(gate.reason));
    }
    Err(CloneRefused { persona: persona.trim().to_lowercase(), reason: gate.reason })
}

/// [`clone_gate_into`] the [`global`] registry.
pub fn clone_gate(persona: &str, env: &Env) -> Result<Approval, CloneRefused> {
    clone_gate_into(global(), persona, env)
}

/// `samantha` / `jarvis`, or `None`.
pub fn persona_of(name: &str) -> Option<&'static str> {
    let key = name.trim().to_lowercase();
    PERSONAS.iter().copied().find(|p| *p == key)
}

fn blocked(text: &str) -> bool {
    let lower = text.to_lowercase();
    BLOCKED_VOICES.iter().any(|b| lower.contains(b))
}

/// Python `str.splitlines()` boundaries.
fn splitlines(text: &str) -> impl Iterator<Item = &str> {
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut chars = text.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        let brk = matches!(
            c,
            '\n' | '\r' | '\u{0b}' | '\u{0c}' | '\u{1c}' | '\u{1d}' | '\u{1e}' | '\u{85}' | '\u{2028}' | '\u{2029}'
        );
        if !brk {
            continue;
        }
        out.push(&text[start..i]);
        let mut next = i + c.len_utf8();
        if c == '\r' {
            if let Some(&(j, '\n')) = chars.peek() {
                chars.next();
                next = j + 1;
            }
        }
        start = next;
    }
    if start < text.len() {
        out.push(&text[start..]);
    }
    out.into_iter()
}

/// True only inside the Chatterbox exception table. Missing means false.
pub fn reference_clips_allowed(text: &str) -> bool {
    let mut in_table = false;
    for raw in splitlines(text) {
        let line = raw.trim();
        if line.starts_with('[') && line.ends_with(']') {
            in_table = line == "[formats.exceptions.v2s14_chatterbox]";
            continue;
        }
        if !in_table || !line.starts_with("reference_clips") {
            continue;
        }
        let Some((_, value)) = line.split_once('=') else {
            return false;
        };
        let value = value.split('#').next().unwrap_or("").trim().to_lowercase();
        return value == "true";
    }
    false
}

/// `key: value` or `key = value` lines. Same rules as consent.py.
pub fn parse_consent(text: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for raw in splitlines(text) {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let pair = if line.contains(':') { line.split_once(':') } else { line.split_once('=') };
        let Some((key, value)) = pair else { continue };
        let key = key.trim().to_lowercase().replace(' ', "_");
        let value = value.trim().to_string();
        if let Some(slot) = out.iter_mut().find(|(k, _)| *k == key) {
            slot.1 = value;
        } else {
            out.push((key, value));
        }
    }
    out
}

fn field<'a>(fields: &'a [(String, String)], key: &str) -> &'a str {
    fields.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str()).unwrap_or("")
}

pub fn value_rejected(value: &str) -> bool {
    if value.is_empty() || value.chars().count() > 400 {
        return true;
    }
    if value.contains('[') || value.contains(']') {
        return true;
    }
    let lower = value.trim().to_lowercase();
    PLACEHOLDERS.contains(&lower.as_str())
}

/// Python `float()` for the strings a consent record carries.
fn py_float(s: &str) -> Option<f64> {
    let s = s.trim();
    let bytes: Vec<char> = s.chars().collect();
    let mut clean = String::with_capacity(s.len());
    for (i, c) in bytes.iter().enumerate() {
        if *c == '_' {
            let prev = i > 0 && bytes[i - 1].is_ascii_digit();
            let next = i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit();
            if prev && next {
                continue;
            }
            return None;
        }
        clean.push(*c);
    }
    clean.parse::<f64>().ok()
}

/// `(seconds, channels, sample_width_bytes)` under Python `wave` rules.
pub fn wav_info(path: &Path) -> Option<(f64, u16, u16)> {
    let bytes = std::fs::read(path).ok()?;
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return None;
    }
    // Reads stop at the end of the RIFF chunk, as in Python `wave`.
    let riff = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
    let limit = bytes.len().min(riff.saturating_add(8));
    let mut pos = 12usize;
    let mut fmt: Option<(u16, u32, u16)> = None;
    let mut data: Option<u32> = None;
    while pos + 8 <= limit {
        let id = &bytes[pos..pos + 4];
        let size = u32::from_le_bytes([bytes[pos + 4], bytes[pos + 5], bytes[pos + 6], bytes[pos + 7]]);
        let body = pos + 8;
        if id == b"fmt " {
            let end = body.saturating_add(size as usize).min(limit);
            let chunk = &bytes[body..end];
            fmt = Some(read_fmt(chunk)?);
        } else if id == b"data" {
            fmt?;
            data = Some(size);
            break;
        }
        let padded = size as usize + (size as usize & 1);
        pos = body.saturating_add(padded);
    }
    let (channels, rate, width) = fmt?;
    let size = data?;
    let frame = channels as u64 * width as u64;
    let frames = size as u64 / frame;
    if rate == 0 {
        return None;
    }
    Some((frames as f64 / rate as f64, channels, width))
}

fn read_fmt(chunk: &[u8]) -> Option<(u16, u32, u16)> {
    if chunk.len() < 14 {
        return None;
    }
    let u16_at = |i: usize| u16::from_le_bytes([chunk[i], chunk[i + 1]]);
    let tag = u16_at(0);
    let channels = u16_at(2);
    let rate = u32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]);
    if tag != 1 && tag != 0xFFFE {
        return None;
    }
    if chunk.len() < 16 {
        return None;
    }
    let bits = u16_at(14);
    let width = bits.div_ceil(8);
    if width == 0 {
        return None;
    }
    if tag == 0xFFFE {
        // cbSize(2) validBits(2) channelMask(4), then a 16-byte subformat GUID.
        const PCM: [u8; 16] = [
            0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71,
        ];
        if chunk.len() < 16 + 8 + 16 || chunk[24..40] != PCM {
            return None;
        }
    }
    if channels == 0 {
        return None;
    }
    Some((channels, rate, width))
}

fn read_sidecar(path: &Path) -> String {
    let Ok(text) = std::fs::read_to_string(path) else {
        return String::new();
    };
    text.split_whitespace().next().unwrap_or("").trim().to_lowercase()
}

fn pins(path: &Path) -> Option<Vec<(String, u64)>> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut out: Vec<(String, u64)> = Vec::new();
    for raw in splitlines(&text) {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 3 {
            continue;
        }
        let size = parts[2].replace('_', "").trim_start_matches('+').parse().ok()?;
        if let Some(slot) = out.iter_mut().find(|(k, _)| k == parts[0]) {
            slot.1 = size;
        } else {
            out.push((parts[0].to_string(), size));
        }
    }
    Some(out)
}

pub fn weight_files(model: &str) -> Vec<String> {
    let mut out = vec![
        "shared/s3gen.safetensors".to_string(),
        "shared/s3gen_meanflow.safetensors".to_string(),
        "shared/ve.safetensors".to_string(),
        format!("{model}/t3_{model}_v1.safetensors"),
    ];
    out.extend(TOKENIZER.iter().map(|s| s.to_string()));
    out
}

/// Size match against the pin table. The speech slot still hashes before it speaks.
/// Tokenizer rows live in `TOKENIZER.sha256` beside `PINS.sha256` (the same
/// split chatterbox_worker.py and clone_worker.rs read), so both tables are merged.
pub fn weights_sized(env: &Env) -> bool {
    let Some(mut table) = pins(&env.pins) else {
        return false;
    };
    if let Some(tok) = env.pins.parent().and_then(|d| pins(&d.join("TOKENIZER.sha256"))) {
        table.extend(tok);
    }
    weight_files(env.model).iter().all(|rel| {
        let Some((_, size)) = table.iter().find(|(k, _)| k == rel) else {
            return false;
        };
        std::fs::metadata(env.weights.join(rel)).map(|m| m.len() == *size).unwrap_or(false)
    })
}

/// `os.path.realpath`: absolute, symlinks resolved for the part that exists.
pub fn realpath(path: &Path) -> PathBuf {
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    };
    let mut lexical = PathBuf::new();
    for comp in abs.components() {
        match comp {
            Component::ParentDir => {
                lexical.pop();
            }
            Component::CurDir => {}
            other => lexical.push(other.as_os_str()),
        }
    }
    if let Ok(real) = std::fs::canonicalize(&abs) {
        return real;
    }
    let mut tail = Vec::new();
    let mut head = lexical.clone();
    while !head.as_os_str().is_empty() {
        if let Ok(real) = std::fs::canonicalize(&head) {
            let mut out = real;
            for part in tail.iter().rev() {
                out.push(part);
            }
            return out;
        }
        match head.file_name() {
            Some(name) => tail.push(name.to_os_string()),
            None => break,
        }
        head.pop();
    }
    lexical
}

pub fn inside_tree(path: &Path, root: &Path) -> bool {
    let path = realpath(path);
    let root = realpath(root);
    path == root || path.starts_with(&root)
}

/// Days since 1970-01-01 to a civil date (Howard Hinnant's algorithm).
fn civil(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    ((if m <= 2 { y + 1 } else { y }) as i32, m, d)
}

fn today_utc() -> (i32, u32, u32) {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    civil(secs.div_euclid(86_400))
}

/// `YYYY-MM-DD` at the start of `value`.
pub fn expiry_date(value: &str) -> Option<(i32, u32, u32)> {
    let v = value.trim();
    let head = v.get(0..10)?;
    let b = head.as_bytes();
    if b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let y: i32 = head[0..4].parse().ok()?;
    let m: u32 = head[5..7].parse().ok()?;
    let d: u32 = head[8..10].parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    if let Some(c) = v[10..].chars().next() {
        if c.is_ascii_digit() {
            return None;
        }
    }
    Some((y, m, d))
}

/// SPDX-style name shown to the caller before a CC0 voice can be used.
pub const CC0_LICENCE_NAME: &str = "CC0 1.0 Universal (Public Domain Dedication)";
/// Licence text the UI links to.
pub const CC0_LICENCE_URL: &str = "https://creativecommons.org/publicdomain/zero/1.0/";

/// True when the record's `instrument` or `grantor` names CC0 or a
/// public-domain dedication.
pub fn consent_is_cc0(fields: &[(String, String)]) -> bool {
    ["instrument", "grantor"].iter().any(|k| {
        let v = field(fields, k).to_ascii_lowercase();
        v.contains("cc0") || v.contains("public-domain") || v.contains("public domain") || v.contains("publicdomain")
    })
}

/// CC0 licence acceptance, in memory only, registered once at clone /
/// registration time (owner rule: consent is checked once at clone time, in
/// Rust, never per synthesis turn). Keys are the sha256 of the session token
/// that ticked the box, so the token itself is never kept; nothing is
/// written to disk. Turns do not touch this store; the clone slot and the
/// persona floor read the cached `licensed` verdict.
#[derive(Debug, Default)]
pub struct Cc0Acceptances {
    inner: Mutex<Vec<String>>,
}

fn session_key(session: &str) -> Option<String> {
    let t = session.trim();
    if t.is_empty() {
        return None;
    }
    let mut h = sha256::Sha256::new();
    h.update(t.as_bytes());
    Some(h.hex())
}

impl Cc0Acceptances {
    pub const fn new() -> Self {
        Self { inner: Mutex::new(Vec::new()) }
    }
    /// The licence was accepted by this session (registration time).
    pub fn accept(&self, session: &str) -> bool {
        let Some(k) = session_key(session) else { return false };
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if !g.contains(&k) {
            g.push(k);
        }
        true
    }
    /// The acceptance was withdrawn.
    pub fn revoke(&self, session: &str) {
        let Some(k) = session_key(session) else { return };
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        g.retain(|x| *x != k);
    }
    pub fn accepted(&self, session: &str) -> bool {
        let Some(k) = session_key(session) else { return false };
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).contains(&k)
    }
    /// Clone-time verdict: has the CC0 licence been accepted and registered?
    /// A CC0 voice can be selected only when this is true.
    pub fn licensed(&self) -> bool {
        !self.inner.lock().unwrap_or_else(|e| e.into_inner()).is_empty()
    }
    pub fn clear(&self) {
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }
}

/// The process-wide CC0 acceptance store (web layer writes, clone slot reads).
pub fn cc0_acceptances() -> &'static Cc0Acceptances {
    static GLOBAL: OnceLock<Cc0Acceptances> = OnceLock::new();
    GLOBAL.get_or_init(Cc0Acceptances::new)
}

/// Registration / clone time: record the CC0 acceptance and run the clone
/// gate once for the CC0 voices (in process, Rust). Nothing CC0-related runs
/// per turn or per synthesis afterwards.
pub fn cc0_register(session: &str, env: &Env) -> bool {
    if !cc0_acceptances().accept(session) {
        return false;
    }
    for voice in ["samantha", "jarvis"] {
        let _ = clone_gate(voice, env);
    }
    true
}

/// Withdraw a CC0 acceptance; CC0 voices close once none remains.
pub fn cc0_unregister(session: &str) {
    cc0_acceptances().revoke(session);
}

/// Selection / clone-time gate: a CC0 voice is refused unless the CC0
/// licence was accepted at registration. A non-CC0 approval passes unchanged.
pub fn cc0_use_gate(approval: &Approval, accepted: bool) -> Result<(), CloneRefused> {
    if approval.cc0 && !accepted {
        return Err(CloneRefused { persona: approval.persona.to_string(), reason: "cc0_licence" });
    }
    Ok(())
}

/// Whether Piper fine-tuning is allowed for this persona under the owner
/// addendum (t662s3211). Checks the existing consent gate (without Nano
/// weights), then the addendum file: persona match, scope
/// [`SCOPE_PIPER_TRAINING`], and the decision card. Only `samantha` and
/// `jarvis` can pass. Never creates or edits consent text.
pub fn piper_training_allowed(persona: &str, env: &Env) -> Result<(), CloneRefused> {
    if blocked(persona) {
        return Err(CloneRefused { persona: persona.trim().to_lowercase(), reason: "celebrity" });
    }
    let Some(name) = persona_of(persona) else {
        return Err(CloneRefused { persona: persona.trim().to_lowercase(), reason: "persona" });
    };
    let gate = evaluate(name, env, false);
    if !gate.ok {
        return Err(CloneRefused { persona: name.to_string(), reason: gate.reason });
    }
    if !gate.cc0 {
        return Err(CloneRefused { persona: name.to_string(), reason: "not_cc0" });
    }
    let Some(store) = env.store.as_ref() else {
        return Err(CloneRefused { persona: name.to_string(), reason: "store" });
    };
    let path = store.join(name).join(PIPER_TRAINING_ADDENDUM);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Err(CloneRefused { persona: name.to_string(), reason: "addendum" });
    };
    let fields = parse_consent(&text);
    if field(&fields, "persona").trim().to_lowercase() != name {
        return Err(CloneRefused { persona: name.to_string(), reason: "addendum" });
    }
    if field(&fields, "scope") != SCOPE_PIPER_TRAINING {
        return Err(CloneRefused { persona: name.to_string(), reason: "addendum" });
    }
    let decision = field(&fields, "decision").trim().to_lowercase();
    if decision != "t662s3211" {
        return Err(CloneRefused { persona: name.to_string(), reason: "addendum" });
    }
    if ["persona", "performer"].iter().any(|k| blocked(field(&fields, k))) {
        return Err(CloneRefused { persona: name.to_string(), reason: "celebrity" });
    }
    Ok(())
}

/// The gate. See the crate docs for the order. Never spawns.
pub fn evaluate(persona: &str, env: &Env, need_weights: bool) -> Gate {
    EVALUATIONS.fetch_add(1, Ordering::SeqCst);
    if blocked(persona) {
        return Gate::absent("celebrity");
    }
    let Some(name) = persona_of(persona) else {
        return Gate::absent("persona");
    };
    let Some(store) = env.store.as_ref() else {
        return Gate::absent("store");
    };
    if inside_tree(store, &env.repo) {
        return Gate::absent("inside_repo");
    }
    let folder = store.join(name);
    let wav = folder.join("reference.wav");
    if !wav.is_file() || inside_tree(&wav, &env.repo) {
        return Gate::absent("wav");
    }
    if blocked(&realpath(&wav).to_string_lossy()) {
        return Gate::absent("celebrity");
    }
    let Some((seconds, channels, width)) = wav_info(&wav) else {
        return Gate::absent("wav");
    };
    if channels != 1 || width != 2 {
        return Gate::absent("mono");
    }
    if !(DURATION_MIN..=DURATION_MAX).contains(&seconds) {
        return Gate::absent("duration");
    }
    let Ok(text) = std::fs::read_to_string(folder.join("consent.txt")) else {
        return Gate::absent("consent");
    };
    let fields = parse_consent(&text);
    if REQUIRED.iter().any(|k| value_rejected(field(&fields, k))) {
        return Gate::absent("consent");
    }
    if ["persona", "performer", "work", "grantor"].iter().any(|k| blocked(field(&fields, k))) {
        return Gate::absent("celebrity");
    }
    if field(&fields, "persona").trim().to_lowercase() != name {
        return Gate::absent("consent");
    }
    if field(&fields, "scope") != SCOPE {
        return Gate::absent("consent");
    }
    let public = field(&fields, "public_demo").trim().to_lowercase();
    if public != "yes" && public != "no" {
        return Gate::absent("consent");
    }
    let date = field(&fields, "date");
    if !date.contains("CET") && !date.contains("Zagreb") {
        return Gate::absent("consent");
    }
    let Some(declared) = py_float(field(&fields, "duration_s")) else {
        return Gate::absent("consent");
    };
    if let Some(until) = expiry_date(field(&fields, "expiry")) {
        if until < env.today.unwrap_or_else(today_utc) {
            return Gate::absent("expired");
        }
    }
    if (declared - seconds).abs() > DURATION_TOLERANCE {
        return Gate::absent("duration");
    }
    let digest = field(&fields, "sha256").trim().to_lowercase();
    if digest.len() != 64 || !digest.chars().all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)) {
        return Gate::absent("sha");
    }
    let side = folder.join("reference.wav.sha256");
    if read_sidecar(&side) != digest || sha256::file_hex(&wav).ok().as_deref() != Some(digest.as_str()) {
        return Gate::absent("sha");
    }
    let Ok(lock_text) = std::fs::read_to_string(&env.locks) else {
        return Gate::absent("lock");
    };
    if !reference_clips_allowed(&lock_text) {
        return Gate::absent("lock");
    }
    if !env.worker.is_file() {
        return Gate::absent("worker");
    }
    if need_weights && !weights_sized(env) {
        return Gate::absent("weights");
    }
    Gate {
        ok: true,
        reason: "ready",
        seconds: Some(seconds),
        public: public == "yes",
        cc0: consent_is_cc0(&fields),
        wav: Some(realpath(&wav)),
        sha256: Some(digest),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_dates() {
        assert_eq!(civil(0), (1970, 1, 1));
        assert_eq!(civil(20_761), (2026, 11, 4));
        assert_eq!(expiry_date("2027-02-01 Europe/Zagreb"), Some((2027, 2, 1)));
        assert_eq!(expiry_date("on withdrawal"), None);
        assert_eq!(expiry_date("2027-02-011"), None);
    }

    #[test]
    fn lock_table_only() {
        assert!(!reference_clips_allowed("reference_clips = true\n"));
        assert!(reference_clips_allowed("[formats.exceptions.v2s14_chatterbox]\nreference_clips = true # ok\n"));
        assert!(!reference_clips_allowed("[formats.exceptions.v2s14_chatterbox]\nreference_clips = false\n"));
        assert!(!reference_clips_allowed("[formats.exceptions.v2s14_chatterbox]\n[x]\nreference_clips = true\n"));
    }

    #[test]
    fn consent_lines() {
        let f = parse_consent("# c\nDate: 2026-11-04 02:39 CET\npublic demo = no\nx\n");
        assert_eq!(field(&f, "date"), "2026-11-04 02:39 CET");
        assert_eq!(field(&f, "public_demo"), "no");
        assert!(value_rejected("TBD") && value_rejected("[fill]") && value_rejected(""));
        assert!(!value_rejected("Maciej Grajczyk"));
        assert_eq!(py_float("1_0.5"), Some(10.5));
        assert_eq!(py_float("_1"), None);
    }

    #[test]
    fn armstrong_is_blocked_before_anything_else() {
        let env = Env::at(Path::new("/nonexistent"));
        assert_eq!(evaluate("armstrong", &env, true).reason, "celebrity");
        assert_eq!(evaluate("Neil-Armstrong", &env, true).reason, "celebrity");
        let reg = Registry::new();
        let err = clone_gate_into(&reg, "armstrong", &env).unwrap_err();
        assert!(err.to_string().contains("celebrity"), "{err}");
        assert!(reg.lookup("armstrong").is_none());
    }

    #[test]
    fn cc0_voice_refused_until_the_licence_is_registered() {
        let a = Approval { persona: "samantha", seconds: 9.6, public: false, cc0: true, wav: PathBuf::new(), sha256: String::new() };
        let store = Cc0Acceptances::new();
        // Not registered: refused at selection.
        let e = cc0_use_gate(&a, store.licensed()).unwrap_err();
        assert_eq!(e.reason, "cc0_licence");
        assert!(e.to_string().contains("CC0"));
        // Registered once (clone time): allowed, with no per-turn check.
        assert!(store.accept("tok-1"));
        assert!(store.accepted("tok-1") && !store.accepted("tok-2"));
        assert!(cc0_use_gate(&a, store.licensed()).is_ok());
        // Withdrawn: refused again.
        store.revoke("tok-1");
        assert!(cc0_use_gate(&a, store.licensed()).is_err());
        // Empty token never registers.
        assert!(!store.accept("  "));
        assert!(!store.licensed());
        // A non-CC0 voice does not need the licence.
        let signed = Approval { cc0: false, ..a };
        assert!(cc0_use_gate(&signed, false).is_ok());
    }

    #[test]
    fn cc0_store_keeps_no_token() {
        let store = Cc0Acceptances::new();
        store.accept("secret-token-value");
        let dbg = format!("{store:?}");
        assert!(!dbg.contains("secret-token-value"), "{dbg}");
    }

    #[test]
    fn cc0_is_read_from_the_consent_record() {
        let cc0 = parse_consent("instrument: Public-domain dedication (LibriVox) plus CC0 1.0\ngrantor: x\n");
        assert!(consent_is_cc0(&cc0));
        let signed = parse_consent("instrument: signed release 2026-11-01\ngrantor: the performer\n");
        assert!(!consent_is_cc0(&signed));
    }
}
