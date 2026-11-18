// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! The lungs tick this layer consumes.
//!
//! Field names match `Tick` in tools/symphony/lungs on
//! `cursor/lungs-breath-layer-b72f` (draft PR 195): `t_ms`,
//! `subglottal_pressure_rel`, `f0_st`, `loudness_db`, `segment`.
//! The breath planner is not copied. A missing `f0_st` is not replaced
//! with a second pressure-to-semitone slope.

use serde::Deserialize;
use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
pub struct LungsTick {
    pub t_ms: u32,
    pub subglottal_pressure_rel: Option<f64>,
    pub f0_st: Option<f64>,
    pub loudness_db: Option<f64>,
    pub segment: String,
}

#[derive(Deserialize)]
struct RawTick {
    t_ms: u32,
    #[serde(default)]
    subglottal_pressure_rel: Option<f64>,
    #[serde(default)]
    f0_st: Option<f64>,
    #[serde(default)]
    loudness_db: Option<f64>,
    #[serde(default)]
    segment: String,
}

pub fn parse_lungs(value: &Value) -> Result<Vec<LungsTick>, String> {
    let ticks = if let Some(arr) = value.as_array() {
        arr.clone()
    } else {
        let obj = value
            .as_object()
            .ok_or("lungs JSON must be an object or an array of ticks")?;
        if let Some(ticks) = obj.get("ticks") {
            ticks
                .as_array()
                .cloned()
                .ok_or("lungs ticks must be an array")?
        } else if let Some(lungs) = obj.get("lungs") {
            return parse_lungs(lungs);
        } else {
            return Err("lungs JSON has no ticks".into());
        }
    };
    if ticks.is_empty() {
        return Err("lungs ticks are empty".into());
    }
    let mut out = Vec::with_capacity(ticks.len());
    for t in ticks {
        let raw: RawTick = serde_json::from_value(t).map_err(|e| format!("lungs tick: {e}"))?;
        for (name, v) in [
            ("subglottal_pressure_rel", raw.subglottal_pressure_rel),
            ("f0_st", raw.f0_st),
            ("loudness_db", raw.loudness_db),
        ] {
            if let Some(x) = v {
                if !x.is_finite() {
                    return Err(format!("lungs tick {name} must be finite"));
                }
            }
        }
        out.push(LungsTick {
            t_ms: raw.t_ms,
            subglottal_pressure_rel: raw.subglottal_pressure_rel,
            f0_st: raw.f0_st,
            loudness_db: raw.loudness_db,
            segment: raw.segment,
        });
    }
    for w in out.windows(2) {
        if w[1].t_ms != w[0].t_ms.saturating_add(crate::TICK_MS) {
            return Err(format!(
                "lungs ticks must step by {} ms (saw {} then {})",
                crate::TICK_MS,
                w[0].t_ms,
                w[1].t_ms
            ));
        }
    }
    Ok(out)
}

pub fn is_phonation(tick: &LungsTick) -> bool {
    tick.segment == "speech" && tick.subglottal_pressure_rel.is_some()
}
