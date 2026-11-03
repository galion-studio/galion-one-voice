// Galion Voice — Perpetual Open MIT License
//! `galion-ml-voice` — ENGLISH-ONLY prototype scaffold (agent W20-19).
//!
//! Provides:
//! - [`LanguageId`] with a single variant [`LanguageId::En`] and the `<|en|>`
//!   control-token helper ([`LanguageId::token`], [`LanguageId::from_token`]).
//! - Stub traits [`AsrEngine`], [`TtsEngine`], [`LangRouter`].
//! - Deterministic synthetic stubs ([`stub`]) for tests — no weights, no I/O.
//!
//! NOTE: More languages come later per the multilingual plan
//! (docs/multilingual/; English + Polish priority, then the top-20 spoken
//! languages). This prototype intentionally ships English only — do NOT add
//! language modules here until the multilingual plan is approved.

// V1-03: `deny` (was `forbid`) so the opt-in `speech::flite` FFI module can
// `allow` it locally. Every other module stays unsafe-free.
#![deny(unsafe_code)]

use core::fmt;

pub mod stub;
/// Jarvis hub texture: a separate on/off layer beside the speech asset.
/// The toggle ABI and the energy-band gate live in [`jarvis_texture`].
pub mod jarvis_texture;
/// V1-03 real-speech path (feature `speech`): A21 weight slot + interim system TTS.
#[cfg(feature = "speech")]
pub mod speech;

/// Supported languages. English only for the prototype.
///
/// More languages come later per the multilingual plan; adding a variant is
/// expected to be a non-breaking-by-design change (`#[non_exhaustive]`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum LanguageId {
    /// English.
    En,
}

impl LanguageId {
    /// All languages enabled in this build.
    pub const ALL: &'static [LanguageId] = &[LanguageId::En];

    /// ISO 639-1 code.
    pub const fn code(self) -> &'static str {
        match self {
            LanguageId::En => "en",
        }
    }

    /// Whisper/Grok-style language control token, e.g. `<|en|>`.
    pub const fn token(self) -> &'static str {
        match self {
            LanguageId::En => "<|en|>",
        }
    }

    /// Parse an ISO 639-1 code (case-insensitive, trimmed).
    pub fn from_code(code: &str) -> Option<Self> {
        let c = code.trim();
        if c.eq_ignore_ascii_case("en") {
            Some(LanguageId::En)
        } else {
            None
        }
    }

    /// Parse a control token such as `<|en|>`.
    pub fn from_token(tok: &str) -> Option<Self> {
        let inner = tok.trim().strip_prefix("<|")?.strip_suffix("|>")?;
        Self::from_code(inner)
    }
}

impl fmt::Display for LanguageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

/// Prefix `text` with the language token: `"<|en|>hello"`.
pub fn with_lang_token(lang: LanguageId, text: &str) -> String {
    let mut s = String::with_capacity(lang.token().len() + text.len());
    s.push_str(lang.token());
    s.push_str(text);
    s
}

/// Split a leading language token off `text`, if present and known.
pub fn split_lang_token(text: &str) -> (Option<LanguageId>, &str) {
    if let Some(rest) = text.strip_prefix("<|") {
        if let Some(end) = rest.find("|>") {
            if let Some(lang) = LanguageId::from_code(&rest[..end]) {
                return (Some(lang), &rest[end + 2..]);
            }
        }
    }
    (None, text)
}

/// Errors from the stub engines.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VoiceError {
    /// Language not enabled in this build.
    UnsupportedLanguage(String),
    /// Empty / invalid input.
    EmptyInput,
    /// V1-03: a speech backend is missing or failed (e.g. A21 weights absent).
    Backend(String),
    /// The caller superseded this utterance. A warm Piper process is left running.
    Cancelled,
}

impl fmt::Display for VoiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VoiceError::UnsupportedLanguage(l) => write!(f, "unsupported language: {l}"),
            VoiceError::EmptyInput => f.write_str("empty input"),
            VoiceError::Backend(m) => write!(f, "speech backend: {m}"),
            VoiceError::Cancelled => f.write_str("speech cancelled"),
        }
    }
}

impl std::error::Error for VoiceError {}

/// Mono PCM audio buffer (f32, [-1, 1]).
#[derive(Clone, Debug, PartialEq)]
pub struct Audio {
    pub sample_rate: u32,
    pub samples: Vec<f32>,
}

/// ASR result.
#[derive(Clone, Debug, PartialEq)]
pub struct Transcript {
    pub lang: LanguageId,
    pub text: String,
}

/// Speech-to-text engine.
pub trait AsrEngine {
    /// Languages this engine can transcribe.
    fn languages(&self) -> &[LanguageId];
    /// Transcribe `audio`; `hint` is an optional forced language.
    fn transcribe(&mut self, audio: &Audio, hint: Option<LanguageId>) -> Result<Transcript, VoiceError>;
}

/// Text-to-speech engine.
pub trait TtsEngine {
    /// Languages this engine can speak.
    fn languages(&self) -> &[LanguageId];
    /// Synthesize `text` in `lang`.
    fn synthesize(&mut self, text: &str, lang: LanguageId) -> Result<Audio, VoiceError>;
}

/// Picks the language for a turn (fast switching lives here later).
pub trait LangRouter {
    /// Detect language of text (e.g. an ASR draft or a `<|xx|>`-tagged string).
    fn route_text(&self, text: &str) -> LanguageId;
    /// Detect language from audio.
    fn route_audio(&self, audio: &Audio) -> LanguageId;
}
