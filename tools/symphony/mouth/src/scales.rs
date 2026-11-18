// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Step sizes. Unknown stays unknown.
//!
//! A published sign is not a size. Nothing in this file is a hertz value
//! taken from Tartter, Juslin, Banse, Garnier, or Lindblom.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::error::MouthError;

pub const SCALE_KEYS: [&str; 12] = [
    "f1_hz",
    "f2_hz",
    "f3_hz",
    "smile_formant_hz",
    "smile_spread",
    "address_f1_hz",
    "address_jaw",
    "address_lip_opening",
    "jaw_emotion",
    "rate",
    "precision",
    "coarticulation",
];

const UNIT_INTERVAL: [&str; 2] = ["precision", "coarticulation"];

#[derive(Clone, Debug, PartialEq)]
pub struct Scales {
    /// Grid used when a phone has no duration. The Symphony clock, not a
    /// measured phone length.
    pub phone_ms: u32,
    shared: BTreeMap<String, f64>,
    poles: BTreeMap<String, BTreeMap<String, f64>>,
}

impl Scales {
    pub fn unknown(phone_ms: u32) -> Self {
        Self {
            phone_ms,
            shared: BTreeMap::new(),
            poles: BTreeMap::new(),
        }
    }

    pub fn get(&self, pole: Option<&str>, key: &str) -> Option<f64> {
        if let Some(name) = pole {
            if let Some(row) = self.poles.get(name) {
                if let Some(v) = row.get(key) {
                    return Some(*v);
                }
            }
        }
        self.shared.get(key).copied()
    }
}

pub fn load_scales(path: &Path) -> Result<Scales, MouthError> {
    let text =
        fs::read_to_string(path).map_err(|e| MouthError::Io(format!("{}: {e}", path.display())))?;
    parse_scales(&text)
}

pub fn parse_scales(text: &str) -> Result<Scales, MouthError> {
    let v: toml::Value = toml::from_str(text).map_err(|e| MouthError::BadScale(e.to_string()))?;
    let phone_ms = v
        .get("timing")
        .and_then(|t| t.get("phone_ms"))
        .ok_or_else(|| MouthError::BadScale("timing.phone_ms is required".into()))?;
    let phone_ms = finite_u32(phone_ms, "timing.phone_ms")?;
    if phone_ms < 1 {
        return Err(MouthError::BadScale(
            "timing.phone_ms must be at least 1".into(),
        ));
    }
    let shared = match v.get("scales") {
        Some(s) => parse_row(s, "scales")?,
        None => BTreeMap::new(),
    };
    let mut poles = BTreeMap::new();
    if let Some(table) = v.get("poles").and_then(|p| p.as_table()) {
        for (name, row) in table {
            poles.insert(name.clone(), parse_row(row, name)?);
        }
    }
    Ok(Scales {
        phone_ms,
        shared,
        poles,
    })
}

fn parse_row(v: &toml::Value, where_: &str) -> Result<BTreeMap<String, f64>, MouthError> {
    let table = v
        .as_table()
        .ok_or_else(|| MouthError::BadScale(format!("{where_} must be a table")))?;
    let mut out = BTreeMap::new();
    for (key, value) in table {
        if !SCALE_KEYS.contains(&key.as_str()) {
            return Err(MouthError::BadScale(format!(
                "{where_}.{key} is not a mouth scale"
            )));
        }
        match value {
            toml::Value::String(s) if s == "unknown" => {}
            toml::Value::Float(n) => {
                out.insert(key.clone(), check_scale(key, *n)?);
            }
            toml::Value::Integer(n) => {
                out.insert(key.clone(), check_scale(key, *n as f64)?);
            }
            _ => {
                return Err(MouthError::BadScale(format!(
                    "{where_}.{key} must be a number or \"unknown\""
                )));
            }
        }
    }
    Ok(out)
}

fn check_scale(key: &str, n: f64) -> Result<f64, MouthError> {
    if !n.is_finite() || n < 0.0 {
        return Err(MouthError::BadScale(format!(
            "{key} must be a non-negative finite step"
        )));
    }
    if UNIT_INTERVAL.contains(&key) && n > 1.0 {
        return Err(MouthError::BadScale(format!(
            "{key} must be between 0 and 1"
        )));
    }
    Ok(n)
}

fn finite_u32(v: &toml::Value, key: &str) -> Result<u32, MouthError> {
    let n = match v {
        toml::Value::Integer(n) => *n as f64,
        toml::Value::Float(n) => *n,
        _ => {
            return Err(MouthError::BadScale(format!(
                "{key} must be a positive number"
            )))
        }
    };
    if n.is_finite() && n >= 1.0 && n <= u32::MAX as f64 {
        Ok(n.round() as u32)
    } else {
        Err(MouthError::BadScale(format!(
            "{key} must be a positive number"
        )))
    }
}
