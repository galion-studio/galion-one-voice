// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Feeling drive for the glottal source.
//!
//! Names go through `galion-mne-contract` (`resolve_name`). This module keeps
//! the throat's own `Feeling` value, which the LF step reads. `blend` on a
//! decoder tick is still accepted and ignored.

use crate::inventory::{self, Category, Track};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One feeling state. `category` absent, or a baseline name, is the speaker's
/// own neutral: no speech sign is applied.
#[derive(Clone, Debug, PartialEq)]
pub struct Feeling {
    pub t_ms: u32,
    pub category: Option<Category>,
    pub track: Track,
    pub arousal: f64,
    pub valence: f64,
    pub dominance: f64,
    pub intensity: f64,
    /// Only `creaky` is read. Creak is not an Rd region in Fant's table, so
    /// it does not by itself move Rd, open quotient, or jitter.
    pub creaky: bool,
}

impl Feeling {
    pub fn baseline() -> Self {
        Feeling {
            t_ms: 0,
            category: None,
            track: Track::Prosody,
            arousal: 0.0,
            valence: 0.0,
            dominance: 0.0,
            intensity: 1.0,
            creaky: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum FeelingDrive {
    /// One state on every tick.
    Constant(Feeling),
    /// Stepwise. The latest moment with `t_ms` at or before the tick is used.
    /// Ticks before the first moment stay at baseline.
    Moments(Vec<Feeling>),
}

impl FeelingDrive {
    pub fn at(&self, t_ms: u32) -> Feeling {
        match self {
            FeelingDrive::Constant(f) => {
                let mut g = f.clone();
                g.t_ms = t_ms;
                g
            }
            FeelingDrive::Moments(ms) => {
                let mut chosen: Option<&Feeling> = None;
                for m in ms {
                    if m.t_ms <= t_ms {
                        chosen = Some(m);
                    }
                }
                match chosen {
                    Some(m) => {
                        let mut g = m.clone();
                        g.t_ms = t_ms;
                        g
                    }
                    None => {
                        let mut g = Feeling::baseline();
                        g.t_ms = t_ms;
                        g
                    }
                }
            }
        }
    }
}

#[derive(Deserialize)]
struct RawFeeling {
    #[serde(default)]
    t_ms: u32,
    #[serde(default)]
    at_ms: Option<u32>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    track: Option<String>,
    #[serde(default)]
    channel: Option<String>,
    #[serde(default)]
    arousal: Option<f64>,
    #[serde(default)]
    valence: Option<f64>,
    #[serde(default)]
    dominance: Option<f64>,
    #[serde(default)]
    intensity: Option<f64>,
    #[serde(default)]
    phonation: Option<String>,
    /// Decoder field. Not interpreted. Present so a PR 191 tick parses.
    #[serde(default)]
    #[allow(dead_code)]
    blend: Option<Value>,
}

pub fn parse_feeling_drive(value: &Value) -> Result<FeelingDrive, String> {
    let obj = value
        .as_object()
        .ok_or("feeling JSON must be an object or an array of moments")?;
    if let Some(moments) = obj.get("moments").or_else(|| obj.get("trajectory")) {
        let arr = moments
            .as_array()
            .ok_or("feeling moments must be an array")?;
        let mut out = Vec::with_capacity(arr.len());
        for m in arr {
            out.push(parse_feeling(m)?);
        }
        out.sort_by_key(|m| m.t_ms);
        return Ok(FeelingDrive::Moments(out));
    }
    if value.as_array().is_some() {
        return parse_feeling_drive_array(value);
    }
    Ok(FeelingDrive::Constant(parse_feeling(value)?))
}

fn parse_feeling_drive_array(value: &Value) -> Result<FeelingDrive, String> {
    let arr = value.as_array().ok_or("feeling array must be an array")?;
    let mut out = Vec::with_capacity(arr.len());
    for m in arr {
        out.push(parse_feeling(m)?);
    }
    out.sort_by_key(|m| m.t_ms);
    Ok(FeelingDrive::Moments(out))
}

pub fn parse_feeling(value: &Value) -> Result<Feeling, String> {
    if value.as_array().is_some() {
        return Err("a feeling moment must be an object".into());
    }
    let raw: RawFeeling =
        serde_json::from_value(value.clone()).map_err(|e| format!("feeling: {e}"))?;
    let t_ms = raw.at_ms.unwrap_or(raw.t_ms);
    let (category, track_from_name) = match raw.category.as_deref() {
        None => (None, None),
        Some(s) if s.trim().is_empty() => (None, None),
        Some(s) => {
            let r = galion_mne_contract::resolve_name(s, galion_mne_contract::Track::Prosody)?;
            let category = match r.pin {
                None => None,
                Some(name) => inventory::resolve(name.as_str())?.category,
            };
            // Laughter is the one alias that forces the burst track.
            let forced = if matches!(r.alias_from, Some("laughter")) {
                Some(inventory::Track::Burst)
            } else {
                None
            };
            (category, forced)
        }
    };
    let track_name = raw.track.as_deref().or(raw.channel.as_deref());
    let track = if let Some(name) = track_name {
        inventory::parse_track(name)?
    } else {
        track_from_name.unwrap_or(Track::Prosody)
    };
    let track = track_from_name.unwrap_or(track);
    for (name, v) in [
        ("arousal", raw.arousal),
        ("valence", raw.valence),
        ("dominance", raw.dominance),
        ("intensity", raw.intensity),
    ] {
        if let Some(x) = v {
            if !x.is_finite() {
                return Err(format!("feeling {name} must be finite"));
            }
        }
    }
    let intensity = raw.intensity.unwrap_or(1.0).clamp(0.0, 1.0);
    let creaky = match raw.phonation.as_deref() {
        None => false,
        Some(s) => match s.trim().to_ascii_lowercase().as_str() {
            "creaky" | "creak" => true,
            "modal" | "breathy" | "pressed" => {
                return Err(
                    "phonation modal, breathy and pressed are derived from Rd; only creaky can be requested"
                        .into(),
                );
            }
            other => return Err(format!("unknown phonation hint {other}")),
        },
    };
    Ok(Feeling {
        t_ms,
        category,
        track,
        arousal: raw.arousal.unwrap_or(0.0),
        valence: raw.valence.unwrap_or(0.0),
        dominance: raw.dominance.unwrap_or(0.0),
        intensity,
        creaky,
    })
}

/// Speaker baseline. F0 is required and is this speaker's, not a population norm.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Speaker {
    pub f0_hz: f64,
    /// Absent means the config's LF reference Rd, which is not a measurement.
    #[serde(default)]
    pub rd: Option<f64>,
}

/// One drive from an S1 intent document. A frame without a pin is baseline.
pub fn drive_from_document(doc: &galion_mne_contract::Document) -> Result<FeelingDrive, String> {
    if doc.kind != galion_mne_contract::DocKind::Intent {
        return Err("throat input must be an intent document".into());
    }
    let mut moments = Vec::with_capacity(doc.intent_frames.len());
    for frame in &doc.intent_frames {
        let (category, track) = match &frame.feeling.pin {
            None => (None, inventory::Track::Prosody),
            Some(pin) => {
                let category = inventory::resolve(pin.name.as_str())?.category;
                let track = match pin.track {
                    galion_mne_contract::Track::Burst => inventory::Track::Burst,
                    galion_mne_contract::Track::Prosody => inventory::Track::Prosody,
                };
                (category, track)
            }
        };
        moments.push(Feeling {
            t_ms: frame.t_ms,
            category,
            track,
            arousal: frame.feeling.arousal,
            valence: frame.feeling.valence,
            dominance: frame.feeling.dominance,
            intensity: frame.intent.strength,
            creaky: false,
        });
    }
    moments.sort_by_key(|m| m.t_ms);
    if moments.is_empty() {
        return Err("throat input has no frames".into());
    }
    Ok(FeelingDrive::Moments(moments))
}

pub fn parse_speaker(baseline_f0_hz: f64, rd: Option<f64>) -> Result<Speaker, String> {
    if !baseline_f0_hz.is_finite() || baseline_f0_hz <= 0.0 {
        return Err("speaker baseline F0 must be a positive finite hertz value".into());
    }
    if let Some(rd) = rd {
        if !rd.is_finite() || rd <= 0.0 {
            return Err("speaker Rd must be positive and finite".into());
        }
    }
    Ok(Speaker {
        f0_hz: baseline_f0_hz,
        rd,
    })
}
