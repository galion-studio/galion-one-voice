// Galion Voice — Perpetual Open MIT License
// SPDX-License-Identifier: MIT
//! W20-20: canned English round trip -> 24 kHz WAV.
#[path = "../examples/common/demo.rs"]
mod demo;

use galion_ml_voice::stub::*;
use galion_ml_voice::*;

fn u32le(b: &[u8], o: usize) -> u32 { u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]) }
fn u16le(b: &[u8], o: usize) -> u16 { u16::from_le_bytes([b[o], b[o + 1]]) }

#[test]
fn canned_english_round_trip() {
    let t = demo::round_trip(FIXTURE_EN_TEXT, "en").unwrap();
    assert_eq!(t.lang, LanguageId::En);
    assert_eq!(t.heard, "hello galion");
    assert_eq!(t.reply, "<|en|>you said: hello galion");
    assert_eq!(t.audio.sample_rate, 24_000);
    assert_eq!(t.audio.samples.len(), "you said: hello galion".len() * SAMPLES_PER_CHAR);
}

#[test]
fn tts_output_decodes_back_to_reply() {
    // Closing the loop: feed TTS output back through StubAsr.
    let t = demo::round_trip(FIXTURE_EN_TEXT, "en").unwrap();
    let back = StubAsr.transcribe(&t.audio, None).unwrap();
    assert_eq!(back.text, "you said: hello galion");
}

#[test]
fn wav_is_24khz_mono_pcm16() {
    let t = demo::round_trip(FIXTURE_EN_TEXT, "en").unwrap();
    let w = demo::wav_bytes(&t.audio);
    assert_eq!(&w[0..4], b"RIFF");
    assert_eq!(&w[8..16], b"WAVEfmt ");
    assert_eq!(u32le(&w, 16), 16);
    assert_eq!(u16le(&w, 20), 1, "PCM");
    assert_eq!(u16le(&w, 22), 1, "mono");
    assert_eq!(u32le(&w, 24), demo::WAV_RATE_HZ);
    assert_eq!(u32le(&w, 24), SAMPLE_RATE);
    assert_eq!(u32le(&w, 28), 48_000);
    assert_eq!(u16le(&w, 32), 2);
    assert_eq!(u16le(&w, 34), 16);
    assert_eq!(&w[36..40], b"data");
    assert_eq!(u32le(&w, 40) as usize, t.audio.samples.len() * 2);
    assert_eq!(u32le(&w, 4) as usize, w.len() - 8);
}

#[test]
fn deterministic_bytes() {
    let a = demo::wav_bytes(&demo::round_trip(FIXTURE_EN_TEXT, "en").unwrap().audio);
    let b = demo::wav_bytes(&demo::round_trip(FIXTURE_EN_TEXT, "en").unwrap().audio);
    assert_eq!(a, b);
}

#[test]
fn non_english_rejected() {
    for tag in ["he-IL", "he", "pl", "pl-PL", "es", "zh", ""] {
        assert!(matches!(demo::round_trip(FIXTURE_EN_TEXT, tag), Err(VoiceError::UnsupportedLanguage(_))), "{tag}");
    }
}

#[test]
fn empty_input_rejected() {
    assert_eq!(demo::round_trip("", "en").unwrap_err(), VoiceError::EmptyInput);
}

#[test]
fn pcm16_clamps() {
    assert_eq!(demo::to_pcm16(&[2.0, -2.0, 0.0]), vec![i16::MAX, -i16::MAX, 0]);
}
