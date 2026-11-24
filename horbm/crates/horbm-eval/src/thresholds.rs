// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

use serde::Deserialize;
use std::path::Path;

const EMBEDDED: &str = include_str!("../thresholds.toml");

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Thresholds {
    pub schema: String,
    pub persona: PersonaThresholds,
    pub grounding: GroundingThresholds,
    pub emotion: EmotionThresholds,
    pub latency: LatencyThresholds,
    /// Real-mode word error rate. The fake gate does not read it.
    pub asr: AsrThresholds,
    /// Real-mode wall clock. The fake gate keeps [`Self::latency`].
    pub real: RealThresholds,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersonaThresholds {
    pub id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroundingThresholds {
    pub unknown_reply: String,
    pub card_prefix: String,
    pub research_prefix: String,
    pub honest_source: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmotionThresholds {
    pub frame_ms: u32,
    pub frame_budget_us: u32,
    pub sample_rate_hz: u32,
    pub fatigue_arousal_drop_min: f32,
    pub valence_rise_min: f32,
    pub frames: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LatencyThresholds {
    pub clock: String,
    pub frame_ms: u32,
    pub eager_to_voice_p50_max_ms: u32,
    pub eager_to_voice_p95_max_ms: u32,
    pub barge_in_max_ms: u32,
    pub backchannel_cancels: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AsrThresholds {
    pub wer_max: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RealThresholds {
    pub latency: LatencyThresholds,
}

impl Thresholds {
    pub fn embedded() -> Self {
        Self::parse(EMBEDDED).expect("embedded thresholds.toml")
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let parsed: Self = toml::from_str(text).map_err(|err| err.to_string())?;
        parsed.check()?;
        Ok(parsed)
    }

    pub fn load_path(path: &Path) -> Result<Self, String> {
        let text =
            std::fs::read_to_string(path).map_err(|err| format!("{}: {err}", path.display()))?;
        Self::parse(&text)
    }

    fn check(&self) -> Result<(), String> {
        if self.schema != "horbm-eval/1" {
            return Err(format!("schema {} is not horbm-eval/1", self.schema));
        }
        if self.latency.clock != "virtual" {
            return Err("latency.clock must be virtual".into());
        }
        if self.real.latency.clock != "wall" {
            return Err("real.latency.clock must be wall".into());
        }
        if self.emotion.frame_ms != self.latency.frame_ms
            || self.emotion.frame_ms != self.real.latency.frame_ms
        {
            return Err("emotion and latency frame_ms differ".into());
        }
        if !self.asr.wer_max.is_finite() || !(0.0..=1.0).contains(&self.asr.wer_max) {
            return Err("asr.wer_max must be between 0 and 1".into());
        }
        if self.emotion.frame_ms == 0 || self.emotion.frames == 0 {
            return Err("frame count is empty".into());
        }
        if self.emotion.sample_rate_hz == 0
            || (u64::from(self.emotion.sample_rate_hz) * u64::from(self.emotion.frame_ms)) % 1000
                != 0
        {
            return Err("sample rate does not divide an 80 ms frame".into());
        }
        if self.persona.id.trim().is_empty() {
            return Err("persona id is empty".into());
        }
        Ok(())
    }
}
