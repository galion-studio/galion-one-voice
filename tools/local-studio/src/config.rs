// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Studio config. Gates and the delivery spec can be stricter than the
//! rollout numbers. They cannot be loosened from this file.

use crate::device::DeviceRequest;
use crate::{
    BED_HZ, BED_UNDER_DB, ROLLOUT_ECAPA_MIN, ROLLOUT_EMOTION_MIN, ROLLOUT_WER_MAX, TARGET_LUFS,
    TRUE_PEAK_DBTP,
};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct StudioConfig {
    pub consent: Consent,
    #[serde(default)]
    pub render: RenderConfig,
    #[serde(default)]
    pub gates: Gates,
    #[serde(default)]
    pub whisper: WhisperConfig,
    #[serde(default)]
    pub ecapa: ToolConfig,
    #[serde(default)]
    pub emotion: EmotionConfig,
    #[serde(default)]
    pub bed: BedConfig,
    #[serde(default)]
    pub master: MasterConfig,
    #[serde(default)]
    pub timing: TimingConfig,
    #[serde(default)]
    pub output: OutputConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Consent {
    pub owner_reference: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Backend {
    Cbx,
    Command,
    Synthetic,
}

impl Default for Backend {
    fn default() -> Self {
        Self::Cbx
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct RenderConfig {
    #[serde(default)]
    pub backend: Backend,
    #[serde(default = "default_takes")]
    pub takes: usize,
    #[serde(default)]
    pub device: DeviceRequest,
    #[serde(default = "default_rate")]
    pub sample_rate: u32,
    #[serde(default = "default_seed")]
    pub seed: u64,
    #[serde(default = "default_temperature")]
    pub temperature: f64,
    #[serde(default)]
    pub command: Vec<String>,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            backend: Backend::Cbx,
            takes: default_takes(),
            device: DeviceRequest::Auto,
            sample_rate: default_rate(),
            seed: default_seed(),
            temperature: default_temperature(),
            command: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Gates {
    #[serde(default = "default_wer")]
    pub wer_max: f64,
    #[serde(default = "default_ecapa")]
    pub ecapa_min: f64,
    #[serde(default = "default_emotion")]
    pub emotion_min: f64,
}

impl Default for Gates {
    fn default() -> Self {
        Self {
            wer_max: default_wer(),
            ecapa_min: default_ecapa(),
            emotion_min: default_emotion(),
        }
    }
}

impl Gates {
    pub fn rollout() -> Self {
        Self::default()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct WhisperConfig {
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default)]
    pub command: Vec<String>,
}

impl Default for WhisperConfig {
    fn default() -> Self {
        Self {
            model: default_model(),
            command: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ToolConfig {
    #[serde(default)]
    pub command: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct EmotionConfig {
    #[serde(default)]
    pub command: Vec<String>,
    #[serde(default)]
    pub label_map: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BedConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_hz")]
    pub tuning_hz: f64,
    #[serde(default = "default_under")]
    pub under_voice_db: f64,
}

impl Default for BedConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            tuning_hz: default_hz(),
            under_voice_db: default_under(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoudnessPref {
    Auto,
    InProcess,
    Ffmpeg,
}

impl Default for LoudnessPref {
    fn default() -> Self {
        Self::Auto
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct MasterConfig {
    #[serde(default = "default_lufs")]
    pub target_lufs: f64,
    #[serde(default = "default_tp")]
    pub true_peak_dbtp: f64,
    #[serde(default)]
    pub engine: LoudnessPref,
}

impl Default for MasterConfig {
    fn default() -> Self {
        Self {
            target_lufs: default_lufs(),
            true_peak_dbtp: default_tp(),
            engine: LoudnessPref::Auto,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct TimingConfig {
    #[serde(default = "default_lead")]
    pub lead_in_ms: u32,
    #[serde(default = "default_gap")]
    pub gap_ms: u32,
    #[serde(default = "default_tail")]
    pub tail_ms: u32,
}

impl Default for TimingConfig {
    fn default() -> Self {
        Self {
            lead_in_ms: default_lead(),
            gap_ms: default_gap(),
            tail_ms: default_tail(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct OutputConfig {
    #[serde(default = "default_formats")]
    pub formats: Vec<String>,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            formats: default_formats(),
        }
    }
}

fn default_takes() -> usize {
    3
}
fn default_rate() -> u32 {
    24_000
}
fn default_seed() -> u64 {
    1
}
fn default_temperature() -> f64 {
    0.7
}
fn default_wer() -> f64 {
    ROLLOUT_WER_MAX
}
fn default_ecapa() -> f64 {
    ROLLOUT_ECAPA_MIN
}
fn default_emotion() -> f64 {
    ROLLOUT_EMOTION_MIN
}
fn default_model() -> String {
    "small".into()
}
fn default_true() -> bool {
    true
}
fn default_hz() -> f64 {
    BED_HZ
}
fn default_under() -> f64 {
    BED_UNDER_DB
}
fn default_lufs() -> f64 {
    TARGET_LUFS
}
fn default_tp() -> f64 {
    TRUE_PEAK_DBTP
}
fn default_lead() -> u32 {
    120
}
fn default_gap() -> u32 {
    280
}
fn default_tail() -> u32 {
    400
}
fn default_formats() -> Vec<String> {
    vec!["wav".into(), "opus".into(), "mp3".into()]
}

impl StudioConfig {
    pub fn parse(text: &str) -> Result<Self, String> {
        let cfg: Self = toml::from_str(text).map_err(|e| format!("config: {e}"))?;
        cfg.validate()?;
        Ok(cfg)
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::parse(&text)
    }

    pub fn validate(&self) -> Result<(), String> {
        if !self.consent.owner_reference {
            return Err(
                "consent.owner_reference must be true. The reference clip is the owner's own voice, supplied at runtime"
                    .into(),
            );
        }
        if !(1..=8).contains(&self.render.takes) {
            return Err("render.takes must be from 1 to 8".into());
        }
        if !(16_000..=96_000).contains(&self.render.sample_rate) {
            return Err("render.sample_rate must be from 16000 to 96000".into());
        }
        if !self.render.temperature.is_finite() || !(0.0..=2.0).contains(&self.render.temperature) {
            return Err("render.temperature must be from 0 to 2".into());
        }
        if self.render.backend == Backend::Command && self.render.command.is_empty() {
            return Err("render.backend = \"command\" needs render.command".into());
        }
        finite("gates.wer_max", self.gates.wer_max)?;
        finite("gates.ecapa_min", self.gates.ecapa_min)?;
        finite("gates.emotion_min", self.gates.emotion_min)?;
        if self.gates.wer_max > ROLLOUT_WER_MAX + 1e-9 {
            return Err(format!(
                "gates.wer_max must stay at or below {ROLLOUT_WER_MAX}"
            ));
        }
        if self.gates.ecapa_min < ROLLOUT_ECAPA_MIN - 1e-9 {
            return Err(format!(
                "gates.ecapa_min must stay at or above {ROLLOUT_ECAPA_MIN}"
            ));
        }
        if self.gates.emotion_min < ROLLOUT_EMOTION_MIN - 1e-9 {
            return Err(format!(
                "gates.emotion_min must stay at or above {ROLLOUT_EMOTION_MIN}"
            ));
        }
        if (self.bed.tuning_hz - BED_HZ).abs() > 1e-6 {
            return Err("bed.tuning_hz is 432. The bed is generated at that tuning".into());
        }
        if (self.bed.under_voice_db - BED_UNDER_DB).abs() > 1e-6 {
            return Err("bed.under_voice_db is 18".into());
        }
        if (self.master.target_lufs - TARGET_LUFS).abs() > 1e-6 {
            return Err("master.target_lufs is -16".into());
        }
        if (self.master.true_peak_dbtp - TRUE_PEAK_DBTP).abs() > 1e-6 {
            return Err("master.true_peak_dbtp is -1".into());
        }
        for (name, ms) in [
            ("lead_in_ms", self.timing.lead_in_ms),
            ("gap_ms", self.timing.gap_ms),
            ("tail_ms", self.timing.tail_ms),
        ] {
            if ms > 10_000 {
                return Err(format!("timing.{name} is above 10000"));
            }
        }
        if self.output.formats.is_empty() {
            return Err("output.formats needs at least wav".into());
        }
        let mut saw_wav = false;
        for fmt in &self.output.formats {
            match fmt.as_str() {
                "wav" => saw_wav = true,
                "opus" | "mp3" => {}
                other => return Err(format!("output format {other} is not wav, opus, or mp3")),
            }
        }
        if !saw_wav {
            return Err(
                "output.formats must include wav. opus and mp3 are encoded from that wav".into(),
            );
        }
        if self.whisper.model.trim().is_empty() {
            return Err("whisper.model is empty".into());
        }
        Ok(())
    }
}

fn finite(name: &str, value: f64) -> Result<(), String> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(format!("{name} must be a finite number"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_config_is_valid() {
        let cfg = StudioConfig::parse(include_str!("../examples/studio.toml")).unwrap();
        assert_eq!(cfg.render.takes, 3);
        assert!((cfg.gates.ecapa_min - 0.7057).abs() < 1e-12);
        assert!((cfg.bed.tuning_hz - 432.0).abs() < 1e-12);
        assert_eq!(cfg.output.formats, vec!["wav", "opus", "mp3"]);
    }

    #[test]
    fn loosened_gates_are_rejected() {
        let mut text = include_str!("../examples/studio.toml").to_string();
        text = text.replace("wer_max = 0.018", "wer_max = 0.05");
        let err = StudioConfig::parse(&text).unwrap_err();
        assert!(err.contains("wer_max"));

        let mut text = include_str!("../examples/studio.toml").to_string();
        text = text.replace("ecapa_min = 0.7057", "ecapa_min = 0.5");
        let err = StudioConfig::parse(&text).unwrap_err();
        assert!(err.contains("ecapa_min"));

        let mut text = include_str!("../examples/studio.toml").to_string();
        text = text.replace("tuning_hz = 432.0", "tuning_hz = 440.0");
        let err = StudioConfig::parse(&text).unwrap_err();
        assert!(err.contains("432"));
    }

    #[test]
    fn owner_consent_is_required() {
        let text = include_str!("../examples/studio.toml")
            .replace("owner_reference = true", "owner_reference = false");
        let err = StudioConfig::parse(&text).unwrap_err();
        assert!(err.contains("owner_reference"));
    }
}
