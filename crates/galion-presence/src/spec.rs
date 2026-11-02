// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! `presence.json` schema: typed model + validator.
//!
//! Rules enforced here (A13, FINAL-BUILD-SPEC S8):
//! * every spec has `must` (blocking) and `should` (advisory) assertions;
//! * assertions may only name metrics from [`METRICS`];
//! * **no engagement metric** — any key, metric or assertion containing a forbidden word fails validation;
//! * `attunement` is the presence-quality metric instead;
//! * `lean_in_ms` exists as a Rika placeholder (null until A10).

use crate::json::{self, Value};
use crate::wav;
use std::path::Path;

pub const SCHEMA: &str = "galion-presence/v1";

/// Metrics a spec may assert on. Aggregates (`*_p50_ms`, `*_p95_ms`) only at run level.
pub const METRICS: &[&str] = &[
    "barge_in_ms",
    "onset_ms",
    "e_to_a_frames",
    "e_to_a_ms",
    "attunement",
    "affect_out_changes",
    "me_onsets_while_user_speaking",
    "late_frames",
    "lean_in_ms",
];
pub const AGGREGATE_METRICS: &[&str] = &["onset_p50_ms", "onset_p95_ms"];

/// Words that may never appear in a metric, spec key or assertion (A4 invariant 1).
pub const FORBIDDEN: &[&str] = &["engagement", "retention", "session_length", "time_in_session", "dwell", "speech_elicited"];

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Op {
    Le,
    Lt,
    Ge,
    Gt,
    Eq,
}

