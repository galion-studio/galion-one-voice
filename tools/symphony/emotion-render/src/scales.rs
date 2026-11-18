// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Step sizes. Unknown stays unknown.
//!
//! A published sign is not a size. Banse & Scherer (1996) z and the Hughes
//! (2010) hertz means are not copied into this file. The engine block is the
//! Chatterbox worker's existing literals, not an emotion effect.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Clone, Debug, PartialEq)]
pub struct EngineDefaults {
    /// `prepare_conditionals(..., exaggeration=0.0)` in the Nano worker.
    pub exaggeration: f64,
    /// `cfg_weight=0.0` in `nano_stream._iter_turbo_tokens`.
    pub cfg_weight: f64,
    /// `temperature = 0.8` in that same function.
    pub temperature: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FeatureScales {
    /// Semitones at weight 1, before the literature sign.
    pub pitch_st: Option<f64>,
    /// Added to the rate multiplier 1.
    pub rate: Option<f64>,
    /// Added to the energy multiplier 1.
    pub energy: Option<f64>,
    /// Milliseconds of extra pre-pause at weight 1, when the sign is "more".
    pub pause_ms: Option<f64>,
    /// Added to the pitch-range multiplier 1.
    pub pitch_range: Option<f64>,
    /// Added to the engine exaggeration at weight 1. Not a Scherer parameter.
    pub exaggeration: Option<f64>,
    /// Added to the engine cfg_weight. Not a Scherer parameter.
    pub cfg_weight: Option<f64>,
    /// Added to the engine temperature. Not a Scherer parameter.
    pub temperature: Option<f64>,
}

impl FeatureScales {
    pub fn all_unknown() -> Self {
        Self {
            pitch_st: None,
            rate: None,
            energy: None,
            pause_ms: None,
            pitch_range: None,
            exaggeration: None,
            cfg_weight: None,
            temperature: None,
        }
    }

    pub fn get(&self, feature: &str) -> Option<f64> {
        match feature {
            "pitch_st" => self.pitch_st,
            "rate" => self.rate,
            "energy" => self.energy,
            "pause_ms" => self.pause_ms,
            "pitch_range" => self.pitch_range,
            "exaggeration" => self.exaggeration,
            "cfg_weight" => self.cfg_weight,
            "temperature" => self.temperature,
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Scales {
    pub engine: EngineDefaults,
    pub shared: FeatureScales,
    /// Pole id → step sizes that replace the shared scale for that feature.
    /// `hot_anger`, `cold_anger`, `panic`, `anxiety`.
    pub poles: BTreeMap<String, FeatureScales>,
}

impl Scales {
    pub fn unknown_with(engine: EngineDefaults) -> Self {
        Self {
            engine,
            shared: FeatureScales::all_unknown(),
            poles: BTreeMap::new(),
        }
    }

    pub fn scale_for(&self, pole: Option<&str>, feature: &str) -> Option<f64> {
        if let Some(name) = pole {
            if let Some(row) = self.poles.get(name) {
                if let Some(v) = row.get(feature) {
                    return Some(v);
                }
            }
        }
        self.shared.get(feature)
    }
}

pub fn load_scales(path: &Path) -> Result<Scales, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    parse_scales(&text)
}

pub fn parse_scales(text: &str) -> Result<Scales, String> {
    let v: toml::Value = toml::from_str(text).map_err(|e| e.to_string())?;
    let engine = v.get("engine").ok_or("missing [engine]")?;
    let engine = EngineDefaults {
        exaggeration: required_f64(engine, "exaggeration")?,
        cfg_weight: required_f64(engine, "cfg_weight")?,
        temperature: required_f64(engine, "temperature")?,
    };
    let shared = match v.get("scales") {
        Some(s) => parse_feature_scales(s)?,
        None => FeatureScales::all_unknown(),
    };
    let mut poles = BTreeMap::new();
    if let Some(table) = v.get("poles").and_then(|p| p.as_table()) {
        for (name, row) in table {
            poles.insert(name.clone(), parse_feature_scales(row)?);
        }
    }
    Ok(Scales {
        engine,
        shared,
        poles,
    })
}

fn parse_feature_scales(v: &toml::Value) -> Result<FeatureScales, String> {
    Ok(FeatureScales {
        pitch_st: optional_scale(v, "pitch_st")?,
        rate: optional_scale(v, "rate")?,
        energy: optional_scale(v, "energy")?,
        pause_ms: optional_scale(v, "pause_ms")?,
        pitch_range: optional_scale(v, "pitch_range")?,
        exaggeration: optional_scale(v, "exaggeration")?,
        cfg_weight: optional_scale(v, "cfg_weight")?,
        temperature: optional_scale(v, "temperature")?,
    })
}

fn required_f64(v: &toml::Value, key: &str) -> Result<f64, String> {
    match v.get(key) {
        Some(toml::Value::Float(n)) if n.is_finite() => Ok(*n),
        Some(toml::Value::Integer(n)) => Ok(*n as f64),
        _ => Err(format!(
            "engine.{key} must be a finite number (the worker's literal, not an emotion size)"
        )),
    }
}

fn optional_scale(v: &toml::Value, key: &str) -> Result<Option<f64>, String> {
    match v.get(key) {
        None => Ok(None),
        Some(toml::Value::String(s)) if s == "unknown" => Ok(None),
        Some(toml::Value::Float(n)) => nonnegative(*n, key),
        Some(toml::Value::Integer(n)) => nonnegative(*n as f64, key),
        _ => Err(format!(
            "{key} must be a non-negative number or \"unknown\""
        )),
    }
}

fn nonnegative(n: f64, key: &str) -> Result<Option<f64>, String> {
    if n.is_finite() && n >= 0.0 {
        Ok(Some(n))
    } else {
        Err(format!("{key} must be a non-negative finite step size"))
    }
}
