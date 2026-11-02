// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! V1-03 real-speech path (feature `speech`). ENGLISH ONLY.
//!
//! Selection order (first that can actually speak wins):
//! 1. [`weights::A21WeightTts`] — the A21 neural TTS slot (FP-03). Activates when a
//!    sha256-verified voice pack is on disk (`GALION_TTS_WEIGHTS` /
//!    `GALION_SPEECH_WEIGHTS` or `<repo>/weights/tts/`) AND a decoder for it exists
//!    ([`weights::decoder_for`]). Today: Piper VITS `en_US-ljspeech-medium` (MIT),
//!    decoded on the CPU by the out-of-process Piper runtime. With no pack it
//!    reports `Absent` and the stack falls back to flite.
//! 2. [`flite::FliteTts`] — FALLBACK system TTS (only when the A21 pack/runtime is
//!    missing): the distro `libflite1`, opened with `dlopen`. Real words, not neural.
//! 3. none — the caller must fail loudly (FRXDEL Q2: the EN cascade must say
//!    actual words; tones are not an acceptable reply).
//!
//! Env: `GALION_SPEECH=auto|a21|flite|off` (default `auto`: a21, else flite),
//!      `GALION_A21_RUNTIME` (Piper exe), `GALION_A21_NOISE=natural` (sampled prosody),
//!      `GALION_FLITE_VOICE=slt|kal16|awb|rms|kal` (default `slt`).

use crate::{Audio, LanguageId, TtsEngine, VoiceError};

pub mod flite;
pub mod sha256;
pub mod weights;

/// Output rate of every speech backend (Mimi-class 24 kHz lane bus).
pub const SPEECH_RATE: u32 = crate::stub::SAMPLE_RATE;

/// Which backend produced (or would produce) speech.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpeechBackend {
    /// A21 neural TTS (Piper VITS voice pack, CPU, out of process).
    A21Weights { voice: String },
    /// Interim system TTS (libflite, runtime dlopen). Real words, not neural.
    SystemFlite { voice: String },
    /// Nothing can speak on this machine.
    None { reason: String },
}

impl SpeechBackend {
    /// Stable short id for JSON / UI labels.
    pub fn id(&self) -> &'static str {
        match self {
            SpeechBackend::A21Weights { .. } => "a21-weights",
            SpeechBackend::SystemFlite { .. } => "system-flite",
            SpeechBackend::None { .. } => "none",
        }
    }
    /// Gate/runbook token: `SPEECH=a21|flite|ABSENT`.
    pub fn speech_token(&self) -> &'static str {
        match self { SpeechBackend::A21Weights { .. } => "a21", SpeechBackend::SystemFlite { .. } => "flite", SpeechBackend::None { .. } => "ABSENT" }
    }
    /// Human label; always states what it is NOT.
    pub fn label(&self) -> String {
        match self {
            SpeechBackend::A21Weights { voice } => format!("A21 neural TTS (Piper VITS {voice}; CPU; MIT)"),
            SpeechBackend::SystemFlite { voice } => format!("fallback system TTS (libflite cmu_us_{voice}; real words; NOT A21, NOT neural)"),
            SpeechBackend::None { reason } => format!("no speech backend ({reason})"),
        }
    }
}

/// The selected speech engine plus an honest status report.
pub struct SpeechStack {
    pub backend: SpeechBackend,
    pub weights: weights::WeightStatus,
    engine: Option<Box<dyn TtsEngine + Send>>,
}

impl SpeechStack {
    /// Detect from env + disk. Never downloads anything.
    pub fn detect() -> Self {
        let mode = std::env::var("GALION_SPEECH").unwrap_or_else(|_| "auto".into()).to_ascii_lowercase();
        let weights = weights::WeightStatus::discover();
        if mode == "off" {
            return Self { backend: SpeechBackend::None { reason: "GALION_SPEECH=off".into() }, weights, engine: None };
        }
        if mode == "auto" || mode == "a21" {
            if let Ok(t) = weights::A21WeightTts::try_new(&weights) {
                let voice = t.voice().to_string();
                return Self { backend: SpeechBackend::A21Weights { voice }, weights, engine: Some(Box::new(t)) };
            }
            if mode == "a21" {
                let reason = format!("GALION_SPEECH=a21 but A21 slot inactive: {}", weights.summary());
                return Self { backend: SpeechBackend::None { reason }, weights, engine: None };
            }
        }
        if mode == "auto" || mode == "flite" { match flite::FliteTts::open_default() {
            Ok(t) => {
                let voice = t.voice().to_string();
                Self { backend: SpeechBackend::SystemFlite { voice }, weights, engine: Some(Box::new(t)) }
            }
            Err(e) => Self { backend: SpeechBackend::None { reason: format!("{e}; {}", weights.summary()) }, weights, engine: None },
        } } else {
            Self { backend: SpeechBackend::None { reason: format!("unknown GALION_SPEECH={mode} (auto|a21|flite|off)") }, weights, engine: None }
        }
    }

    /// True if this stack can produce spoken words.
    pub fn can_speak(&self) -> bool { self.engine.is_some() }

    /// Speak `text` (English) at [`SPEECH_RATE`].
    pub fn speak(&mut self, text: &str) -> Result<Audio, VoiceError> {
        match self.engine.as_mut() {
            Some(e) => e.synthesize(text, LanguageId::En),
            None => Err(VoiceError::Backend(self.backend.label())),
        }
    }
}

/// Cubic (Catmull-Rom) resampler, mono. Deterministic; adequate for speech
/// up-sampling 16 kHz -> 24 kHz.
pub fn resample(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || input.is_empty() { return input.to_vec(); }
    let n_out = ((input.len() as u64 * to as u64) / from as u64) as usize;
    let at = |i: isize| -> f32 { input[i.clamp(0, input.len() as isize - 1) as usize] };
    (0..n_out)
        .map(|k| {
            let pos = k as f64 * from as f64 / to as f64;
            let i = pos.floor() as isize;
            let t = (pos - i as f64) as f32;
            let (p0, p1, p2, p3) = (at(i - 1), at(i), at(i + 1), at(i + 2));
            let a = -0.5 * p0 + 1.5 * p1 - 1.5 * p2 + 0.5 * p3;
            let b = p0 - 2.5 * p1 + 2.0 * p2 - 0.5 * p3;
            let c = -0.5 * p0 + 0.5 * p2;
            ((a * t + b) * t + c) * t + p1
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn resample_len_and_identity() {
        let x: Vec<f32> = (0..1600).map(|i| (i as f32 * 0.01).sin()).collect();
        assert_eq!(resample(&x, 16_000, 24_000).len(), 2400);
        assert_eq!(resample(&x, 24_000, 24_000), x);
    }
    #[test] fn off_mode_never_speaks() {
        // Pure function check on the label path (env is process-global; not mutated here).
        let b = SpeechBackend::None { reason: "x".into() };
        assert_eq!(b.id(), "none");
        assert!(SpeechBackend::SystemFlite { voice: "slt".into() }.label().contains("NOT A21"));
    }
}
