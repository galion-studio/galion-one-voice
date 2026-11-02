// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! Deterministic CPU replay: expand a spec into frames, drive a backend, measure.

use crate::backend::{AffectIn, AffectOut, FrameIn, FrameOut, ReplayBackend};
use crate::spec::{Assertion, PresenceFile, Spec, YouState};

/// Minimum |delta| on valence or arousal that counts as a scripted E-in shift.
pub const SHIFT_MIN: f32 = 0.2;
/// Minimum |delta| on any E-out channel that counts as an E-out change.
pub const OUT_EPS: f32 = 1e-4;

/// Pre-expanded per-frame inputs (allocated once, before the loop).
pub fn expand(spec: &Spec) -> Vec<FrameIn> {
    let n = spec.frames as usize;
    let gate_open = spec.sense && !spec.purpose.prohibited();
    let adapt = spec.adapt && spec.sense && !spec.purpose.prohibited();
    let mut ins = Vec::with_capacity(n);
    let mut cur = AffectIn { valence: 0.0, arousal: 0.3 };
    let mut k = 0;
    for t in 0..spec.frames {
        while k < spec.affect_in.len() && spec.affect_in[k].at <= t {
            cur = AffectIn { valence: spec.affect_in[k].valence, arousal: spec.affect_in[k].arousal };
            k += 1;
        }
        let you = spec
            .you
            .iter()
            .rev()
            .find(|s| s.range.from <= t && t < s.range.to)
            .map(|s| s.state)
            .unwrap_or(YouState::Silence);
        let me_planned = spec.me_planned.iter().any(|r| r.from <= t && t < r.to);
        ins.push(FrameIn { index: t, you, affect_in: gate_open.then_some(cur), adapt, late: spec.late.contains(&t), me_planned });
    }
    ins
}

/// Metrics for one spec. `None` = not measured in this spec.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Metrics {
    pub barge_in_ms: Option<f64>,
    pub onset_ms: Option<f64>,
    pub onset_events_ms: Vec<f64>,
    pub e_to_a_frames: Option<f64>,
    pub e_to_a_ms: Option<f64>,
    pub attunement: Option<f64>,
    pub affect_out_changes: f64,
    pub me_onsets_while_user_speaking: f64,
    pub late_frames: f64,
    pub lean_in_ms: Option<f64>,
    pub rika_supported: bool,
    pub barge_in_events: u32,
    pub e_shift_events: u32,
}

impl Metrics {
    pub fn get(&self, name: &str) -> Option<f64> {
        match name {
            "barge_in_ms" => self.barge_in_ms,
            "onset_ms" => self.onset_ms,
            "e_to_a_frames" => self.e_to_a_frames,
            "e_to_a_ms" => self.e_to_a_ms,
            "attunement" => self.attunement,
            "affect_out_changes" => Some(self.affect_out_changes),
            "me_onsets_while_user_speaking" => Some(self.me_onsets_while_user_speaking),
            "late_frames" => Some(self.late_frames),
            "lean_in_ms" => self.lean_in_ms,
            _ => None,
        }
    }
}

fn changed(a: AffectOut, b: AffectOut) -> bool {
    (a.valence - b.valence).abs() > OUT_EPS || (a.arousal - b.arousal).abs() > OUT_EPS || (a.warmth - b.warmth).abs() > OUT_EPS
}

fn max_opt(cur: Option<f64>, v: f64) -> Option<f64> {
    Some(cur.map_or(v, |c| c.max(v)))
}

