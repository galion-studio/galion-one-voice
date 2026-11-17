// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Local stand-in for the intent contract.
//!
//! TODO(S1): replace this module with `galion-mne-contract` when agent S1
//! publishes that crate. Do not grow a second inventory here.
//!
//! The fields follow the decoder tick from draft PR 191 (`t_ms`, `arousal`,
//! `valence`, `dominance`, `intensity`) and the render target from draft
//! PR 192 (`category`, `track`). `blend` is accepted and ignored.

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
            let r = inventory::resolve(s)?;
            (r.category, r.track_override)
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
