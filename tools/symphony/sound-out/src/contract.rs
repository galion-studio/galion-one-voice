// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Local stand-in for the intent contract.
//!
//! TODO(S1): replace this module with `galion-mne-contract` when that crate
//! lands. Sound-out reads the spoken text, the body inputs the lungs layer
//! already accepts, the clone controls from PR #192's `clone_request_patch`,
//! and the room / distance / noise on the moment. Feeling, throat, mouth and
//! nose own everything else.

use crate::environment::Environment;
use crate::request::{CloneRequest, Prosody};
use serde::{Deserialize, Serialize};

fn neutral() -> String {
    "neutral".into()
}

fn half() -> f64 {
    0.5
}

fn prosody_track() -> String {
    "prosody".into()
}

/// One moment, as sound-out needs it. Field names follow the voice-v2
/// pipeline's outer factors and the emotion-render clone patch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MomentFixture {
    pub text: String,
    /// A Cowen category, or `neutral` for the speaker baseline. Not `breath`.
    #[serde(default = "neutral")]
    pub category: String,
    #[serde(default)]
    pub arousal: f64,
    /// Lungs energy, 0..=1. Not the Chatterbox prosody multiplier.
    #[serde(default = "half")]
    pub energy: f64,
    #[serde(default)]
    pub fatigue: f64,
    #[serde(default)]
    pub environment: Environment,
    #[serde(default)]
    pub controls: ControlPatch,
}

/// The clone-request fields sound-out forwards. Defaults are the worker's
/// existing literals, so an empty patch does not move the voice.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ControlPatch {
    #[serde(default = "persona_default")]
    pub persona: String,
    #[serde(default = "prosody_track")]
    pub track: String,
    #[serde(default)]
    pub exaggeration: f64,
    #[serde(default)]
    pub cfg_weight: f64,
    /// Alias of `cfg_weight` on the worker request. Decoder name from PR #191.
    #[serde(default)]
    pub cfg: Option<f64>,
    #[serde(default = "temperature_default")]
    pub temperature: f64,
    #[serde(default)]
    pub prosody: Prosody,
}

fn persona_default() -> String {
    "samantha".into()
}

fn temperature_default() -> f64 {
    0.8
}

impl Default for ControlPatch {
    fn default() -> Self {
        Self {
            persona: persona_default(),
            track: prosody_track(),
            exaggeration: 0.0,
            cfg_weight: 0.0,
            cfg: None,
            temperature: 0.8,
            prosody: Prosody::identity(),
        }
    }
}

impl MomentFixture {
    pub fn new(text: &str) -> Self {
        Self {
            text: text.to_string(),
            category: neutral(),
            arousal: 0.0,
            energy: 0.5,
            fatigue: 0.0,
            environment: Environment::default(),
            controls: ControlPatch::default(),
        }
    }

    pub fn clone_request(&self) -> CloneRequest {
        let mut cfg_weight = self.controls.cfg_weight;
        if let Some(cfg) = self.controls.cfg {
            cfg_weight = cfg;
        }
        CloneRequest {
            text: self.text.clone(),
            persona: self.controls.persona.clone(),
            emotion: self.category.clone(),
            track: self.controls.track.clone(),
            exaggeration: self.controls.exaggeration,
            cfg_weight,
            temperature: self.controls.temperature,
            prosody: self.controls.prosody.clone(),
        }
        .normalized()
    }
}
