// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! The features agent's breath stage, as this layer consumes it.
//!
//! `BreathCue`, `Lungs`, `apply_cues` and `splice_inhales` match
//! `tools/symphony/articulation/src/breath.rs` on `cursor/articulation-v2-cad4`
//! (features agent bc-56b9f22c, draft PR 191). `measure` is not copied: it
//! needs that crate's `Articulation` card and praatfan. Pass its JSON in.
//!
//! `splice_inhales` is the same fallback the features agent uses when the
//! cloner has no breath token: the first detected inhale, taken from this
//! speaker's samples, placed at phrase starts. It does not change loudness
//! for emotion. [`crate::synth`] does that on the planned timeline.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct BreathCue {
    pub start_s: f64,
    pub end_s: f64,
    pub kind: String,
    pub loudness_dbfs: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Lungs {
    pub breath_rate_per_min: f64,
    pub breath_group_s: f64,
    pub inhale_loudness_dbfs: Option<f64>,
    pub breathiness_cpps_db: Option<f64>,
    pub h1_h2_db: Option<f64>,
    pub n_inhales: usize,
    pub voiced_runs_per_s: f64,
    pub pause_ratio: f64,
    pub source: String,
    pub cues: Vec<BreathCue>,
}

/// Replace cues the way `breath::apply_cues` does.
pub fn apply_cues(lungs: &mut Lungs, cues: Vec<BreathCue>, source: &str, duration_s: f64) {
    lungs.n_inhales = cues.len();
    lungs.breath_rate_per_min = if duration_s > 0.0 {
        cues.len() as f64 / duration_s * 60.0
    } else {
        0.0
    };
    lungs.inhale_loudness_dbfs = if cues.is_empty() {
        None
    } else {
        Some(cues.iter().map(|c| c.loudness_dbfs).sum::<f64>() / cues.len() as f64)
    };
    lungs.cues = cues;
    lungs.source = source.into();
}

/// Accept a voice-profile (`{"lungs": ...}`), a `Lungs` object, or the
/// breath worker's `{"source", "cues"}` object.
pub fn parse_stage(value: &Value) -> Result<Lungs, String> {
    let obj = value
        .as_object()
        .ok_or("breath stage JSON must be an object")?;
    if let Some(lungs) = obj.get("lungs") {
        return parse_lungs(lungs);
    }
    if obj.contains_key("cues") && !obj.contains_key("breath_rate_per_min") {
        return lungs_from_worker(value);
    }
    parse_lungs(value)
}

fn parse_lungs(value: &Value) -> Result<Lungs, String> {
    serde_json::from_value(value.clone()).map_err(|e| format!("lungs object: {e}"))
}

fn lungs_from_worker(value: &Value) -> Result<Lungs, String> {
    let cues_v = value.get("cues").cloned().unwrap_or(Value::Array(vec![]));
    let cues: Vec<BreathCue> =
        serde_json::from_value(cues_v).map_err(|e| format!("breath cues: {e}"))?;
    let source = value
        .get("source")
        .and_then(|s| s.as_str())
        .unwrap_or("external-breath");
    let duration = value
        .get("duration_s")
        .and_then(|s| s.as_f64())
        .unwrap_or(0.0);
    let mut lungs = Lungs {
        breath_rate_per_min: 0.0,
        breath_group_s: 0.0,
        inhale_loudness_dbfs: None,
        breathiness_cpps_db: None,
        h1_h2_db: None,
        n_inhales: 0,
        voiced_runs_per_s: 0.0,
        pause_ratio: 0.0,
        source: String::new(),
        cues: Vec::new(),
    };
    apply_cues(&mut lungs, cues, source, duration);
    Ok(lungs)
}

/// First cue the features agent would splice, preferring one labelled inhale.
pub fn primary_cue(cues: &[BreathCue]) -> Option<&BreathCue> {
    cues.iter()
        .find(|c| c.kind == "inhale")
        .or_else(|| cues.first())
}

pub fn cue_samples(samples: &[f64], sr: f64, cue: &BreathCue) -> Option<Vec<f64>> {
    if sr <= 0.0 || samples.is_empty() {
        return None;
    }
    let a = (cue.start_s * sr).round().max(0.0) as usize;
    let b = (cue.end_s * sr).round() as usize;
    if b <= a || b > samples.len() {
        return None;
    }
    Some(samples[a..b].to_vec())
}

/// Splice the reference's own inhales in front of phrase starts that do not
/// already begin with one. This is the features agent's fallback when the
/// cloner has no breath token. Chatterbox in this repo has no `[breath]` tag.
///
/// Behaviour matches `breath::splice_inhales` in the features agent's crate:
/// the first cue only, no gain change, same sample rate as `samples`.
pub fn splice_inhales(
    samples: &[f64],
    sr: f64,
    cues: &[BreathCue],
    phrase_starts_s: &[f64],
) -> Vec<f64> {
    if cues.is_empty() || phrase_starts_s.is_empty() || sr <= 0.0 {
        return samples.to_vec();
    }
    let cue = &cues[0];
    let a = (cue.start_s * sr).round().max(0.0) as usize;
    let b = (cue.end_s * sr).round() as usize;
    if b <= a || b > samples.len() {
        return samples.to_vec();
    }
    let inhale = samples[a..b].to_vec();
    let mut inserts: Vec<usize> = phrase_starts_s
        .iter()
        .map(|t| (*t * sr).round().max(0.0) as usize)
        .filter(|i| *i > inhale.len() && *i < samples.len())
        .collect();
    inserts.sort_unstable();
    inserts.dedup();
    let mut out = Vec::with_capacity(samples.len() + inserts.len() * inhale.len());
    let mut cursor = 0;
    for i in inserts {
        if i < cursor {
            continue;
        }
        let already = i >= inhale.len()
            && samples[i - inhale.len()..i]
                .iter()
                .zip(&inhale)
                .all(|(x, y)| (x - y).abs() < 1e-6);
        out.extend_from_slice(&samples[cursor..i]);
        if !already {
            out.extend_from_slice(&inhale);
        }
        cursor = i;
    }
    out.extend_from_slice(&samples[cursor..]);
    out
}
