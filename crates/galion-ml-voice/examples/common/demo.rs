// Galion Voice — Perpetual Open MIT License
// SPDX-License-Identifier: MIT
//! W20-20 shared round-trip demo logic (used by `examples/roundtrip.rs`
//! and `tests/roundtrip_wav.rs` via `#[path]`). ENGLISH-ONLY, no weights.
//!
//! Pipeline: canned EN audio -> StubAsr -> echo reply -> StubTts -> 24 kHz WAV.
#![allow(dead_code)]

use galion_ml_voice::stub::*;
use galion_ml_voice::*;

/// Output WAV rate (Mimi-class 24 kHz). Must equal `stub::SAMPLE_RATE`.
pub const WAV_RATE_HZ: u32 = 24_000;

/// One completed turn.
#[derive(Debug, Clone, PartialEq)]
pub struct Turn {
    pub lang: LanguageId,
    pub heard: String,
    pub reply: String,
    pub audio: Audio,
}

/// Deterministic "LLM" slot: echo back, tagged with the language token.
pub fn echo_reply(t: &Transcript) -> Result<String, VoiceError> {
    if t.text.trim().is_empty() {
        return Err(VoiceError::EmptyInput);
    }
    Ok(with_lang_token(t.lang, &format!("you said: {}", t.text)))
}

/// Run one round trip. `lang_code` must be `en` (anything else is rejected).
pub fn round_trip(input_text: &str, lang_code: &str) -> Result<Turn, VoiceError> {
    let lang = LanguageId::from_code(lang_code)
        .ok_or_else(|| VoiceError::UnsupportedLanguage(lang_code.to_string()))?;
    // Canned mic input: the synthetic fixture encoding of `input_text`.
    let mic = fixture_audio(input_text);
    let router = EnglishOnlyRouter;
    let routed = router.route_audio(&mic);
    let mut asr = StubAsr;
    let t = asr.transcribe(&mic, Some(lang))?;
    debug_assert_eq!(routed, t.lang);
    let reply = echo_reply(&t)?;
    let out_lang = router.route_text(&reply);
    let mut tts = StubTts;
    let audio = tts.synthesize(&reply, out_lang)?;
    Ok(Turn { lang: out_lang, heard: t.text, reply, audio })
}

/// f32 [-1,1] -> PCM16 with clamping.
pub fn to_pcm16(samples: &[f32]) -> Vec<i16> {
    samples.iter().map(|s| (s.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16).collect()
}

/// Encode mono PCM16 little-endian RIFF/WAVE.
pub fn wav_bytes(audio: &Audio) -> Vec<u8> {
    let pcm = to_pcm16(&audio.samples);
    let rate = audio.sample_rate;
    let data_len = (pcm.len() * 2) as u32;
    let mut v = Vec::with_capacity(44 + data_len as usize);
    v.extend_from_slice(b"RIFF");
    v.extend_from_slice(&(36 + data_len).to_le_bytes());
    v.extend_from_slice(b"WAVEfmt ");
    v.extend_from_slice(&16u32.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes()); // PCM
    v.extend_from_slice(&1u16.to_le_bytes()); // mono
    v.extend_from_slice(&rate.to_le_bytes());
    v.extend_from_slice(&(rate * 2).to_le_bytes()); // byte rate
    v.extend_from_slice(&2u16.to_le_bytes()); // block align
    v.extend_from_slice(&16u16.to_le_bytes()); // bits/sample
    v.extend_from_slice(b"data");
    v.extend_from_slice(&data_len.to_le_bytes());
    for s in pcm {
        v.extend_from_slice(&s.to_le_bytes());
    }
    v
}
