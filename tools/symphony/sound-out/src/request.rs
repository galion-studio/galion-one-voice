// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! The request line the Chatterbox worker already accepts, plus the knobs
//! it used to only log. Keys match `clone_request_patch` in PR #192.

use serde::{Deserialize, Serialize};

fn one() -> f64 {
    1.0
}

fn warmth_default() -> f64 {
    0.5
}

fn temperature_default() -> f64 {
    0.8
}

fn persona_default() -> String {
    "samantha".into()
}

fn track_default() -> String {
    "prosody".into()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Prosody {
    /// Semitones relative to the speaker baseline. 0 leaves pitch alone.
    #[serde(default)]
    pub pitch_st: f64,
    /// Logged. The worker does not scale the waveform by this. 1 is unity.
    #[serde(default = "one")]
    pub range: f64,
    /// Pitch-preserving speaking rate. The worker clamps 0.8..=1.2.
    #[serde(default = "one")]
    pub rate: f64,
    /// Amplitude multiplier. The worker clamps 0.6..=1.2.
    #[serde(default = "one")]
    pub energy: f64,
    /// Logged by the worker. Not applied to the samples.
    #[serde(default = "warmth_default")]
    pub warmth: f64,
    /// Leading silence, milliseconds. The worker clamps to 400.
    #[serde(default)]
    pub pre_pause_ms: u32,
}

impl Default for Prosody {
    fn default() -> Self {
        Self::identity()
    }
}

impl Prosody {
    pub fn identity() -> Self {
        Self {
            pitch_st: 0.0,
            range: 1.0,
            rate: 1.0,
            energy: 1.0,
            warmth: 0.5,
            pre_pause_ms: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CloneRequest {
    pub text: String,
    #[serde(default = "persona_default")]
    pub persona: String,
    #[serde(default)]
    pub emotion: String,
    #[serde(default = "track_default")]
    pub track: String,
    #[serde(default)]
    pub exaggeration: f64,
    #[serde(default)]
    pub cfg_weight: f64,
    #[serde(default = "temperature_default")]
    pub temperature: f64,
    #[serde(default)]
    pub prosody: Prosody,
}

impl CloneRequest {
    pub fn from_text(text: &str) -> Self {
        Self {
            text: text.to_string(),
            persona: persona_default(),
            emotion: String::new(),
            track: track_default(),
            exaggeration: 0.0,
            cfg_weight: 0.0,
            temperature: 0.8,
            prosody: Prosody::identity(),
        }
    }

    /// Same clamps as the worker (`controls_of` in `chatterbox_worker.py`)
    /// and `FrameProsody::clamped` in galion-ml-voice.
    pub fn normalized(mut self) -> Self {
        self.exaggeration = finite(self.exaggeration, 0.0).clamp(0.0, 2.0);
        self.cfg_weight = finite(self.cfg_weight, 0.0).clamp(0.0, 1.0);
        self.temperature = finite(self.temperature, 0.8).clamp(0.05, 1.5);
        self.prosody.pitch_st = finite(self.prosody.pitch_st, 0.0).clamp(-4.0, 4.0);
        self.prosody.range = finite(self.prosody.range, 1.0).clamp(0.5, 1.5);
        self.prosody.rate = finite(self.prosody.rate, 1.0).clamp(0.8, 1.2);
        self.prosody.energy = finite(self.prosody.energy, 1.0).clamp(0.6, 1.2);
        self.prosody.warmth = finite(self.prosody.warmth, 0.5).clamp(0.0, 1.0);
        self.prosody.pre_pause_ms = self.prosody.pre_pause_ms.min(400);
        if self.persona.trim().is_empty() {
            self.persona = persona_default();
        }
        if self.track.trim().is_empty() {
            self.track = track_default();
        }
        self
    }

    /// One JSON line for `scripts/clone/chatterbox_worker.py`.
    /// Device stays `cpu` because that process refuses anything else.
    pub fn worker_line(&self) -> String {
        let req = self.clone().normalized();
        serde_json::json!({
            "text": req.text,
            "persona": req.persona,
            "model": "nano",
            "tags": req.persona != "jarvis",
            "device": "cpu",
            "denoise": false,
            "emotion": req.emotion,
            "track": req.track,
            "exaggeration": req.exaggeration,
            "cfg_weight": req.cfg_weight,
            "temperature": req.temperature,
            "prosody": {
                "pitch_st": req.prosody.pitch_st,
                "range": req.prosody.range,
                "rate": req.prosody.rate,
                "energy": req.prosody.energy,
                "warmth": req.prosody.warmth,
                "pre_pause_ms": req.prosody.pre_pause_ms,
            }
        })
        .to_string()
    }
}

fn finite(x: f64, fallback: f64) -> f64 {
    if x.is_finite() {
        x
    } else {
        fallback
    }
}
