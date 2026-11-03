// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! Utterances the virtual user can hand to PhoneLane.
//! Text is not audio. A fixture WAV is the stub codec (exact decode). A voice WAV is
//! a synthetic tone with no words in it, used to check the interim placeholder.

use galion_cascade::en_lane::{to_pcm16, wav_bytes};
use galion_ml_voice::stub::fixture_audio;

/// Stub-fixture WAV. PhoneLane decodes `text` exactly and labels it `stub-fixture`.
pub fn fixture_wav(text: &str) -> Vec<u8> {
    let audio = fixture_audio(text);
    let pcm = to_pcm16(&audio.samples);
    wav_bytes(&pcm, audio.sample_rate)
}

/// Synthetic voiced PCM at 16 kHz. Not a fixture, so interim mode must not treat it as words.
pub fn voice_pcm(secs: f32) -> Vec<i16> {
    let n = (secs * 16_000.0) as usize;
    (0..n)
        .map(|i| {
            let t = i as f32 / 16_000.0;
            (0.2 * (t * 180.0 * std::f32::consts::TAU).sin()
                * (t * 3.0 * std::f32::consts::TAU).sin().abs()
                * 32767.0) as i16
        })
        .collect()
}

pub fn voice_wav(secs: f32) -> Vec<u8> {
    wav_bytes(&voice_pcm(secs), 16_000)
}
