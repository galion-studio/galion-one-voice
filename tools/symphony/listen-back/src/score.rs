// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Layer scores against the fixture's intended shape. Each is in 0..=1.
//! Higher is closer. None of these is the rollout gate.

use galion_articulation::{score, Articulation};
use serde::{Deserialize, Serialize};

use crate::contract::Intent;
use crate::decode::{ReadBack, TICK_S};
use crate::fixture::BreathSpan;
use crate::render::hz;
use crate::round4;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct LayerScores {
    pub feeling_match: f64,
    pub breath_plan_match: f64,
    pub pitch_voice_quality: f64,
    pub articulation: f64,
    pub speaker_kept: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct FeelingView {
    pub intended_track: String,
    pub intended_category: String,
    pub intended_pitch_st: f64,
    pub readback_pitch_st: f64,
    pub readback_category: Option<String>,
    pub category_compared: bool,
    pub contour_error_st: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct BodyView {
    pub lungs_status: String,
    pub intended_inhales: Vec<BreathSpan>,
    pub readback_inhales: Vec<BreathSpan>,
    pub throat_status: String,
    pub mouth_status: String,
    pub nose_status: String,
    pub nose_todo: String,
    pub hnr_db: Option<f64>,
    pub f1_hz: Option<f64>,
    pub f2_hz: Option<f64>,
    pub f3_hz: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct BaselineView {
    /// The fixture baseline. The correction does not write this field.
    pub f0_hz: f64,
    pub readback_f0_hz: f64,
    pub profile_speaker_f0_hz: Option<f64>,
    pub steered: bool,
}

pub fn pitch_error_st(read: &ReadBack, intent: &Intent) -> Option<f64> {
    if read.f0_hz_median < 40.0 || intent.baseline_f0_hz <= 0.0 {
        return None;
    }
    let got = 12.0 * (read.f0_hz_median / intent.baseline_f0_hz).ln() / std::f64::consts::LN_2;
    Some(got - intent.pitch_st)
}

pub fn feeling(intent: &Intent, read: &ReadBack) -> (f64, FeelingView) {
    let mut errs = Vec::new();
    for hz in read.f0_hz_ticks.iter().flatten() {
        if *hz > 40.0 {
            let st = 12.0 * (*hz / intent.baseline_f0_hz).ln() / std::f64::consts::LN_2;
            errs.push((st - intent.pitch_st).abs());
        }
    }
    let contour_error = if errs.is_empty() {
        0.0
    } else {
        errs.iter().sum::<f64>() / errs.len() as f64
    };
    let contour = if errs.is_empty() {
        0.0
    } else {
        1.0 / (1.0 + contour_error)
    };
    let (category_compared, category_score) = match read.feeling_top.as_deref() {
        Some(top) => (true, if category_hit(intent, top) { 1.0 } else { 0.0 }),
        None => (false, 0.0),
    };
    let score = if category_compared {
        0.5 * contour + 0.5 * category_score
    } else {
        contour
    };
    let readback_pitch = if read.f0_hz_median < 40.0 {
        0.0
    } else {
        12.0 * (read.f0_hz_median / intent.baseline_f0_hz).ln() / std::f64::consts::LN_2
    };
    (
        round4(score),
        FeelingView {
            intended_track: intent.track.clone(),
            intended_category: intent.category.clone(),
            intended_pitch_st: round4(intent.pitch_st),
            readback_pitch_st: round4(readback_pitch),
            readback_category: read.feeling_top.clone(),
            category_compared,
            contour_error_st: round4(contour_error),
        },
    )
}

fn category_hit(intent: &Intent, top: &str) -> bool {
    if intent.track == "baseline" {
        top == "neutral" || top == "none" || top.is_empty()
    } else {
        top == intent.category
    }
}

/// Inhalations heard in the read-back, against the fixture's intended spans.
/// A tick counts when it has energy and the analyser did not call it voiced.
pub fn breath_events(voicing: &[f64], rms_db: &[f64]) -> Vec<BreathSpan> {
    let n = voicing.len().min(rms_db.len());
    let mut spans = Vec::new();
    let mut i = 0;
    while i < n {
        if is_breath(voicing[i], rms_db[i]) {
            let start = i;
            while i < n && is_breath(voicing[i], rms_db[i]) {
                i += 1;
            }
            let ticks = i - start;
            if ticks >= 2 {
                spans.push(BreathSpan {
                    start_s: round4(start as f64 * TICK_S),
                    dur_s: round4(ticks as f64 * TICK_S),
                });
            }
        } else {
            i += 1;
        }
    }
    spans
}

fn is_breath(voicing: f64, rms_db: f64) -> bool {
    voicing < 0.20 && rms_db > -50.0
}

pub fn breath_plan_match(intended: &[BreathSpan], heard: &[BreathSpan]) -> f64 {
    if intended.is_empty() && heard.is_empty() {
        return 1.0;
    }
    let denom = intended.len().max(heard.len()).max(1);
    let mut used = vec![false; heard.len()];
    let mut hit = 0;
    for want in intended {
        if let Some((j, _)) = heard
            .iter()
            .enumerate()
            .filter(|(j, _)| !used[*j])
            .map(|(j, got)| {
                (
                    j,
                    (got.start_s - want.start_s).abs() + (got.dur_s - want.dur_s).abs(),
                )
            })
            .filter(|(_, d)| *d <= 0.16)
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        {
            used[j] = true;
            hit += 1;
        }
    }
    round4(hit as f64 / denom as f64)
}

/// Plan inhalations from a `galion-lungs/1` document (PR #195), if one was
/// produced. Returns the inhale segments only.
pub fn lungs_inhales(plan: &serde_json::Value) -> Result<Vec<BreathSpan>, String> {
    let schema = plan.get("schema").and_then(|v| v.as_str()).unwrap_or("");
    if schema != "galion-lungs/1" {
        return Err(format!(
            "lungs plan schema is {schema:?}, want galion-lungs/1"
        ));
    }
    if plan.get("not_emotion").and_then(|v| v.as_bool()) == Some(false) {
        return Err("lungs plan marked itself as an emotion".into());
    }
    let segments = plan
        .get("segments")
        .and_then(|v| v.as_array())
        .ok_or("lungs plan missing segments")?;
    let mut out = Vec::new();
    for seg in segments {
        if seg.get("kind").and_then(|v| v.as_str()) != Some("inhale") {
            continue;
        }
        let start = seg
            .get("start_s")
            .and_then(|v| v.as_f64())
            .ok_or("inhale missing start_s")?;
        let end = seg
            .get("end_s")
            .and_then(|v| v.as_f64())
            .ok_or("inhale missing end_s")?;
        out.push(BreathSpan {
            start_s: round4(start),
            dur_s: round4((end - start).max(0.0)),
        });
    }
    Ok(out)
}

pub fn pitch_voice_quality(err_st: Option<f64>, hnr_db: Option<f64>) -> f64 {
    let pitch = match err_st {
        Some(e) => 1.0 / (1.0 + e.abs()),
        None => 0.0,
    };
    let hnr = match hnr_db {
        Some(h) if h.is_finite() => ((h) / 15.0).clamp(0.0, 1.0),
        _ => 0.0,
    };
    round4(0.75 * pitch + 0.25 * hnr)
}

pub fn articulation(reference: &Articulation, read: &Articulation) -> f64 {
    round4(score::compare(reference, read).total)
}

/// Speaker identity from F1 and F2. Those are the mouth tracks this
/// analyser's vowel tests already trust. F0 is expression and is not in
/// the score. F3 and VTL are reported on the body and left out of the
/// number: on a two-resonator tone they wander, and PR #191's TractPrint v2
/// is the split that keeps them with the speaker baseline.
/// The stored baseline F0 is checked separately and is not steered.
pub fn speaker_kept(reference: &Articulation, read: &Articulation) -> f64 {
    let mut parts = Vec::new();
    push_sim(
        &mut parts,
        med(reference, "f1_hz"),
        med(read, "f1_hz"),
        80.0,
    );
    push_sim(
        &mut parts,
        med(reference, "f2_hz"),
        med(read, "f2_hz"),
        150.0,
    );
    if parts.is_empty() {
        return 0.0;
    }
    let mean = parts.iter().map(|v| v.max(1e-9).ln()).sum::<f64>() / parts.len() as f64;
    round4(mean.exp())
}

fn push_sim(parts: &mut Vec<f64>, a: Option<f64>, b: Option<f64>, scale: f64) {
    if let (Some(a), Some(b)) = (a, b) {
        parts.push(sim(a, b, scale));
    }
}

fn sim(a: f64, b: f64, scale: f64) -> f64 {
    1.0 / (1.0 + (a - b).abs() / scale.max(1e-6))
}

fn med(art: &Articulation, name: &str) -> Option<f64> {
    art.summary
        .get(name)?
        .q
        .get(4)
        .copied()
        .filter(|v| v.is_finite() && *v > 0.0)
}

pub fn intended_f0(intent: &Intent) -> f64 {
    hz(intent.baseline_f0_hz, intent.pitch_st)
}

pub fn body_view(
    intent: &Intent,
    read: &ReadBack,
    lungs_status: &str,
    heard: &[BreathSpan],
) -> BodyView {
    BodyView {
        lungs_status: lungs_status.into(),
        intended_inhales: intent.breaths.clone(),
        readback_inhales: heard.to_vec(),
        throat_status: "decoded".into(),
        mouth_status: "decoded".into(),
        nose_status: "stub".into(),
        nose_todo: "TODO(S5 nose): phon_nasal and A1-P0 are not measured. The decoder draft leaves nose.a1_p0 empty.".into(),
        hnr_db: read.hnr_db,
        f1_hz: read.f1_hz,
        f2_hz: read.f2_hz,
        f3_hz: read.f3_hz,
    }
}

pub fn baseline_view(intent: &Intent, read: &ReadBack) -> BaselineView {
    BaselineView {
        f0_hz: round4(intent.baseline_f0_hz),
        readback_f0_hz: read.f0_hz_median,
        profile_speaker_f0_hz: read.profile_speaker_f0_hz.map(round4),
        steered: false,
    }
}
