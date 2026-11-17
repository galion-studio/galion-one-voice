// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Local stand-in for `galion-mne-contract` (agent S1).
//!
//! TODO(S1): delete this module's name table when `galion-mne-contract` is
//! linked. The names match `configs/emotions-v2.toml` (PR #189) and
//! `state::resolve` on the decoder draft (PR #191). This file does not own
//! the inventory.

use serde::{Deserialize, Serialize};

use crate::fixture::{BreathSpan, Fixture};
use crate::round4;

/// Cowen, Elfenbein, Laukka and Keltner (2019), American Psychologist, Figure 3B.
pub const COWEN: [&str; 24] = [
    "adoration",
    "amusement",
    "anger",
    "awe",
    "confusion",
    "contempt",
    "contentment",
    "desire",
    "disappointment",
    "disgust",
    "distress",
    "ecstasy",
    "elation",
    "embarrassment",
    "fear",
    "interest",
    "pain",
    "realization",
    "relief",
    "sadness",
    "surprise_negative",
    "surprise_positive",
    "sympathy",
    "triumph",
];

/// Cowen, Laukka, Elfenbein, Liu and Keltner (2019), Nature Human Behaviour.
/// A confidence tier for prosody, not a shorter inventory.
pub const PROSODY_RELIABLE: [&str; 12] = [
    "adoration",
    "amusement",
    "anger",
    "awe",
    "confusion",
    "contempt",
    "desire",
    "disappointment",
    "distress",
    "fear",
    "interest",
    "sadness",
];

pub const CONTRACT_TODO: &str = "TODO(S1 galion-mne-contract): this intent was built by the local stub in tools/symphony/listen-back/src/contract.rs. Replace it with the contract crate.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Resolved {
    pub track: &'static str,
    pub category: &'static str,
}

pub fn resolve(name: &str) -> Result<Resolved, String> {
    let prosody = |category| Resolved {
        track: "prosody",
        category,
    };
    let burst = |category| Resolved {
        track: "burst",
        category,
    };
    match name {
        "neutral" | "calm" => Ok(Resolved {
            track: "baseline",
            category: "neutral",
        }),
        "breath" => Err("breath is physiology, not an emotion".into()),
        "sigh" => Err(
            "sigh is a detector across relief, sadness, disappointment, and contentment; name one"
                .into(),
        ),
        "laughter" => Ok(burst("amusement")),
        "seductive" => Ok(prosody("desire")),
        "tender" => Ok(prosody("adoration")),
        "angry" => Ok(prosody("anger")),
        "fearful" => Ok(prosody("fear")),
        "sad" => Ok(prosody("sadness")),
        "disappointed" => Ok(prosody("disappointment")),
        "amused" => Ok(prosody("amusement")),
        "disgusted" => Ok(prosody("contempt")),
        "happy" => Err("happy splits into amusement and interest; name one".into()),
        "excited" => {
            Err("excited splits into interest and amusement; elation is its own category".into())
        }
        "surprised" => Err("surprised splits into awe and confusion".into()),
        other => COWEN
            .iter()
            .find(|id| **id == other)
            .copied()
            .map(prosody)
            .ok_or_else(|| format!("unknown emotion {other}")),
    }
}

