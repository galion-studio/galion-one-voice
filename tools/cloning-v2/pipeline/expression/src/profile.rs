//! Per-reference expression profile: prosody, voice colour, singing,
//! and whatever emotion2vec+ / CLAP numbers the caller already measured.

use crate::prosody::{analyze_prosody, ProsodyStats};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const SCHEMA: &str = "galion-expression-profile/1";

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ScoreOverlay {
    #[serde(default)]
    pub e2v: BTreeMap<String, f64>,
    #[serde(default)]
    pub clap_love: Option<f64>,
    #[serde(default)]
    pub clap_seductive: Option<f64>,
    #[serde(default)]
    pub love_mean_cos: Option<f64>,
    #[serde(default)]
    pub neutral_mean_cos: Option<f64>,
    #[serde(default)]
    pub sed_mean_cos: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChannelInput {
    pub emotion: String,
    pub samples_16k: Vec<f32>,
    #[serde(default)]
    pub scores: ScoreOverlay,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Timbre {
    pub f0_median_hz: f64,
    pub f0_sd_st: f64,
    pub rms_db: f64,
    pub spectral_tilt_db_oct: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChannelProfile {
    pub emotion: String,
    pub prosody: ProsodyStats,
    pub timbre: Timbre,
    pub e2v: BTreeMap<String, f64>,
    pub clap_love: Option<f64>,
    pub clap_seductive: Option<f64>,
    pub love_mean_cos: Option<f64>,
    pub neutral_mean_cos: Option<f64>,
    pub sed_mean_cos: Option<f64>,
    pub singing: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmotionalRange {
    pub n_channels: usize,
    pub emotions: Vec<String>,
    pub f0_min_hz: f64,
    pub f0_max_hz: f64,
    pub f0_span_st: f64,
    pub rms_span_db: f64,
    pub tilt_span_db_oct: f64,
    pub sung_phrases: usize,
    pub e2v_tops: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExpressionProfile {
    pub schema: String,
    pub voice: String,
    pub channels: Vec<ChannelProfile>,
    pub range: EmotionalRange,
    /// This profile describes sound. It does not choose a reference clip.
    pub reference_selection: bool,
}

pub fn analyze_channels(voice: &str, inputs: &[ChannelInput]) -> ExpressionProfile {
    let channels: Vec<ChannelProfile> = inputs
        .iter()
        .map(|inp| {
            let prosody = analyze_prosody(&inp.samples_16k);
            let singing = !prosody.sung_phrases.is_empty();
            ChannelProfile {
                emotion: inp.emotion.clone(),
                timbre: Timbre {
                    f0_median_hz: prosody.f0_median_hz,
                    f0_sd_st: prosody.f0_sd_st,
                    rms_db: prosody.rms_db,
                    spectral_tilt_db_oct: prosody.spectral_tilt_db_oct,
                },
                singing,
                e2v: inp.scores.e2v.clone(),
                clap_love: inp.scores.clap_love,
                clap_seductive: inp.scores.clap_seductive,
                love_mean_cos: inp.scores.love_mean_cos,
                neutral_mean_cos: inp.scores.neutral_mean_cos,
                sed_mean_cos: inp.scores.sed_mean_cos,
                prosody,
            }
        })
        .collect();
    let range = range_of(&channels);
    ExpressionProfile {
        schema: SCHEMA.into(),
        voice: voice.into(),
        channels,
        range,
        reference_selection: false,
    }
}

fn range_of(channels: &[ChannelProfile]) -> EmotionalRange {
    let mut emotions = Vec::new();
    let mut f0s = Vec::new();
    let mut rms = Vec::new();
    let mut tilts = Vec::new();
    let mut sung = 0usize;
    let mut tops = Vec::new();
    for ch in channels {
        emotions.push(ch.emotion.clone());
        if ch.prosody.f0_median_hz > 0.0 {
            f0s.push(ch.prosody.f0_median_hz);
        }
        rms.push(ch.prosody.rms_db);
        tilts.push(ch.prosody.spectral_tilt_db_oct);
        sung += ch.prosody.sung_phrases.len();
        if let Some((lab, _)) = ch
            .e2v
            .iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
        {
            if !tops.contains(lab) {
                tops.push(lab.clone());
            }
        }
    }
    let (f0_min, f0_max) = if f0s.is_empty() {
        (0.0, 0.0)
    } else {
        (
            f0s.iter().cloned().fold(f64::INFINITY, f64::min),
            f0s.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        )
    };
    let span_st = if f0_min > 0.0 && f0_max > 0.0 {
        12.0 * (f0_max / f0_min).log2()
    } else {
        0.0
    };
    let span = |v: &[f64]| {
        if v.is_empty() {
            0.0
        } else {
            v.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
                - v.iter().cloned().fold(f64::INFINITY, f64::min)
        }
    };
    EmotionalRange {
        n_channels: channels.len(),
        emotions,
        f0_min_hz: if f0s.is_empty() { 0.0 } else { f0_min },
        f0_max_hz: if f0s.is_empty() { 0.0 } else { f0_max },
        f0_span_st: span_st,
        rms_span_db: span(&rms),
        tilt_span_db_oct: span(&tilts),
        sung_phrases: sung,
        e2v_tops: tops,
    }
}

/// Pull e2v / CLAP fields off a `score_persona` or `score_universal` file record.
pub fn overlay_from_score_record(rec: &serde_json::Value) -> ScoreOverlay {
    let mut e2v = BTreeMap::new();
    if let Some(probs) = rec.pointer("/e2v/probs").and_then(|v| v.as_object()) {
        for (k, v) in probs {
            if let Some(x) = v.as_f64() {
                e2v.insert(k.clone(), x);
            }
        }
    }
    let clap = rec.get("clap").cloned().unwrap_or(serde_json::Value::Null);
    let clap_prob = |name: &str| clap.pointer(&format!("/probs/{name}")).and_then(|v| v.as_f64());
    ScoreOverlay {
        e2v,
        clap_love: clap_prob("love").or_else(|| rec.get("clap_love").and_then(|v| v.as_f64())),
        clap_seductive: clap_prob("seductive")
            .or_else(|| rec.get("clap_seductive").and_then(|v| v.as_f64())),
        love_mean_cos: rec.get("love_mean_cos").and_then(|v| v.as_f64()),
        neutral_mean_cos: rec.get("neu_mean_cos").and_then(|v| v.as_f64()).or_else(|| {
            rec.get("neutral_mean_cos").and_then(|v| v.as_f64())
        }),
        sed_mean_cos: rec.get("sed_mean_cos").and_then(|v| v.as_f64()),
    }
}
