// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! # galion-presence (agent A13)
//!
//! Harness before model. Reads `specs/presence.json`, replays every spec frame by
//! frame (80 ms heartbeat) through a [`backend::ReplayBackend`] — today the
//! deterministic [`backend::MockBackend`] — and reports barge-in, onset and
//! E→A in frames/ms plus the **attunement** metric. There is deliberately **no
//! engagement metric**; the schema validator rejects one.
//!
//! Verdict: `green` iff every `must` passes (or is `pending` for a capability the
//! backend does not have yet, i.e. Rika `lean_in_ms`) and the run finished inside
//! the 5-minute verdict budget. `should` failures are warnings.
//!
//! Zero dependencies, CPU only, no weights, byte-identical report per input.

pub mod backend;
pub mod json;
pub mod replay;
pub mod spec;
pub mod wav;

use backend::ReplayBackend;
use json::Value;
use replay::{check, Checked, SpecResult, Status};
use spec::PresenceFile;
use std::time::{Duration, Instant};

pub const REPORT_SCHEMA: &str = "galion-presence-report/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Green,
    Red,
}

#[derive(Debug, Clone)]
pub struct RunReport {
    pub backend: &'static str,
    pub specs: Vec<SpecResult>,
    pub onset_p50_ms: Option<f64>,
    pub onset_p95_ms: Option<f64>,
    pub onset_events: usize,
    pub run_must: Vec<Checked>,
    pub run_should: Vec<Checked>,
    pub budget: Duration,
    pub elapsed: Duration,
    pub verdict: Verdict,
}

/// Nearest-rank percentile on a sorted slice.
pub fn percentile(sorted: &[f64], p: f64) -> Option<f64> {
    if sorted.is_empty() {
        return None;
    }
    let rank = ((p / 100.0) * sorted.len() as f64).ceil().max(1.0) as usize;
    Some(sorted[rank.min(sorted.len()) - 1])
}

/// Replay every spec in `file` through `backend`.
pub fn run<B: ReplayBackend + ?Sized>(file: &PresenceFile, backend: &mut B) -> RunReport {
    let start = Instant::now();
    let budget = Duration::from_secs(file.verdict_budget_s);
    let mut specs = Vec::with_capacity(file.specs.len());
    let mut over_budget = false;
    for s in &file.specs {
        if start.elapsed() > budget {
            over_budget = true;
            break;
        }
        specs.push(replay::run_spec(backend, s, file));
    }
    let mut onsets: Vec<f64> = specs.iter().flat_map(|r| r.metrics.onset_events_ms.iter().copied()).collect();
    onsets.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let onset_p50_ms = percentile(&onsets, 50.0);
    let onset_p95_ms = percentile(&onsets, 95.0);
    let agg = |name: &str| match name {
        "onset_p50_ms" => onset_p50_ms,
        "onset_p95_ms" => onset_p95_ms,
        _ => None,
    };
    let rika = backend.supports_rika();
    let run_must: Vec<Checked> = file.must.iter().map(|a| check(a, agg(&a.metric), rika)).collect();
    let run_should: Vec<Checked> = file.should.iter().map(|a| check(a, agg(&a.metric), rika)).collect();
    let elapsed = start.elapsed();
    over_budget |= elapsed > budget;
    let ok = !specs.is_empty()
        && specs.len() == file.specs.len()
        && !over_budget
        && specs.iter().all(SpecResult::must_ok)
        && run_must.iter().all(|c| matches!(c.status, Status::Pass | Status::Pending));
    RunReport {
        backend: backend.name(),
        specs,
        onset_p50_ms,
        onset_p95_ms,
        onset_events: onsets.len(),
        run_must,
        run_should,
        budget,
        elapsed,
        verdict: if ok { Verdict::Green } else { Verdict::Red },
    }
}

fn opt(v: Option<f64>) -> Value {
    v.map_or(Value::Null, Value::Num)
}

fn checked_json(c: &[Checked]) -> Value {
    Value::Arr(
        c.iter()
            .map(|c| {
                Value::Obj(vec![
                    ("assert".into(), Value::Str(c.assertion.text())),
                    ("value".into(), opt(c.value)),
                    ("status".into(), Value::Str(c.status.as_str().into())),
                ])
            })
            .collect(),
    )
}

/// FNV-1a 64-bit. Non-cryptographic; used only as a determinism fingerprint.
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

