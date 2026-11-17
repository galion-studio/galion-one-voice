// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Feeling input. Accepts the decoder `TickState` JSON (blend and burst
//! weights) and the emotion-render object (`emotion` or `category` when
//! `blend` is absent). Mixture weights are the caller's, not a published
//! mixture coefficient. Valence does not move a mouth control. Arousal is
//! recorded and is not a per-unit gain.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::MouthError;
use crate::inventory::{resolve_name, Category, Resolved, Track};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Part {
    pub category: String,
    pub weight: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias_from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pole: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FeelingTick {
    pub t_ms: u32,
    /// `baseline`, `prosody`, or `burst`.
    pub track: String,
    pub parts: Vec<Part>,
    pub arousal: f64,
    pub valence: f64,
    pub dominance: f64,
    pub intensity: f64,
}

impl FeelingTick {
    pub fn from_document(doc: &galion_mne_contract::Document) -> Vec<Self> {
        doc.intent_frames
            .iter()
            .map(|frame| match &frame.feeling.pin {
                None => Self {
                    t_ms: frame.t_ms,
                    track: "baseline".into(),
                    parts: Vec::new(),
                    arousal: frame.feeling.arousal,
                    valence: frame.feeling.valence,
                    dominance: frame.feeling.dominance,
                    intensity: frame.intent.strength,
                },
                Some(pin) => Self {
                    t_ms: frame.t_ms,
                    track: match pin.track {
                        galion_mne_contract::Track::Burst => "burst".into(),
                        galion_mne_contract::Track::Prosody => "prosody".into(),
                    },
                    parts: vec![Part {
                        category: pin.name.as_str().into(),
                        weight: pin.weight,
                        alias_from: None,
                        pole: None,
                    }],
                    arousal: frame.feeling.arousal,
                    valence: frame.feeling.valence,
                    dominance: frame.feeling.dominance,
                    intensity: frame.intent.strength,
                },
            })
            .collect()
    }

    pub fn baseline(t_ms: u32) -> Self {
        Self {
            t_ms,
            track: "baseline".into(),
            parts: Vec::new(),
            arousal: 0.0,
            valence: 0.0,
            dominance: 0.0,
            intensity: 0.0,
        }
    }

    pub fn is_prosody(&self) -> bool {
        self.track == Track::Prosody.as_str()
    }

    pub fn is_burst(&self) -> bool {
        self.track == Track::Burst.as_str()
    }
}

/// One object or an array of objects, in time order.
pub fn parse_feelings(v: &Value) -> Result<Vec<FeelingTick>, MouthError> {
    let items: Vec<&Value> = if let Some(arr) = v.as_array() {
        if arr.is_empty() {
            return Ok(Vec::new());
        }
        arr.iter().collect()
    } else if v.is_object() {
        vec![v]
    } else if v.is_null() {
        return Ok(Vec::new());
    } else {
        return Err(MouthError::Json(
            "feeling must be an object or an array".into(),
        ));
    };
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        out.push(parse_one(item)?);
    }
    out.sort_by_key(|t| t.t_ms);
    Ok(out)
}

pub fn feeling_at(feelings: &[FeelingTick], time_ms: u32) -> FeelingTick {
    feelings
        .iter()
        .rev()
        .find(|f| f.t_ms <= time_ms)
        .cloned()
        .unwrap_or_else(|| FeelingTick::baseline(time_ms))
}

fn parse_one(v: &Value) -> Result<FeelingTick, MouthError> {
    let t_ms = v.get("t_ms").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
    let arousal = num_or(v, "arousal", 0.0)?;
    let valence = num_or(v, "valence", 0.0)?;
    let dominance = num_or(v, "dominance", 0.0)?;
    check_axis(arousal, valence, dominance)?;
    let intensity = num_or(v, "intensity", 0.0)?;
    check_unit("intensity", intensity)?;

    let explicit = match v.get("track").and_then(|x| x.as_str()) {
        None => None,
        Some(s) => Some(
            Track::parse(s)
                .ok_or_else(|| MouthError::BadBlend("track must be prosody or burst".into()))?,
        ),
    };

    let blend = parse_weight_array(v.get("blend"))?;
    let burst = parse_weight_array(v.get("burst"))?;
    let named = named_part(v, intensity)?;

    let (track, parts) = if let Some(track) = explicit {
        let parts = match track {
            Track::Prosody => {
                if !blend.is_empty() {
                    blend
                } else {
                    named
                }
            }
            Track::Burst => {
                if !burst.is_empty() {
                    burst
                } else if !blend.is_empty() {
                    blend
                } else {
                    named
                }
            }
        };
        (track.as_str().to_string(), parts)
    } else if !blend.is_empty() && !burst.is_empty() {
        return Err(MouthError::BadBlend(
            "prosody blend and burst are both set; name one track".into(),
        ));
    } else if !burst.is_empty() {
        (Track::Burst.as_str().to_string(), burst)
    } else if !blend.is_empty() {
        (Track::Prosody.as_str().to_string(), blend)
    } else if !named.is_empty() {
        let track = named[0]
            .alias_from
            .as_deref()
            .filter(|a| *a == "laughter")
            .map(|_| Track::Burst.as_str())
            .unwrap_or(Track::Prosody.as_str());
        (track.to_string(), named)
    } else {
        return Ok(FeelingTick {
            t_ms,
            track: "baseline".into(),
            parts: Vec::new(),
            arousal,
            valence,
            dominance,
            intensity,
        });
    };

    let mut tick = FeelingTick {
        t_ms,
        track,
        parts,
        arousal,
        valence,
        dominance,
        intensity,
    };
    stamp_poles(&mut tick);
    Ok(tick)
}

