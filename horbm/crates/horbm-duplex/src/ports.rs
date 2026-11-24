// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Seams for layers that are still open pull requests.
//!
//! The real crates are not on `main`, so this loop does not name them.
//! A later adapter implements these traits and the stand-ins in the
//! integration test go away.
//!
//! | Port | Open change | What plugs in |
//! |---|---|---|
//! | [`Recognizer`] | speech recognizer, PR 279, `horbm-asr::SpeechRecognizer` | `push_frame` / `finish` / `cancel` |
//! | [`Emotion`] | emotion core, PR 263, `EmotionCore::step` | one affect sample per frame |
//! | [`Breath`] | breath, PR 278, `plan_speech` | one control cue per frame |
//! | [`Persona`] | persona, PR 276, `Persona::samantha` | id, voice, style, miss line |
//!
//! Development speaks as Samantha only. Another id is refused.

/// Share of brevity taken off a rate of 1. Same knob as the persona style hook.
pub const BREVITY_RATE: f32 = 0.15;

/// Development speaker. The loop refuses any other id or voice.
pub const DEV_PERSONA: &str = "samantha";

/// Partial hypotheses can be replaced. The final is the text to keep.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HypothesisKind {
    Partial,
    Final,
}

/// One hypothesis. Times are on the utterance clock, in milliseconds.
///
/// Empty text is not a transcript. A failure is [`RecogError`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hypothesis {
    pub kind: HypothesisKind,
    pub text: String,
    pub start_ms: u32,
    pub end_ms: u32,
}

/// A frame was the wrong length, or the engine failed.
///
/// The display text is not a transcript. The loop does not speak it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecogError {
    Frame { got: usize, expected: usize },
    Engine(String),
}

impl std::fmt::Display for RecogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Frame { got, expected } => {
                write!(f, "audio frame is {got} samples, want {expected}")
            }
            Self::Engine(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for RecogError {}

/// Streaming recognizer. One thread. No async runtime.
///
/// [`Recognizer::push_frame`] takes one ear frame of [`FRAME_SAMPLES`]
/// samples. [`Recognizer::finish`] closes the utterance.
/// [`Recognizer::cancel`] drops the open hypothesis and emits nothing.
pub trait Recognizer {
    fn push_frame(&mut self, pcm: &[i16]) -> Result<Vec<Hypothesis>, RecogError>;
    fn finish(&mut self) -> Result<Vec<Hypothesis>, RecogError>;
    fn cancel(&mut self) -> Result<(), RecogError>;
    /// Audio accepted in the open utterance, in milliseconds.
    fn audio_ms(&self) -> u32;
    fn name(&self) -> &str;
}

/// Pleasure, arousal, dominance for this frame. Plain numbers.
///
/// The emotion-core moment is four axes. This port keeps the three the
/// voice already stores on a turn. Approach can wait for the real crate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affect {
    pub valence: f32,
    pub arousal: f32,
    pub dominance: f32,
}

/// One step per 80 ms frame. The loop does not store the baseline.
pub trait Emotion {
    fn step(&mut self, t_ms: u32) -> Affect;
}

/// Where the breath is. Speech is an exhale.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BreathPhase {
    Inhale,
    Exhale,
    Hold,
}

/// One control cue. No samples.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BreathCue {
    pub phase: BreathPhase,
    pub speaking: bool,
}

/// One cue per frame. A later adapter can index a `plan_speech` track.
pub trait Breath {
    fn cue(&mut self, t_ms: u32, speaking: bool) -> BreathCue;
}

/// What the voice reads from the persona. The moment is not an input.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Style {
    pub brevity: f32,
    pub warmth: f32,
    /// Sentences one turn may spend.
    pub max_sentences: u8,
    /// `1 - BREVITY_RATE * brevity`.
    pub rate: f32,
}

impl Style {
    #[must_use]
    pub fn from_brevity(brevity: f32, warmth: f32, max_sentences: u8) -> Self {
        Self {
            brevity,
            warmth,
            max_sentences,
            rate: 1.0 - BREVITY_RATE * brevity,
        }
    }
}

/// The character that stays put for the whole loop.
pub trait Persona {
    fn id(&self) -> &str;
    fn voice(&self) -> &str;
    fn style(&self) -> Style;
    /// The spoken miss. Development requires `I don't know.`
    fn unknown_line(&self) -> &str;
}
