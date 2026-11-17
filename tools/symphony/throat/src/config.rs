// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Magnitudes. A missing or "unknown" value is not applied.
//! The LF regression coefficients are not config: they are the model.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use toml::Value;

#[derive(Clone, Debug)]
pub struct Config {
    pub tick_ms: u32,
    pub rd_baseline: f64,
    pub rd_min: f64,
    pub rd_max: f64,
    pub pressed_rd_max: f64,
    pub breathy_rd_min: f64,
    pub source_hz: u32,
    pub tilt_harmonic_from: u32,
    pub tilt_harmonic_to: u32,
    /// Fant's Ks. None means the effort covariation is not applied.
    pub ks: Option<f64>,
    pub scales: BTreeMap<String, Option<f64>>,
    pub poles: BTreeMap<String, BTreeMap<String, Option<f64>>>,
}

impl Config {
    pub fn scale(&self, pole: Option<&str>, key: &str) -> Option<f64> {
        if let Some(name) = pole {
            if let Some(table) = self.poles.get(name) {
                if let Some(&Some(v)) = table.get(key) {
                    return Some(v);
                }
            }
        }
        self.scales.get(key).copied().flatten()
    }
}

pub fn load_path(path: &Path) -> Result<Config, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    parse(&text)
}

pub fn parse(text: &str) -> Result<Config, String> {
    let value: Value = toml::from_str(text).map_err(|e| format!("throat config: {e}"))?;
    let root = value.as_table().ok_or("throat config must be a table")?;
    let schema = root
        .get("schema")
        .and_then(Value::as_str)
        .ok_or("throat config schema missing")?;
    if schema != "galion-throat/1" {
        return Err(format!("throat config schema {schema}"));
    }
    let tick_ms = root
        .get("tick_ms")
        .and_then(Value::as_integer)
        .ok_or("tick_ms")? as u32;
    if tick_ms != crate::TICK_MS {
        return Err(format!(
            "tick_ms is {tick_ms}; the clock is {}",
            crate::TICK_MS
        ));
    }
    let model = table(root, "model")?;
    let effort = table(root, "effort")?;
    let scales = scale_table(table(root, "scales")?)?;
    let mut poles = BTreeMap::new();
    if let Some(pole_root) = root.get("poles") {
        let pole_root = pole_root.as_table().ok_or("poles must be a table")?;
        for (name, body) in pole_root {
            let body = body
                .as_table()
                .ok_or(format!("poles.{name} must be a table"))?;
            poles.insert(name.clone(), scale_table(body)?);
        }
    }
    let cfg = Config {
        tick_ms,
        rd_baseline: num(model, "rd_baseline")?,
        rd_min: num(model, "rd_min")?,
        rd_max: num(model, "rd_max")?,
        pressed_rd_max: num(model, "pressed_rd_max")?,
        breathy_rd_min: num(model, "breathy_rd_min")?,
        source_hz: num(model, "source_hz")? as u32,
        tilt_harmonic_from: num(model, "tilt_harmonic_from")? as u32,
        tilt_harmonic_to: num(model, "tilt_harmonic_to")? as u32,
        ks: opt_num(effort, "ks")?,
        scales,
        poles,
    };
    check(&cfg)?;
    Ok(cfg)
}

fn check(cfg: &Config) -> Result<(), String> {
    if cfg.rd_min <= 0.0 || cfg.rd_max <= cfg.rd_min {
        return Err("rd_min and rd_max are out of order".into());
    }
    if cfg.rd_baseline < cfg.rd_min || cfg.rd_baseline > cfg.rd_max {
        return Err("rd_baseline is outside rd_min..=rd_max".into());
    }
    if !(cfg.pressed_rd_max < cfg.breathy_rd_min) {
        return Err("pressed_rd_max must sit below breathy_rd_min".into());
    }
    if cfg.source_hz < 8000 || (cfg.source_hz as u64 * cfg.tick_ms as u64) % 1000 != 0 {
        return Err("source_hz must be at least 8000 and divide the 80 ms tick".into());
    }
    if cfg.tilt_harmonic_to <= cfg.tilt_harmonic_from || cfg.tilt_harmonic_from < 1 {
        return Err("tilt harmonics are out of order".into());
    }
    if let Some(ks) = cfg.ks {
        if !(0.0..=1.0).contains(&ks) {
            return Err("ks must be in 0..=1 or unknown".into());
        }
    }
    for (name, v) in &cfg.scales {
        check_scale(name, *v)?;
    }
    for (pole, table) in &cfg.poles {
        for (name, v) in table {
            check_scale(&format!("{pole}.{name}"), *v)?;
        }
    }
    Ok(())
}

fn check_scale(name: &str, v: Option<f64>) -> Result<(), String> {
    if let Some(x) = v {
        if !x.is_finite() {
            return Err(format!("{name} must be finite"));
        }
        if name.ends_with("jitter") || name.ends_with("shimmer") {
            if !(0.0..0.5).contains(&x) {
                return Err(format!("{name} must be in 0..0.5 or unknown"));
            }
        }
    }
    Ok(())
}

fn table<'a>(
    root: &'a toml::map::Map<String, Value>,
    key: &str,
) -> Result<&'a toml::map::Map<String, Value>, String> {
    root.get(key)
        .and_then(Value::as_table)
        .ok_or_else(|| format!("missing [{key}]"))
}

fn num(table: &toml::map::Map<String, Value>, key: &str) -> Result<f64, String> {
    match table.get(key) {
        Some(Value::Float(f)) => Ok(*f),
        Some(Value::Integer(i)) => Ok(*i as f64),
        Some(_) => Err(format!("{key} must be a number")),
        None => Err(format!("missing {key}")),
    }
}

fn opt_num(table: &toml::map::Map<String, Value>, key: &str) -> Result<Option<f64>, String> {
    match table.get(key) {
        None => Ok(None),
        Some(v) => unknown_or_num(v, key),
    }
}

fn scale_table(
    table: &toml::map::Map<String, Value>,
) -> Result<BTreeMap<String, Option<f64>>, String> {
    let mut out = BTreeMap::new();
    for (k, v) in table {
        out.insert(k.clone(), unknown_or_num(v, k)?);
    }
    Ok(out)
}

fn unknown_or_num(v: &Value, key: &str) -> Result<Option<f64>, String> {
    match v {
        Value::String(s) if s == "unknown" => Ok(None),
        Value::Float(f) => Ok(Some(*f)),
        Value::Integer(i) => Ok(Some(*i as f64)),
        _ => Err(format!("{key} must be a number or \"unknown\"")),
    }
}
