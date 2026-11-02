// Galion Voice — Perpetual Open MIT License
//! Deterministic synthetic stubs (English only). No weights, no I/O.

use crate::*;

/// Sample rate for synthetic audio (Mimi-compatible 24 kHz).
pub const SAMPLE_RATE: u32 = 24_000;
/// Samples per character in the synthetic TTS (10 ms @ 24 kHz).
pub const SAMPLES_PER_CHAR: usize = 240;

/// Synthetic fixture: a short English phrase.
pub const FIXTURE_EN_TEXT: &str = "hello galion";

/// Build a deterministic synthetic audio fixture that encodes `text`:
/// each char becomes a 240-sample constant block of amplitude `byte/255`.
pub fn fixture_audio(text: &str) -> Audio {
    let mut samples = Vec::with_capacity(text.len() * SAMPLES_PER_CHAR);
    for b in text.bytes() {
        let a = b as f32 / 255.0;
        samples.extend(core::iter::repeat(a).take(SAMPLES_PER_CHAR));
    }
    Audio { sample_rate: SAMPLE_RATE, samples }
}

fn decode_fixture(audio: &Audio) -> String {
    audio
        .samples
        .chunks(SAMPLES_PER_CHAR)
        .map(|c| (c[0] * 255.0).round() as u8 as char)
        .collect()
}

/// Stub ASR: decodes [`fixture_audio`] back to text.
#[derive(Default)]
pub struct StubAsr;

impl AsrEngine for StubAsr {
    fn languages(&self) -> &[LanguageId] {
        LanguageId::ALL
    }
    fn transcribe(&mut self, audio: &Audio, hint: Option<LanguageId>) -> Result<Transcript, VoiceError> {
        if audio.samples.is_empty() {
            return Err(VoiceError::EmptyInput);
        }
        Ok(Transcript { lang: hint.unwrap_or(LanguageId::En), text: decode_fixture(audio) })
    }
}

/// Stub TTS: produces [`fixture_audio`] for the text.
#[derive(Default)]
pub struct StubTts;

impl TtsEngine for StubTts {
    fn languages(&self) -> &[LanguageId] {
        LanguageId::ALL
    }
    fn synthesize(&mut self, text: &str, lang: LanguageId) -> Result<Audio, VoiceError> {
        let (_, body) = split_lang_token(text);
        if body.is_empty() {
            return Err(VoiceError::EmptyInput);
        }
        let _ = lang; // English only for now.
        Ok(fixture_audio(body))
    }
}

/// Stub router: honours a leading `<|xx|>` token, else English.
/// More languages / real LID come later per the multilingual plan.
#[derive(Default)]
pub struct EnglishOnlyRouter;

impl LangRouter for EnglishOnlyRouter {
    fn route_text(&self, text: &str) -> LanguageId {
        split_lang_token(text).0.unwrap_or(LanguageId::En)
    }
    fn route_audio(&self, _audio: &Audio) -> LanguageId {
        LanguageId::En
    }
}
