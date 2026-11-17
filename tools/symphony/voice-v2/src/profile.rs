// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! The voice file is TOML and is the pluggable input. The decoded profile
//! is JSON. Baseline and feeling are different objects in both.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::emotion::{EmotionId, EXTENDED, RELIABLE};
use crate::fluid::{BaselineVoice, InnerFactors, OuterFactors, TickFeeling};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatterboxSlot {
    #[serde(default)]
    pub bin: String,
    #[serde(default)]
    pub voice: String,
    #[serde(default)]
    pub voice_wav: String,
    #[serde(default)]
    pub weights: String,
    /// Optional file of schema `galion-voice-v2-chatterbox-params`.
    /// TODO(features): the decode-JSON → Chatterbox param map lands here.
    #[serde(default)]
    pub params_json: String,
    #[serde(default = "default_repo")]
    pub repo_id: String,
    #[serde(default = "default_revision")]
    pub revision: String,
    #[serde(default = "default_dtype")]
    pub dtype: String,
}

fn default_repo() -> String {
    "ResembleAI/chatterbox-turbo-ONNX".into()
}
fn default_revision() -> String {
    "d21799bd0354adb85e348b8a0442a8405110a2cf".into()
}
fn default_dtype() -> String {
    "fp32".into()
}

impl Default for ChatterboxSlot {
    fn default() -> Self {
        Self {
            bin: String::new(),
            voice: String::new(),
            voice_wav: String::new(),
            weights: String::new(),
            params_json: String::new(),
            repo_id: default_repo(),
            revision: default_revision(),
            dtype: default_dtype(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VoiceFile {
    #[serde(default = "default_schema")]
    pub schema: String,
    pub id: String,
    #[serde(default)]
    pub reference_is_neutral: bool,
    pub baseline: BaselineVoice,
    #[serde(default)]
    pub chatterbox: ChatterboxSlot,
}

fn default_schema() -> String {
    "galion-voice-v2-voice".into()
}

impl VoiceFile {
    pub fn synthetic() -> Self {
        Self {
            schema: default_schema(),
            id: "synthetic-a".into(),
            reference_is_neutral: true,
            baseline: BaselineVoice::default(),
            chatterbox: ChatterboxSlot::default(),
        }
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SectionBlock {
    pub status: String,
    pub source: String,
    pub features: BTreeMap<String, f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub todo: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BreathBlock {
    pub status: String,
    /// Fraction of ticks with voicing under 0.5. Not a respiratory rate.
    pub unvoiced_fraction: Option<f64>,
    pub todo: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EnvironmentBlock {
    #[serde(flatten)]
    pub outer: OuterFactors,
    pub mix: String,
    pub reverb: String,
    pub todo: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaxonomyBlock {
    pub set: String,
    pub spoken_words: String,
    pub reliable_in_prosody: Vec<String>,
    pub extended: Vec<String>,
    pub neutral: String,
    pub breath: String,
}

impl TaxonomyBlock {
    pub fn cowen24() -> Self {
        Self {
            set: "cowen-elfenbein-laukka-keltner-2019-24".into(),
            spoken_words: "all 24 categories are legal on the prosody track".into(),
            reliable_in_prosody: RELIABLE.iter().map(|s| (*s).to_string()).collect(),
            extended: EXTENDED.iter().map(|s| (*s).to_string()).collect(),
            neutral: "speaker baseline, not one of the 24".into(),
            breath: "physiology, not one of the 24".into(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VoiceProfile {
    pub schema: u32,
    pub id: String,
    pub role: String,
    pub sample_rate_hz: u32,
    pub duration_s: f64,
    pub tick_ms: u32,
    pub n_ticks: usize,
    pub taxonomy: TaxonomyBlock,
    pub baseline: BaselineVoice,
    pub inner: InnerFactors,
    pub inner_source: String,
    pub throat: SectionBlock,
    pub mouth: SectionBlock,
    pub nose: SectionBlock,
    pub breath: BreathBlock,
    pub environment: EnvironmentBlock,
    pub ticks: Vec<TickFeeling>,
    pub stubs: Vec<String>,
}

impl VoiceProfile {
    pub fn modal_prosody(&self) -> Option<EmotionId> {
        let mut counts: BTreeMap<EmotionId, usize> = BTreeMap::new();
        for tick in &self.ticks {
            if let Some(id) = tick.prosody {
                *counts.entry(id).or_insert(0) += 1;
            }
        }
        counts.into_iter().max_by_key(|(_, n)| *n).map(|(id, _)| id)
    }

    pub fn modal_burst(&self) -> Option<EmotionId> {
        let mut counts: BTreeMap<EmotionId, usize> = BTreeMap::new();
        for tick in &self.ticks {
            if let Some(id) = tick.burst {
                *counts.entry(id).or_insert(0) += 1;
            }
        }
        counts.into_iter().max_by_key(|(_, n)| *n).map(|(id, _)| id)
    }
}

/// Drop-in the features agent can write. Absent file = pipeline-default pitch.
#[derive(Clone, Debug, Deserialize)]
pub struct ChatterboxParamFile {
    pub schema: String,
    #[serde(default)]
    pub by_emotion: BTreeMap<String, ChatterboxParamRow>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ChatterboxParamRow {
    pub pitch_semitones: Option<f32>,
    pub pace: Option<f32>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub repetition_penalty: Option<f32>,
    pub exaggeration: Option<f32>,
}

pub fn load_param_row(path: &str, emotion: &str) -> Result<Option<ChatterboxParamRow>, String> {
    if path.trim().is_empty() {
        return Ok(None);
    }
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("{path}: {e}")),
    };
    let file: ChatterboxParamFile =
        serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))?;
    if file.schema != "galion-voice-v2-chatterbox-params" {
        return Err(format!(
            "{path}: schema is {:?}, want galion-voice-v2-chatterbox-params",
            file.schema
        ));
    }
    Ok(file.by_emotion.get(emotion).cloned())
}