impl Op {
    pub fn holds(self, lhs: f64, rhs: f64) -> bool {
        match self {
            Op::Le => lhs <= rhs,
            Op::Lt => lhs < rhs,
            Op::Ge => lhs >= rhs,
            Op::Gt => lhs > rhs,
            Op::Eq => (lhs - rhs).abs() < 1e-9,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Op::Le => "<=",
            Op::Lt => "<",
            Op::Ge => ">=",
            Op::Gt => ">",
            Op::Eq => "==",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Assertion {
    pub metric: String,
    pub op: Op,
    pub value: f64,
}

impl Assertion {
    pub fn parse(s: &str) -> Result<Self, String> {
        for (tok, op) in [("<=", Op::Le), (">=", Op::Ge), ("==", Op::Eq), ("<", Op::Lt), (">", Op::Gt)] {
            if let Some(idx) = s.find(tok) {
                let metric = s[..idx].trim().to_string();
                let rhs = s[idx + tok.len()..].trim();
                let value: f64 = rhs.parse().map_err(|_| format!("assertion `{s}`: bad number `{rhs}`"))?;
                if metric.is_empty() {
                    return Err(format!("assertion `{s}`: missing metric"));
                }
                return Ok(Assertion { metric, op, value });
            }
        }
        Err(format!("assertion `{s}`: no operator (<=, <, >=, >, ==)"))
    }
    pub fn text(&self) -> String {
        format!("{} {} {}", self.metric, self.op.as_str(), Value::Num(self.value).to_json())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YouState {
    Silence,
    Breath,
    Speech,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Range {
    pub from: u32,
    pub to: u32, // exclusive
}

#[derive(Debug, Clone, PartialEq)]
pub struct YouSeg {
    pub range: Range,
    pub state: YouState,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AffectKey {
    pub at: u32,
    pub valence: f32,
    pub arousal: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    Personal,
    Workplace,
    Education,
}

impl Purpose {
    pub fn prohibited(self) -> bool {
        matches!(self, Purpose::Workplace | Purpose::Education)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Spec {
    pub id: String,
    pub frames: u32,
    pub sense: bool,
    pub adapt: bool,
    pub purpose: Purpose,
    pub you: Vec<YouSeg>,
    pub me_planned: Vec<Range>,
    pub affect_in: Vec<AffectKey>,
    pub late: Vec<u32>,
    pub rika_lean_in_ms: Option<f64>,
    pub must: Vec<Assertion>,
    pub should: Vec<Assertion>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Targets {
    pub barge_in_max_ms: f64,
    pub onset_p50_max_ms: f64,
    pub affect_to_voice_max_frames: u32,
    pub affect_to_voice_max_ms: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PresenceFile {
    pub frame_ms: u32,
    pub verdict_budget_s: u64,
    pub targets: Targets,
    pub must: Vec<Assertion>,
    pub should: Vec<Assertion>,
    pub specs: Vec<Spec>,
}

pub fn contains_forbidden(s: &str) -> Option<&'static str> {
    let l = s.to_ascii_lowercase();
    FORBIDDEN.iter().copied().find(|w| l.contains(w))
}

fn num(v: &Value, key: &str, ctx: &str) -> Result<f64, String> {
    v.get(key).and_then(Value::as_f64).ok_or_else(|| format!("{ctx}: missing number `{key}`"))
}

fn frame_idx(x: f64, ctx: &str) -> Result<u32, String> {
    if x < 0.0 || x.fract() != 0.0 || x > 1_000_000.0 {
        return Err(format!("{ctx}: frame index must be a non-negative integer, got {x}"));
    }
    Ok(x as u32)
}

fn assertions(v: &Value, key: &str, ctx: &str, allow_aggregate: bool) -> Result<Vec<Assertion>, String> {
    let arr = v.get(key).and_then(Value::as_arr).ok_or_else(|| format!("{ctx}: missing array `{key}` (must/should are required, may be empty)"))?;
    let mut out = Vec::new();
    for a in arr {
        let s = a.as_str().ok_or_else(|| format!("{ctx}: `{key}` entries must be strings"))?;
        if let Some(w) = contains_forbidden(s) {
            return Err(format!("{ctx}: assertion `{s}` uses forbidden metric word `{w}` (no engagement metric)"));
        }
        let asrt = Assertion::parse(s).map_err(|e| format!("{ctx}: {e}"))?;
        let known = METRICS.contains(&asrt.metric.as_str()) || (allow_aggregate && AGGREGATE_METRICS.contains(&asrt.metric.as_str()));
        if !known {
            return Err(format!("{ctx}: unknown metric `{}`", asrt.metric));
        }
        out.push(asrt);
    }
    Ok(out)
}

/// Recursively reject forbidden words in any object key or string value of a spec.
fn scan_forbidden(v: &Value, path: &str) -> Result<(), String> {
    match v {
        Value::Obj(kv) => {
            for (k, vv) in kv {
                if let Some(w) = contains_forbidden(k) {
                    return Err(format!("{path}.{k}: forbidden key word `{w}` (no engagement metric)"));
                }
                scan_forbidden(vv, &format!("{path}.{k}"))?;
            }
        }
        Value::Arr(a) => {
            for (i, vv) in a.iter().enumerate() {
                scan_forbidden(vv, &format!("{path}[{i}]"))?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn range(v: &Value, ctx: &str, frames: u32) -> Result<Range, String> {
    let from = frame_idx(num(v, "from", ctx)?, ctx)?;
    let to = frame_idx(num(v, "to", ctx)?, ctx)?;
    if from >= to || to > frames {
        return Err(format!("{ctx}: need from < to <= frames ({from}..{to}, frames {frames})"));
    }
    Ok(Range { from, to })
}

fn parse_spec(v: &Value, idx: usize, base: &Path) -> Result<Spec, String> {
    let id = v.get("id").and_then(Value::as_str).ok_or_else(|| format!("specs[{idx}]: missing `id`"))?.to_string();
    let ctx = format!("spec {id}");
    scan_forbidden(v, &ctx)?;
    if let Some(w) = contains_forbidden(&id) {
        return Err(format!("{ctx}: forbidden word `{w}` in id"));
    }
    // Optional audio source for the you-band (A13 wave 2): per-frame states from a PCM WAV.
    let wav_states = match v.get("you_wav") {
        None | Some(Value::Null) => None,
        Some(w) => Some(load_you_wav(w, base, &ctx)?),
    };
    let frames = match (v.get("frames"), &wav_states) {
        (None, Some(st)) => st.len() as u32,
        _ => frame_idx(num(v, "frames", &ctx)?, &ctx)?,
    };
    if frames == 0 || frames > 75_000 {
        return Err(format!("{ctx}: frames must be 1..=75000 (100 min at 80 ms)"));
    }
    let consent = v.get("consent").and_then(Value::as_arr).ok_or_else(|| format!("{ctx}: missing `consent` array (default deny = [])"))?;
    let mut sense = false;
    let mut adapt = false;
    for c in consent {
        match c.as_str() {
            Some("sense") => sense = true,
            Some("adapt") => adapt = true,
            Some("remember") | Some("share") => {}
            _ => return Err(format!("{ctx}: unknown consent bit {}", c.to_json())),
        }
    }
    let purpose = match v.get("purpose").and_then(Value::as_str).unwrap_or("personal") {
        "personal" => Purpose::Personal,
        "workplace" => Purpose::Workplace,
        "education" => Purpose::Education,
        p => return Err(format!("{ctx}: unknown purpose `{p}`")),
    };
    let mut you = Vec::new();
    for (i, s) in v.get("you").and_then(Value::as_arr).unwrap_or(&[]).iter().enumerate() {
        let c = format!("{ctx}.you[{i}]");
        let state = match s.get("state").and_then(Value::as_str) {
            Some("speech") => YouState::Speech,
            Some("breath") => YouState::Breath,
            Some("silence") => YouState::Silence,
            _ => return Err(format!("{c}: state must be speech|breath|silence")),
        };
        you.push(YouSeg { range: range(s, &c, frames)?, state });
    }
    if let Some(st) = &wav_states {
        if !you.is_empty() {
            return Err(format!("{ctx}: `you` and `you_wav` are mutually exclusive"));
        }
        you = segments(st, frames);
    }
    let mut me_planned = Vec::new();
    for (i, s) in v.get("me_planned").and_then(Value::as_arr).unwrap_or(&[]).iter().enumerate() {
        me_planned.push(range(s, &format!("{ctx}.me_planned[{i}]"), frames)?);
    }
    let mut affect_in = Vec::new();
    for (i, k) in v.get("affect_in").and_then(Value::as_arr).unwrap_or(&[]).iter().enumerate() {
        let c = format!("{ctx}.affect_in[{i}]");
        let at = frame_idx(num(k, "at", &c)?, &c)?;
        let valence = num(k, "valence", &c)? as f32;
        let arousal = num(k, "arousal", &c)? as f32;
        if !(-1.0..=1.0).contains(&valence) || !(0.0..=1.0).contains(&arousal) || at >= frames {
            return Err(format!("{c}: valence in [-1,1], arousal in [0,1], at < frames"));
        }
        if affect_in.last().is_some_and(|p: &AffectKey| p.at >= at) {
            return Err(format!("{c}: keys must be strictly increasing in `at`"));
        }
        affect_in.push(AffectKey { at, valence, arousal });
    }
    let mut late = Vec::new();
    for l in v.get("late").and_then(Value::as_arr).unwrap_or(&[]) {
        let f = frame_idx(l.as_f64().ok_or_else(|| format!("{ctx}: late entries are frame numbers"))?, &ctx)?;
        if f >= frames {
            return Err(format!("{ctx}: late frame {f} out of range"));
        }
        late.push(f);
    }
    let rika_lean_in_ms = match v.get("rika").and_then(|r| r.get("lean_in_ms")) {
        None | Some(Value::Null) => None,
        Some(x) => Some(x.as_f64().ok_or_else(|| format!("{ctx}: rika.lean_in_ms must be number or null"))?),
    };
    Ok(Spec {
        must: assertions(v, "must", &ctx, false)?,
        should: assertions(v, "should", &ctx, false)?,
        id,
        frames,
        sense,
        adapt,
        purpose,
        you,
        me_planned,
        affect_in,
        late,
        rika_lean_in_ms,
    })
}

/// Read `you_wav` (a path string, or `{"path", "speech_dbfs"?, "breath_dbfs"?}`) relative to `base`.
fn load_you_wav(w: &Value, base: &Path, ctx: &str) -> Result<Vec<YouState>, String> {
    let c = format!("{ctx}.you_wav");
    let (path, mut gate) = match w {
        Value::Str(p) => (p.as_str(), wav::Gate::default()),
        Value::Obj(_) => (w.get("path").and_then(Value::as_str).ok_or_else(|| format!("{c}: missing string `path`"))?, wav::Gate::default()),
        _ => return Err(format!("{c}: must be a path string or an object with `path`")),
    };
    if let Some(x) = w.get("speech_dbfs") {
        gate.speech_dbfs = x.as_f64().ok_or_else(|| format!("{c}: speech_dbfs must be a number"))? as f32;
    }
    if let Some(x) = w.get("breath_dbfs") {
        gate.breath_dbfs = x.as_f64().ok_or_else(|| format!("{c}: breath_dbfs must be a number"))? as f32;
    }
    if !(gate.breath_dbfs < gate.speech_dbfs && gate.speech_dbfs <= 0.0) {
        return Err(format!("{c}: need breath_dbfs < speech_dbfs <= 0"));
    }
    let pcm = wav::read_wav(&base.join(path)).map_err(|e| format!("{c}: {e}"))?;
    let st = wav::you_states(&pcm, 80, gate).map_err(|e| format!("{c}: {e}"))?;
    if st.is_empty() || st.len() > 75_000 {
        return Err(format!("{c}: audio must be 1..=75000 frames of 80 ms"));
    }
    Ok(st)
}

/// Run-length encode per-frame states into you-segments (silence is implicit), padded/truncated to `frames`.
fn segments(st: &[YouState], frames: u32) -> Vec<YouSeg> {
    let n = (frames as usize).min(st.len());
    let mut out = Vec::new();
    let mut t = 0;
    while t < n {
        let s = st[t];
        let mut u = t + 1;
        while u < n && st[u] == s {
            u += 1;
        }
        if s != YouState::Silence {
            out.push(YouSeg { range: Range { from: t as u32, to: u as u32 }, state: s });
        }
        t = u;
    }
    out
}

/// Parse a presence file whose `you_wav` paths resolve against the current directory.
pub fn parse_presence(src: &str) -> Result<PresenceFile, String> {
    parse_presence_at(src, Path::new("."))
}

/// Parse a presence file; `you_wav` paths resolve against `base` (normally the spec file's directory).
pub fn parse_presence_at(src: &str, base: &Path) -> Result<PresenceFile, String> {
    let root = json::parse(src).map_err(|e| e.to_string())?;
    if root.get("schema").and_then(Value::as_str) != Some(SCHEMA) {
        return Err(format!("schema must be \"{SCHEMA}\""));
    }
    let frame_ms = num(&root, "frame_ms", "root")?;
    if frame_ms != 80.0 {
        return Err("frame_ms must be 80 (LOCKS.toml [clock], not configurable)".into());
    }
    // Forbidden words: metric registry and every spec are scanned.
    if let Some(m) = root.get("metrics").and_then(Value::as_obj) {
        for (k, _) in m {
            if let Some(w) = contains_forbidden(k) {
                return Err(format!("metrics.{k}: forbidden metric word `{w}` (no engagement metric)"));
            }
        }
        if !m.iter().any(|(k, _)| k == "attunement") {
            return Err("metrics: `attunement` must be declared (it replaces engagement)".into());
        }
    } else {
        return Err("root: missing `metrics` registry".into());
    }
    let t = root.get("targets").ok_or("root: missing `targets`")?;
    let targets = Targets {
        barge_in_max_ms: num(t, "barge_in_max_ms", "targets")?,
        onset_p50_max_ms: num(t, "onset_p50_max_ms", "targets")?,
        affect_to_voice_max_frames: frame_idx(num(t, "affect_to_voice_max_frames", "targets")?, "targets")?,
        affect_to_voice_max_ms: num(t, "affect_to_voice_max_ms", "targets")?,
    };
    let verdict_budget_s = num(&root, "verdict_budget_s", "root")?;
    if !(1.0..=300.0).contains(&verdict_budget_s) {
        return Err("verdict_budget_s must be 1..=300 (5-minute commit verdict)".into());
    }
    let specs_v = root.get("specs").and_then(Value::as_arr).ok_or("root: missing `specs`")?;
    if specs_v.is_empty() {
        return Err("specs: empty (zero specs = fail)".into());
    }
    let mut specs = Vec::new();
    for (i, s) in specs_v.iter().enumerate() {
        let spec = parse_spec(s, i, base)?;
        if specs.iter().any(|x: &Spec| x.id == spec.id) {
            return Err(format!("duplicate spec id `{}`", spec.id));
        }
        specs.push(spec);
    }
    Ok(PresenceFile {
        frame_ms: 80,
        verdict_budget_s: verdict_budget_s as u64,
        targets,
        must: assertions(&root, "must", "root", true)?,
        should: assertions(&root, "should", "root", true)?,
        specs,
    })
}