pub fn prosody_tier(resolved: Resolved) -> &'static str {
    if resolved.track == "baseline" {
        "baseline"
    } else if PROSODY_RELIABLE.contains(&resolved.category) {
        "reliable"
    } else {
        "extended"
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct FeelingTick {
    pub t_ms: u32,
    pub pitch_st: f64,
    pub arousal: f64,
    pub valence: f64,
    pub dominance: f64,
    pub intensity: f64,
    pub category: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Intent {
    pub schema: String,
    pub source: String,
    pub todo: String,
    pub feeling_source: String,
    pub id: String,
    pub text: String,
    pub track: String,
    pub category: String,
    pub prosody_tier: String,
    pub baseline_f0_hz: f64,
    pub f1_hz: f64,
    pub f2_hz: f64,
    pub f3_hz: f64,
    /// Expression offset from the baseline. The fixture states it.
    /// It is not a paper magnitude and not an emotion-render step size.
    pub pitch_st: f64,
    pub speech_s: f64,
    pub breaths: Vec<BreathSpan>,
    pub feeling: Vec<FeelingTick>,
}

pub fn from_fixture(fixture: &Fixture) -> Result<Intent, String> {
    let resolved = resolve(fixture.name)?;
    if fixture.baseline_f0_hz <= 0.0 || !fixture.baseline_f0_hz.is_finite() {
        return Err(format!("{}: baseline f0 must be positive", fixture.id));
    }
    if fixture.speech_s < 0.08 {
        return Err(format!("{}: speech is shorter than one tick", fixture.id));
    }
    check_prefix_breaths(&fixture.breaths)?;
    let feeling = feeling_of(fixture.speech_s, fixture.pitch_st, resolved);
    Ok(Intent {
        schema: "galion-mne-contract/0-stub".into(),
        source: "stub-s1".into(),
        todo: CONTRACT_TODO.into(),
        feeling_source: "stub-s2".into(),
        id: fixture.id.into(),
        text: fixture.text.into(),
        track: resolved.track.into(),
        category: resolved.category.into(),
        prosody_tier: prosody_tier(resolved).into(),
        baseline_f0_hz: fixture.baseline_f0_hz,
        f1_hz: fixture.f1_hz,
        f2_hz: fixture.f2_hz,
        f3_hz: fixture.f3_hz,
        pitch_st: round4(fixture.pitch_st),
        speech_s: fixture.speech_s,
        breaths: fixture.breaths.clone(),
        feeling,
    })
}

fn feeling_of(speech_s: f64, pitch_st: f64, resolved: Resolved) -> Vec<FeelingTick> {
    let n = (speech_s / crate::decode::TICK_S).round().max(1.0) as usize;
    let intensity = if resolved.track == "baseline" {
        0.0
    } else {
        1.0
    };
    (0..n)
        .map(|i| FeelingTick {
            t_ms: (i as u32) * 80,
            pitch_st: round4(pitch_st),
            arousal: 0.0,
            valence: 0.0,
            dominance: 0.0,
            intensity,
            category: resolved.category.into(),
        })
        .collect()
}

fn check_prefix_breaths(breaths: &[BreathSpan]) -> Result<(), String> {
    let mut t = 0.0;
    for b in breaths {
        if !b.start_s.is_finite() || !b.dur_s.is_finite() || b.dur_s <= 0.0 {
            return Err("a breath span needs a positive duration".into());
        }
        if (b.start_s - t).abs() > 1e-6 {
            return Err(
                "synthetic sound-out places inhalations as a prefix from 0; a later inhale waits on the lungs renderer"
                    .into(),
            );
        }
        t += b.dur_s;
    }
    Ok(())
}

/// Replace the feeling trajectory when agent S2's binary answered.
pub fn apply_feeling(intent: &mut Intent, body: &serde_json::Value) -> Result<(), String> {
    let ticks = body
        .get("ticks")
        .and_then(|v| v.as_array())
        .ok_or("feeling json needs ticks")?;
    if ticks.is_empty() {
        return Err("feeling json ticks are empty".into());
    }
    let mut feeling = Vec::new();
    for (i, row) in ticks.iter().enumerate() {
        let pitch = row
            .get("pitch_st")
            .and_then(|v| v.as_f64())
            .unwrap_or(intent.pitch_st);
        feeling.push(FeelingTick {
            t_ms: row
                .get("t_ms")
                .and_then(|v| v.as_u64())
                .unwrap_or((i as u64) * 80) as u32,
            pitch_st: round4(pitch),
            arousal: round4(row.get("arousal").and_then(|v| v.as_f64()).unwrap_or(0.0)),
            valence: round4(row.get("valence").and_then(|v| v.as_f64()).unwrap_or(0.0)),
            dominance: round4(row.get("dominance").and_then(|v| v.as_f64()).unwrap_or(0.0)),
            intensity: round4(row.get("intensity").and_then(|v| v.as_f64()).unwrap_or(1.0)),
            category: row
                .get("category")
                .and_then(|v| v.as_str())
                .unwrap_or(&intent.category)
                .to_string(),
        });
    }
    if let Some(pitch) = body.get("pitch_st").and_then(|v| v.as_f64()) {
        intent.pitch_st = round4(pitch);
    } else if let Some(first) = feeling.first() {
        intent.pitch_st = first.pitch_st;
    }
    intent.feeling = feeling;
    intent.feeling_source = "galion-feeling".into();
    Ok(())
}