fn named_part(v: &Value, intensity: f64) -> Result<Vec<Part>, MouthError> {
    if v.get("blend").and_then(|b| b.as_array()).is_some()
        || v.get("burst").and_then(|b| b.as_array()).is_some()
    {
        return Ok(Vec::new());
    }
    let emotion = v
        .get("emotion")
        .and_then(|x| x.as_str())
        .or_else(|| v.get("category").and_then(|x| x.as_str()));
    let Some(name) = emotion else {
        return Ok(Vec::new());
    };
    if let (Some(a), Some(b)) = (
        v.get("emotion").and_then(|x| x.as_str()),
        v.get("category").and_then(|x| x.as_str()),
    ) {
        if a != b {
            return Err(MouthError::BadBlend("emotion and category disagree".into()));
        }
    }
    match resolve_name(name)? {
        Resolved::Baseline => Ok(Vec::new()),
        Resolved::One {
            category,
            alias_from,
            ..
        } => {
            if intensity == 0.0 {
                return Ok(Vec::new());
            }
            Ok(vec![Part {
                category: category.as_str().into(),
                weight: intensity,
                alias_from,
                pole: None,
            }])
        }
    }
}

fn parse_weight_array(v: Option<&Value>) -> Result<Vec<Part>, MouthError> {
    let Some(arr) = v.and_then(|x| x.as_array()) else {
        return Ok(Vec::new());
    };
    let mut parts = Vec::new();
    for row in arr {
        let id = row
            .get("id")
            .and_then(|x| x.as_str())
            .ok_or_else(|| MouthError::BadBlend("each blend entry needs an id".into()))?;
        let weight = row
            .get("w")
            .and_then(|x| x.as_f64())
            .ok_or_else(|| MouthError::BadBlend(format!("{id} needs a weight")))?;
        if !weight.is_finite() || weight < 0.0 {
            return Err(MouthError::BadNumber("blend weight"));
        }
        if weight == 0.0 {
            continue;
        }
        match resolve_name(id)? {
            Resolved::Baseline => {}
            Resolved::One {
                category,
                alias_from,
                ..
            } => parts.push(Part {
                category: category.as_str().into(),
                weight,
                alias_from,
                pole: None,
            }),
        }
    }
    Ok(parts)
}

fn stamp_poles(tick: &mut FeelingTick) {
    for part in &mut tick.parts {
        if let Some(cat) = Category::parse(&part.category) {
            part.pole = crate::direction::pole(cat, tick.dominance).map(str::to_string);
        }
    }
}

fn num_or(v: &Value, key: &'static str, default: f64) -> Result<f64, MouthError> {
    match v.get(key) {
        None => Ok(default),
        Some(x) => x
            .as_f64()
            .filter(|n| n.is_finite())
            .ok_or(MouthError::BadNumber(key)),
    }
}

fn check_unit(what: &'static str, x: f64) -> Result<(), MouthError> {
    if x.is_finite() && (0.0..=1.0).contains(&x) {
        Ok(())
    } else {
        Err(MouthError::BadNumber(what))
    }
}

fn check_axis(arousal: f64, valence: f64, dominance: f64) -> Result<(), MouthError> {
    for (name, x) in [
        ("arousal", arousal),
        ("valence", valence),
        ("dominance", dominance),
    ] {
        if !x.is_finite() || !(-1.0..=1.0).contains(&x) {
            return Err(MouthError::BadNumber(name));
        }
    }
    Ok(())
}
