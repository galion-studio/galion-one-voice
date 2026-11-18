// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Thresholds from `configs/symphony/voice-guard.toml`.

use serde::Deserialize;
use std::path::Path;

pub const BUNDLED: &str = include_str!("../../../../configs/symphony/voice-guard.toml");

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub schema: String,
    pub tick_ms: u32,
    pub format: FormatCfg,
    pub dc: DcCfg,
    pub clip: ClipCfg,
    pub click: ClickCfg,
    pub dropout: DropoutCfg,
    pub r#loop: LoopCfg,
    pub flatness: FlatnessCfg,
    pub buzz: BuzzCfg,
    pub cutoff: CutoffCfg,
    pub speaker: SpeakerCfg,
    pub neural: NeuralCfg,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormatCfg {
    pub allowed_sample_rate_hz: Vec<u32>,
    pub channels: u16,
    pub allowed_bits: Vec<u16>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DcCfg {
    pub max_dbfs: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClipCfg {
    pub linear: f64,
    pub max_true_peak_dbtp: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClickCfg {
    pub jump: f64,
    pub ar_order: usize,
    pub ar_residual: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DropoutCfg {
    pub min_gap_s: f64,
    pub silence_dbfs: f64,
    pub window_ms: u32,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoopCfg {
    pub ncc: f64,
    pub min_run_frames: u32,
    pub pitch_ncc: f64,
    pub pitch_lag_tolerance_samples: usize,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlatnessCfg {
    pub max_median: f64,
    pub speech_floor_dbfs: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuzzCfg {
    pub min_hnr_db: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CutoffCfg {
    pub jump: f64,
    pub min_tail_ms: u32,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpeakerCfg {
    pub min_speaker: f64,
    pub min_embedding_cos: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NeuralCfg {
    pub enabled: bool,
    pub command: String,
}

impl Config {
    pub fn bundled() -> Result<Self, String> {
        parse(BUNDLED)
    }
}

pub fn load(path: &Path) -> Result<Config, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    parse(&text).map_err(|e| format!("{}: {e}", path.display()))
}

pub fn parse(text: &str) -> Result<Config, String> {
    let cfg: Config = toml::from_str(text).map_err(|e| e.to_string())?;
    if cfg.schema != "galion-voice-guard-config/1" {
        return Err(format!(
            "schema {} is not galion-voice-guard-config/1",
            cfg.schema
        ));
    }
    if cfg.tick_ms == 0 || cfg.format.channels == 0 {
        return Err("tick_ms and format.channels must be non-zero".into());
    }
    if cfg.format.allowed_sample_rate_hz.is_empty() {
        return Err("format.allowed_sample_rate_hz is empty".into());
    }
    if !(0.0..1.0).contains(&cfg.click.jump) || !(0.0..2.0).contains(&cfg.clip.linear) {
        return Err("click.jump and clip.linear must be positive and small".into());
    }
    Ok(cfg)
}