/// Measure a trace. Pure function of (inputs, outputs): deterministic.
pub fn measure(spec: &Spec, ins: &[FrameIn], outs: &[FrameOut], frame_ms: f64, e2a_budget: u32, rika_supported: bool) -> Metrics {
    let n = ins.len();
    let mut m = Metrics { rika_supported, late_frames: ins.iter().filter(|f| f.late).count() as f64, ..Default::default() };
    let voiced = |t: usize| ins[t].you == YouState::Speech;

    for t in 0..n {
        let you_rise = voiced(t) && (t == 0 || !voiced(t - 1));
        let me_rise = outs[t].me_voiced && (t == 0 || !outs[t - 1].me_voiced);
        // Barge-in: user starts voicing while me is speaking (me voiced on the previous frame or this one).
        if you_rise && (outs[t].me_voiced || (t > 0 && outs[t - 1].me_voiced)) {
            m.barge_in_events += 1;
            let end = (t..n).find(|&u| !outs[u].me_voiced);
            let frames = end.map_or((n - t) as f64, |u| (u - t) as f64);
            m.barge_in_ms = max_opt(m.barge_in_ms, frames * frame_ms);
        }
        if me_rise && voiced(t) {
            m.me_onsets_while_user_speaking += 1.0;
        }
        // Onset: end of a user turn (first silent frame after voicing) -> first me rise.
        if t > 0 && voiced(t - 1) && !voiced(t) {
            // only if the user does not resume before me answers
            let mut found = None;
            for u in t..n {
                if voiced(u) {
                    break;
                }
                if outs[u].me_voiced && (u == 0 || !outs[u - 1].me_voiced || u == t) {
                    found = Some(u);
                    break;
                }
            }
            match found {
                Some(u) => {
                    let ms = (u - t) as f64 * frame_ms;
                    m.onset_events_ms.push(ms);
                    m.onset_ms = max_opt(m.onset_ms, ms);
                }
                None => {
                    // user turn ended and nobody answered before the next turn / end of spec
                    let resumed = (t..n).any(voiced);
                    if !resumed {
                        let ms = (n - t) as f64 * frame_ms;
                        m.onset_events_ms.push(ms);
                        m.onset_ms = max_opt(m.onset_ms, ms);
                    }
                }
            }
        }
        if t > 0 && changed(outs[t].affect_out, outs[t - 1].affect_out) {
            m.affect_out_changes += 1.0;
        }
    }

    // E -> A and attunement (only defined when the affect loop is consented).
    let consented = spec.sense && spec.adapt && !spec.purpose.prohibited();
    if consented {
        let mut responsive = 0u32;
        for t in 1..n {
            let (Some(a), Some(b)) = (ins[t - 1].affect_in, ins[t].affect_in) else { continue };
            if (a.valence - b.valence).abs() >= SHIFT_MIN || (a.arousal - b.arousal).abs() >= SHIFT_MIN {
                m.e_shift_events += 1;
                let resp = (t..n).find(|&u| u > 0 && changed(outs[u].affect_out, outs[u - 1].affect_out));
                let frames = resp.map_or((n - t) as f64, |u| (u - t) as f64);
                m.e_to_a_frames = max_opt(m.e_to_a_frames, frames);
                if resp.is_some() && frames <= e2a_budget as f64 {
                    responsive += 1;
                }
            }
        }
        m.e_to_a_ms = m.e_to_a_frames.map(|f| f * frame_ms);
        let mut ok = 0u32;
        let mut total = 0u32;
        for t in 0..n {
            if let Some(a) = ins[t].affect_in {
                total += 1;
                if outs[t].affect_out.arousal <= a.arousal.max(0.3) + OUT_EPS {
                    ok += 1;
                }
            }
        }
        let non_escalating = if total == 0 { 1.0 } else { ok as f64 / total as f64 };
        let resp_share = if m.e_shift_events == 0 { 1.0 } else { responsive as f64 / m.e_shift_events as f64 };
        m.attunement = Some(non_escalating.min(resp_share));
    }

    // Rika lean-in (placeholder until A10): evidence = first breath frame.
    if rika_supported {
        if let Some(ev) = ins.iter().position(|f| f.you == YouState::Breath) {
            m.lean_in_ms = (ev..n).find(|&u| outs[u].rika_lean_in).map(|u| (u - ev) as f64 * frame_ms);
        }
    }
    let _ = spec;
    m
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Pass,
    Fail,
    /// Metric not implemented by this backend yet (only `lean_in_ms` without Rika).
    Pending,
    /// Metric applies but produced no measurement (fails a `must`, warns a `should`).
    NotMeasured,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Pass => "pass",
            Status::Fail => "fail",
            Status::Pending => "pending",
            Status::NotMeasured => "not_measured",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Checked {
    pub assertion: Assertion,
    pub value: Option<f64>,
    pub status: Status,
}

pub fn check(a: &Assertion, value: Option<f64>, rika_supported: bool) -> Checked {
    let status = match value {
        Some(v) => {
            if a.op.holds(v, a.value) {
                Status::Pass
            } else {
                Status::Fail
            }
        }
        None if a.metric == "lean_in_ms" && !rika_supported => Status::Pending,
        None => Status::NotMeasured,
    };
    Checked { assertion: a.clone(), value, status }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpecResult {
    pub id: String,
    pub frames: u32,
    pub metrics: Metrics,
    pub must: Vec<Checked>,
    pub should: Vec<Checked>,
}

impl SpecResult {
    pub fn must_ok(&self) -> bool {
        self.must.iter().all(|c| matches!(c.status, Status::Pass | Status::Pending))
    }
}

/// Drive one spec through a backend: expand, reset, step every frame, measure, check.
pub fn run_spec<B: ReplayBackend + ?Sized>(backend: &mut B, spec: &Spec, file: &PresenceFile) -> SpecResult {
    let ins = expand(spec);
    let mut outs: Vec<FrameOut> = Vec::with_capacity(ins.len());
    backend.reset();
    for f in &ins {
        outs.push(backend.step(f)); // capacity reserved: no reallocation inside the loop
    }
    let rika = backend.supports_rika();
    let metrics = measure(spec, &ins, &outs, file.frame_ms as f64, file.targets.affect_to_voice_max_frames, rika);
    let must = spec.must.iter().map(|a| check(a, metrics.get(&a.metric), rika)).collect();
    let should = spec.should.iter().map(|a| check(a, metrics.get(&a.metric), rika)).collect();
    SpecResult { id: spec.id.clone(), frames: spec.frames, metrics, must, should }
}
