// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Magnitudes from `configs/symphony/lungs.toml`.
//! Cited directions are checked here. The numbers themselves are not.

use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub schema: String,
    pub tick_s: f64,
    pub phrase: PhraseCfg,
    pub inspiration: InspirationCfg,
    pub volume: VolumeCfg,
    pub pressure: PressureCfg,
    pub energy: EnergyCfg,
    pub arousal: ArousalCfg,
    pub fatigue: FatigueCfg,
    pub categories: CategoryCfg,
    pub audible: AudibleCfg,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhraseCfg {
    pub chars_per_s: f64,
    pub max_group_s: f64,
    pub min_group_s: f64,
    pub clause_break_s: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InspirationCfg {
    pub base_s: f64,
    pub per_phrase_s: f64,
    pub min_s: f64,
    pub max_s: f64,
    pub depth_base: f64,
    pub depth_per_phrase_s: f64,
    pub depth_min: f64,
    pub depth_max: f64,
    pub deep_depth: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VolumeCfg {
    pub rest: f64,
    pub floor: f64,
    pub span: f64,
    pub spend_fraction: f64,
    pub exhale_spend: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PressureCfg {
    pub ps_ref: f64,
    pub ps_per_volume: f64,
    pub loudness_db_per_ps: f64,
    pub f0_st_per_ps: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnergyCfg {
    pub at: f64,
    pub ps_per_unit: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArousalCfg {
    pub rate_k: f64,
    pub ps_per_unit: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FatigueCfg {
    pub depth_per_unit: f64,
    pub group_per_unit: f64,
    pub literature: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CategoryCfg {
    pub amusement: AmusementCfg,
    pub disgust: DisgustCfg,
    pub fear: FearCfg,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AmusementCfg {
    pub insp_time_scale: f64,
    pub depth_scale: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DisgustCfg {
    pub hold_s: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FearCfg {
    pub group_scale: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudibleCfg {
    pub neutral_rms: f64,
    pub silence_floor_dbfs: f64,
    pub db_per_depth: f64,
    pub db_per_arousal: f64,
    pub db_per_energy: f64,
    pub db_per_fatigue: f64,
    pub exhale_s: f64,
    pub exhale_db_offset: f64,
    pub seed: u64,
}

impl Config {
    pub fn bundled() -> Result<Self, String> {
        Self::load(&bundled_path())
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let cfg: Config = toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        cfg.check()?;
        Ok(cfg)
    }

    fn check(&self) -> Result<(), String> {
        if self.schema != "galion-lungs-config/1" {
            return Err(format!("unknown lungs config schema {}", self.schema));
        }
        if (self.tick_s - crate::TICK_S).abs() > 1e-9 {
            return Err("tick_s must be 0.08 (80 ms Symphony clock)".into());
        }
        pos("phrase.chars_per_s", self.phrase.chars_per_s)?;
        pos("phrase.max_group_s", self.phrase.max_group_s)?;
        pos("phrase.min_group_s", self.phrase.min_group_s)?;
        pos("phrase.clause_break_s", self.phrase.clause_break_s)?;
        if self.phrase.min_group_s > self.phrase.max_group_s {
            return Err("phrase.min_group_s is above max_group_s".into());
        }
        // Whalen and Kinsella-Shaw: longer upcoming phrase, longer inspiration.
        pos("inspiration.per_phrase_s", self.inspiration.per_phrase_s)?;
        pos("inspiration.base_s", self.inspiration.base_s)?;
        if self.inspiration.min_s <= 0.0 || self.inspiration.min_s > self.inspiration.max_s {
            return Err("inspiration min/max out of order".into());
        }
        // Winkworth and colleagues: longer phrase, deeper inspiration.
        pos(
            "inspiration.depth_per_phrase_s",
            self.inspiration.depth_per_phrase_s,
        )?;
        if self.inspiration.depth_min <= 0.0
            || self.inspiration.depth_min > self.inspiration.depth_max
        {
            return Err("inspiration depth min/max out of order".into());
        }
        if !(0.0 < self.inspiration.deep_depth && self.inspiration.deep_depth <= 1.0) {
            return Err("inspiration.deep_depth must be in (0, 1]".into());
        }
        if self.volume.floor >= self.volume.rest {
            return Err("volume.floor must sit below volume.rest".into());
        }
        pos("volume.span", self.volume.span)?;
        // Hixon, Goldman and Mead: the inhaled air is spent across the phrase.
        if !(0.0 < self.volume.spend_fraction && self.volume.spend_fraction <= 1.0) {
            return Err("volume.spend_fraction must be in (0, 1]".into());
        }
        if !(0.0..=1.0).contains(&self.volume.exhale_spend) {
            return Err("volume.exhale_spend must be in [0, 1]".into());
        }
        // Pressure falls with volume. Loudness and F0 rise with pressure.
        pos("pressure.ps_per_volume", self.pressure.ps_per_volume)?;
        pos(
            "pressure.loudness_db_per_ps",
            self.pressure.loudness_db_per_ps,
        )?;
        pos("pressure.f0_st_per_ps", self.pressure.f0_st_per_ps)?;
        if !self.energy.at.is_finite() || !(0.0..=1.0).contains(&self.energy.at) {
            return Err("energy.at must be in [0, 1]".into());
        }
        pos("energy.ps_per_unit", self.energy.ps_per_unit)?;
        // Homma and Masaoka: positive arousal, faster breathing.
        pos("arousal.rate_k", self.arousal.rate_k)?;
        pos("arousal.ps_per_unit", self.arousal.ps_per_unit)?;
        if !self.fatigue.depth_per_unit.is_finite() || !self.fatigue.group_per_unit.is_finite() {
            return Err("fatigue coefficients must be finite".into());
        }
        if self.fatigue.literature.trim().is_empty() {
            return Err("fatigue.literature must say where the coefficients come from".into());
        }
        // Boiten: amusement decreases inspiratory time and tidal volume.
        unit_interval_open(
            "categories.amusement.insp_time_scale",
            self.categories.amusement.insp_time_scale,
        )?;
        unit_interval_open(
            "categories.amusement.depth_scale",
            self.categories.amusement.depth_scale,
        )?;
        // Boiten: disgust prolongs the inspiratory pause.
        pos("categories.disgust.hold_s", self.categories.disgust.hold_s)?;
        // Homma and Masaoka, mapped onto fear: a shorter breath group, so more breaths.
        unit_interval_open(
            "categories.fear.group_scale",
            self.categories.fear.group_scale,
        )?;
        pos("audible.neutral_rms", self.audible.neutral_rms)?;
        if !self.audible.silence_floor_dbfs.is_finite() {
            return Err("audible.silence_floor_dbfs must be finite".into());
        }
        for (name, v) in [
            ("db_per_depth", self.audible.db_per_depth),
            ("db_per_arousal", self.audible.db_per_arousal),
            ("db_per_energy", self.audible.db_per_energy),
            ("db_per_fatigue", self.audible.db_per_fatigue),
            ("exhale_db_offset", self.audible.exhale_db_offset),
        ] {
            if !v.is_finite() {
                return Err(format!("audible.{name} must be finite"));
            }
        }
        if self.audible.exhale_s < 0.0 {
            return Err("audible.exhale_s must be >= 0".into());
        }
        Ok(())
    }
}

fn pos(name: &str, v: f64) -> Result<(), String> {
    if v.is_finite() && v > 0.0 {
        Ok(())
    } else {
        Err(format!("{name} must be a positive finite magnitude"))
    }
}

/// Cited decrease: the scale is strictly between 0 and 1. The value inside that
/// interval is config.
fn unit_interval_open(name: &str, v: f64) -> Result<(), String> {
    if v.is_finite() && v > 0.0 && v < 1.0 {
        Ok(())
    } else {
        Err(format!(
            "{name} must lie in (0, 1) so the cited direction stays a decrease"
        ))
    }
}

pub fn bundled_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../configs/symphony/lungs.toml")
}

/// Snap a non-negative duration onto the 80 ms grid. Zero stays zero.
pub fn snap(tick_s: f64, seconds: f64) -> f64 {
    if !seconds.is_finite() || seconds <= 0.0 {
        return 0.0;
    }
    let n = (seconds / tick_s).round().max(1.0);
    n * tick_s
}
