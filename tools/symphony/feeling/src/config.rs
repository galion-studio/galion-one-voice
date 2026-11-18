// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Knobs. A pin axis is a cited pole (±1). A missing pin axis is unplaced.
//! `unknown` is not a number and is not applied.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::CORE_AXES;

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub tick_ms: u32,
    pub inertia_half_life_s: f64,
    pub mood_half_life_s: f64,
    pub linger: f64,
    pub default_blend: f64,
    pub max_axes: usize,
    pub energy_rest: f64,
    pub fatigue_rest: f64,
    pub factor_half_life_s: f64,
    pub energy_into: Coupling,
    pub fatigue_into: Coupling,
    pub distance_into: Coupling,
    pub pins: BTreeMap<String, PinPole>,
}

/// A coupling into one axis. `None` means the literature did not give a size,
/// and the term is not applied.
#[derive(Clone, Debug, PartialEq)]
pub struct Coupling {
    pub axis: String,
    pub per_unit: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PinPole {
    pub cite: String,
    pub valence: Option<i8>,
    pub arousal: Option<i8>,
    pub dominance: Option<i8>,
    pub approach: Option<i8>,
}

impl PinPole {
    pub fn sign(&self, axis: &str) -> Option<i8> {
        match axis {
            "valence" => self.valence,
            "arousal" => self.arousal,
            "dominance" => self.dominance,
            "approach" => self.approach,
            _ => None,
        }
    }

    pub fn placed(&self) -> bool {
        CORE_AXES.iter().any(|axis| self.sign(axis).is_some())
    }
}

impl Config {
    pub fn bundled_path() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../configs/symphony/feeling.toml")
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let value: toml::Value = toml::from_str(text).map_err(|e| e.to_string())?;
        let tick_ms = value
            .get("tick_ms")
            .and_then(|v| v.as_integer())
            .ok_or("tick_ms must be an integer")? as u32;
        if tick_ms != crate::TICK_MS {
            return Err(format!(
                "tick_ms is {tick_ms}; the symphony clock is {}",
                crate::TICK_MS
            ));
        }
        let dynamics = value.get("dynamics").ok_or("missing [dynamics]")?;
        let factors = value.get("factors").ok_or("missing [factors]")?;
        let mut cfg = Config {
            tick_ms,
            inertia_half_life_s: pos(dynamics, "inertia_half_life_s")?,
            mood_half_life_s: pos(dynamics, "mood_half_life_s")?,
            linger: number(dynamics, "linger")?,
            default_blend: number(dynamics, "default_blend")?,
            max_axes: {
                let n = dynamics
                    .get("max_axes")
                    .and_then(|v| v.as_integer())
                    .ok_or("dynamics.max_axes")?;
                if n < 0 {
                    return Err("dynamics.max_axes must be >= 0".into());
                }
                n as usize
            },
            energy_rest: number(factors, "energy_rest")?,
            fatigue_rest: number(factors, "fatigue_rest")?,
            factor_half_life_s: pos(factors, "half_life_s")?,
            energy_into: coupling(&value, &["inner", "energy"], "per_unit")?,
            fatigue_into: coupling(&value, &["inner", "fatigue"], "per_unit")?,
            distance_into: coupling(&value, &["outer", "listener_distance"], "per_metre")?,
            pins: BTreeMap::new(),
        };
        if !(0.0..1.0).contains(&cfg.linger) {
            return Err("dynamics.linger must be in [0, 1)".into());
        }
        if !(0.0..=1.0).contains(&cfg.default_blend) {
            return Err("dynamics.default_blend must be in [0, 1]".into());
        }
        if cfg.max_axes < CORE_AXES.len() {
            return Err("dynamics.max_axes is smaller than the core".into());
        }
        for (name, x) in [
            ("energy_rest", cfg.energy_rest),
            ("fatigue_rest", cfg.fatigue_rest),
        ] {
            if !(0.0..=1.0).contains(&x) {
                return Err(format!("{name} must be in [0, 1]"));
            }
        }
        let pins = value
            .get("pins")
            .and_then(|v| v.as_table())
            .ok_or("missing [pins]")?;
        for (name, body) in pins {
            cfg.pins.insert(name.clone(), pin_pole(name, body)?);
        }
        Ok(cfg)
    }
}

fn pin_pole(name: &str, body: &toml::Value) -> Result<PinPole, String> {
    let literature = body
        .get("literature")
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("pins.{name}.literature"))?;
    if literature != "direction_only" {
        return Err(format!(
            "pins.{name}.literature must be \"direction_only\" (got {literature})"
        ));
    }
    let cite = body
        .get("cite")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("pins.{name}.cite"))?
        .to_string();
    let pole = PinPole {
        cite,
        valence: pole_axis(name, body, "valence")?,
        arousal: pole_axis(name, body, "arousal")?,
        dominance: pole_axis(name, body, "dominance")?,
        approach: pole_axis(name, body, "approach")?,
    };
    if !pole.placed() {
        return Err(format!("pins.{name} places no axis"));
    }
    Ok(pole)
}

fn pole_axis(pin: &str, body: &toml::Value, axis: &str) -> Result<Option<i8>, String> {
    match body.get(axis) {
        None => Ok(None),
        Some(v) => {
            let n = v.as_integer().ok_or_else(|| {
                format!("pins.{pin}.{axis} must be +1 or -1, not a fractional magnitude")
            })?;
            if n == 1 || n == -1 {
                Ok(Some(n as i8))
            } else {
                Err(format!("pins.{pin}.{axis} must be +1 or -1"))
            }
        }
    }
}

fn coupling(root: &toml::Value, path: &[&str], coef: &str) -> Result<Coupling, String> {
    let mut cur = root;
    for key in path {
        cur = cur
            .get(*key)
            .ok_or_else(|| format!("missing [{}]", path.join(".")))?;
    }
    let axis = cur
        .get("axis")
        .and_then(|v| v.as_str())
        .filter(|s| CORE_AXES.contains(s))
        .ok_or_else(|| format!("{}.axis must be one of the core axes", path.join(".")))?
        .to_string();
    let per_unit = match cur.get(coef) {
        None => None,
        Some(v) => unknown_or_number(v, &format!("{}.{coef}", path.join(".")))?,
    };
    Ok(Coupling { axis, per_unit })
}

fn unknown_or_number(v: &toml::Value, what: &str) -> Result<Option<f64>, String> {
    match v {
        toml::Value::String(s) if s == "unknown" => Ok(None),
        toml::Value::Float(x) if x.is_finite() => Ok(Some(*x)),
        toml::Value::Integer(i) => Ok(Some(*i as f64)),
        _ => Err(format!("{what} must be a finite number or \"unknown\"")),
    }
}

fn number(table: &toml::Value, key: &str) -> Result<f64, String> {
    match table.get(key) {
        Some(toml::Value::Float(x)) if x.is_finite() => Ok(*x),
        Some(toml::Value::Integer(i)) => Ok(*i as f64),
        _ => Err(format!("missing or non-finite {key}")),
    }
}

fn pos(table: &toml::Value, key: &str) -> Result<f64, String> {
    let x = number(table, key)?;
    if x > 0.0 {
        Ok(x)
    } else {
        Err(format!("{key} must be > 0"))
    }
}