impl RunReport {
    /// Deterministic part of the report (no wall-clock values).
    pub fn deterministic_json(&self) -> Value {
        let specs = self
            .specs
            .iter()
            .map(|r| {
                let m = &r.metrics;
                Value::Obj(vec![
                    ("id".into(), Value::Str(r.id.clone())),
                    ("frames".into(), Value::Num(r.frames as f64)),
                    ("ok".into(), Value::Bool(r.must_ok())),
                    (
                        "metrics".into(),
                        Value::Obj(vec![
                            ("barge_in_ms".into(), opt(m.barge_in_ms)),
                            ("barge_in_events".into(), Value::Num(m.barge_in_events as f64)),
                            ("onset_ms".into(), opt(m.onset_ms)),
                            ("onset_events_ms".into(), Value::Arr(m.onset_events_ms.iter().map(|x| Value::Num(*x)).collect())),
                            ("e_to_a_frames".into(), opt(m.e_to_a_frames)),
                            ("e_to_a_ms".into(), opt(m.e_to_a_ms)),
                            ("e_shift_events".into(), Value::Num(m.e_shift_events as f64)),
                            ("attunement".into(), opt(m.attunement)),
                            ("affect_out_changes".into(), Value::Num(m.affect_out_changes)),
                            ("me_onsets_while_user_speaking".into(), Value::Num(m.me_onsets_while_user_speaking)),
                            ("late_frames".into(), Value::Num(m.late_frames)),
                            ("lean_in_ms".into(), opt(m.lean_in_ms)),
                            ("rika".into(), Value::Str(if m.rika_supported { "implemented" } else { "pending-A10" }.into())),
                        ]),
                    ),
                    ("must".into(), checked_json(&r.must)),
                    ("should".into(), checked_json(&r.should)),
                ])
            })
            .collect();
        Value::Obj(vec![
            ("schema".into(), Value::Str(REPORT_SCHEMA.into())),
            ("backend".into(), Value::Str(self.backend.into())),
            ("frame_ms".into(), Value::Num(80.0)),
            ("verdict".into(), Value::Str(if self.verdict == Verdict::Green { "green" } else { "red" }.into())),
            (
                "aggregate".into(),
                Value::Obj(vec![
                    ("onset_p50_ms".into(), opt(self.onset_p50_ms)),
                    ("onset_p95_ms".into(), opt(self.onset_p95_ms)),
                    ("onset_events".into(), Value::Num(self.onset_events as f64)),
                ]),
            ),
            ("must".into(), checked_json(&self.run_must)),
            ("should".into(), checked_json(&self.run_should)),
            ("specs".into(), Value::Arr(specs)),
        ])
    }

    pub fn digest(&self) -> String {
        format!("{:016x}", fnv1a64(self.deterministic_json().to_json().as_bytes()))
    }

    /// Full report: deterministic body + digest + timing (timing excluded from the digest).
    pub fn to_json(&self) -> String {
        let mut v = self.deterministic_json();
        if let Value::Obj(kv) = &mut v {
            kv.push(("digest_fnv1a64".into(), Value::Str(self.digest())));
            kv.push((
                "timing".into(),
                Value::Obj(vec![
                    ("elapsed_ms".into(), Value::Num(self.elapsed.as_secs_f64() * 1000.0)),
                    ("verdict_budget_s".into(), Value::Num(self.budget.as_secs() as f64)),
                    ("within_budget".into(), Value::Bool(self.elapsed <= self.budget)),
                ]),
            ));
        }
        v.to_json()
    }

    /// One line per spec, `NO_COLOR`-safe plain text.
    pub fn summary(&self) -> String {
        let mut s = String::new();
        let f = |v: Option<f64>| v.map_or("-".to_string(), |x| format!("{x}"));
        for r in &self.specs {
            let m = &r.metrics;
            let warn = r.should.iter().filter(|c| !matches!(c.status, Status::Pass | Status::Pending)).count();
            let pend = r.must.iter().chain(&r.should).filter(|c| c.status == Status::Pending).count();
            s.push_str(&format!(
                "{:<4} {:<30} barge={}ms onset={}ms e2a={}f attune={} lean_in={} warn={} pending={}\n",
                if r.must_ok() { "PASS" } else { "FAIL" },
                r.id,
                f(m.barge_in_ms),
                f(m.onset_ms),
                f(m.e_to_a_frames),
                f(m.attunement),
                f(m.lean_in_ms),
                warn,
                pend
            ));
            for c in r.must.iter().filter(|c| !matches!(c.status, Status::Pass | Status::Pending)) {
                s.push_str(&format!("     must {} -> {} ({})\n", c.assertion.text(), f(c.value), c.status.as_str()));
            }
        }
        s.push_str(&format!(
            "aggregate onset p50={}ms p95={}ms over {} events; verdict={} in {:.1} ms (budget {} s)\n",
            f(self.onset_p50_ms),
            f(self.onset_p95_ms),
            self.onset_events,
            if self.verdict == Verdict::Green { "GREEN" } else { "RED" },
            self.elapsed.as_secs_f64() * 1000.0,
            self.budget.as_secs()
        ));
        s
    }
}
