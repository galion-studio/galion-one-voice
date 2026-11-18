// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! The open feeling state this layer reads.
//!
//! One moment has arousal, valence, dominance and intensity, a prosody
//! category (or the neutral baseline), and burst-track weights on the same
//! 24 names. That is the shape of `TickState` in draft PR 191
//! (`t_ms`, the four axes, `blend`, `burst`). This file parses that JSON.
//! It does not score a category and it does not move a control by itself.
//!
//! Valence and dominance are kept on the tick. No paper read for this layer
//! gives them a nasal or a burst coefficient, so the planner does not apply them.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::inventory::Inventory;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Weight {
    pub id: String,
    pub w: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct OpenFeeling {
    pub arousal: f64,
    pub valence: f64,
    pub dominance: f64,
    pub intensity: f64,
    /// Prosody top, or `neutral` for the speaker baseline. Not a gesture.
    pub category: String,
    /// Wordless track. The same 24, no neutral. Empty means every weight is 0.
    #[serde(default)]
    pub burst: Vec<Weight>,
}

impl OpenFeeling {
    pub fn neutral() -> Self {
        OpenFeeling {
            arousal: 0.0,
            valence: 0.0,
            dominance: 0.0,
            intensity: 0.0,
            category: "neutral".into(),
            burst: Vec::new(),
        }
    }

    /// Axes use the decoder's scale: arousal, valence and dominance in −1..1,
    /// intensity in 0..1. The clamp is that scale, not a new paper.
    pub fn sanitized(mut self) -> Result<Self, String> {
        for (name, v) in [
            ("arousal", self.arousal),
            ("valence", self.valence),
            ("dominance", self.dominance),
            ("intensity", self.intensity),
        ] {
            if !v.is_finite() {
                return Err(format!("feeling.{name} must be finite"));
            }
        }
        self.arousal = self.arousal.clamp(-1.0, 1.0);
        self.valence = self.valence.clamp(-1.0, 1.0);
        self.dominance = self.dominance.clamp(-1.0, 1.0);
        self.intensity = self.intensity.clamp(0.0, 1.0);
        for w in &mut self.burst {
            if !w.w.is_finite() {
                return Err(format!("burst weight {} is not finite", w.id));
            }
            w.w = w.w.clamp(0.0, 1.0);
        }
        Ok(self)
    }

    pub fn check(&self, inventory: &Inventory) -> Result<(), String> {
        inventory.check_prosody(&self.category)?;
        for w in &self.burst {
            inventory.check_burst_category(&w.id)?;
        }
        Ok(())
    }
}

/// Accept either this layer's feeling object (`category`) or a decoder tick
/// (`blend` plus `burst`). If both name a top and they differ, that is an error.
pub fn from_value(v: &Value) -> Result<OpenFeeling, String> {
    let obj = v.as_object().ok_or("feeling JSON must be an object")?;
    let explicit = obj
        .get("category")
        .and_then(|c| c.as_str())
        .map(|s| s.to_string());
    let category = match obj.get("blend") {
        Some(b) => category_from_blend(&weights(b)?, explicit)?,
        None => explicit.unwrap_or_else(|| "neutral".into()),
    };
    let burst = match obj.get("burst") {
        Some(b) => weights(b)?,
        None => Vec::new(),
    };
    let feeling = OpenFeeling {
        arousal: num(obj.get("arousal"), 0.0, "arousal")?,
        valence: num(obj.get("valence"), 0.0, "valence")?,
        dominance: num(obj.get("dominance"), 0.0, "dominance")?,
        intensity: num(obj.get("intensity"), 0.0, "intensity")?,
        category,
        burst,
    };
    feeling.sanitized()
}

fn num(v: Option<&Value>, default: f64, name: &str) -> Result<f64, String> {
    match v {
        None => Ok(default),
        Some(Value::Null) => Ok(default),
        Some(v) => v
            .as_f64()
            .filter(|n| n.is_finite())
            .ok_or_else(|| format!("feeling.{name} must be a finite number")),
    }
}

fn weights(v: &Value) -> Result<Vec<Weight>, String> {
    let arr = v.as_array().ok_or("burst weights must be an array")?;
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        let id = item
            .get("id")
            .and_then(|s| s.as_str())
            .ok_or("a burst weight needs an id")?
            .to_string();
        let w = item
            .get("w")
            .and_then(|n| n.as_f64())
            .filter(|n| n.is_finite())
            .ok_or_else(|| format!("burst weight {id} needs a finite w"))?;
        out.push(Weight { id, w });
    }
    Ok(out)
}

/// Highest non-neutral blend weight. Neutral holds the remainder in the
/// decoder and is not the prosody name while another weight is positive.
/// A tie is kept only when `category` names one of the tied ids.
fn category_from_blend(list: &[Weight], explicit: Option<String>) -> Result<String, String> {
    let mut best = f64::NEG_INFINITY;
    let mut ids: Vec<String> = Vec::new();
    for w in list {
        if w.id == "neutral" || w.w <= 0.0 {
            continue;
        }
        if w.w > best + 1e-12 {
            best = w.w;
            ids.clear();
            ids.push(w.id.clone());
        } else if (w.w - best).abs() <= 1e-12 {
            ids.push(w.id.clone());
        }
    }
    match (ids.len(), explicit) {
        (0, Some(name)) => Ok(name),
        (0, None) => Ok("neutral".into()),
        (1, Some(name)) if name != ids[0] => Err(format!(
            "feeling blend tops {} but category is {name}",
            ids[0]
        )),
        (1, Some(name)) => Ok(name),
        (1, None) => Ok(ids.into_iter().next().unwrap_or_else(|| "neutral".into())),
        (_, Some(name)) if ids.iter().any(|id| id == &name) => Ok(name),
        _ => Err("prosody blend ties between categories; name one".into()),
    }
}

/// Strict top of the burst track. A tie returns None so the planner does not
/// pick a gesture by alphabetical accident.
pub fn exclusive_burst_top(weights: &[Weight]) -> Option<(&str, f64)> {
    let mut best: Option<(&str, f64)> = None;
    let mut tied = false;
    for w in weights {
        if w.w <= 0.0 {
            continue;
        }
        match best {
            None => best = Some((w.id.as_str(), w.w)),
            Some((_, bw)) if w.w > bw + 1e-12 => {
                best = Some((w.id.as_str(), w.w));
                tied = false;
            }
            Some((_, bw)) if (w.w - bw).abs() <= 1e-12 => tied = true,
            _ => {}
        }
    }
    if tied {
        None
    } else {
        best
    }
}
