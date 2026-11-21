// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Samantha emotion prosody.
//!
//! A live HORBM [`Pad`] becomes Chatterbox exaggeration, Piper length and
//! noise scales, pause lengths, and an optional `[inhale]` or `[giggle]`
//! hint. The applied mood slews on the 80 ms heartbeat so one new sample
//! cannot jump the voice.
//!
//! This crate does not synthesise audio, does not load a reference clip,
//! and does not implement HORBM. [`ReplyEngine`] is the drop-in seam.

#![forbid(unsafe_code)]

mod clock;
mod engine;
mod map;
mod pad;

pub use clock::{
    slew_alpha, ProsodyClock, GIGGLE_COOLDOWN_TICKS, GIGGLE_HOLD_TICKS, INHALE_COOLDOWN_TICKS,
    INHALE_HOLD_TICKS, TAU_S,
};
pub use engine::{FakeReplyEngine, ReplyEngine, TurnOut};
pub use map::{
    giggle_open, inhale_open, is_whisper_token, map, SpeakingParams, Tag, BASE_ENERGY, BASE_LENGTH,
    BASE_NOISE, BASE_NOISE_W, BASE_PITCH_ST, BASE_PRE_PAUSE_MS, BASE_SENTENCE_SILENCE_S,
    ENERGY_MAX, ENERGY_MIN, EXAGGERATION_MAX, EXAGGERATION_MIN, EXAGGERATION_REST, LENGTH_MAX,
    LENGTH_MIN, NOISE_MAX, NOISE_MIN, NOISE_W_MAX, NOISE_W_MIN, PAUSE_CEILING_MS, PAUSE_MIN_MS,
    PERSONA_ALLOWED_TAGS, PITCH_BOUND_ST, SILENCE_MAX_S, SILENCE_MIN_S, WHISPER_TOKENS,
    WORKER_RATE_MAX, WORKER_RATE_MIN,
};
pub use pad::Pad;

/// Heartbeat period. `crates/galion-heartbeat` `FRAME_MS`, LOCKS `frame_ms`.
pub const TICK_MS: u32 = 80;
/// `galion-heartbeat` `SAMPLE_RATE` and voice-v2 `TICK_S`.
pub const SAMPLE_RATE_HZ: u32 = 24_000;
pub const TICK_S: f32 = TICK_MS as f32 / 1000.0;
/// Samples in one tick at 24 kHz. `galion-heartbeat` `FRAME_SAMPLES` is 1920.
pub const TICK_SAMPLES: u32 = SAMPLE_RATE_HZ * TICK_MS / 1000;

/// Run `n` ticks and return the elapsed time. Used by the binary and by the
/// timing test. The accumulator keeps the map from being optimised out.
pub fn measure_ticks(n: u32) -> std::time::Duration {
    let mut clock = ProsodyClock::new();
    clock.set_target(Pad {
        arousal: 0.8,
        valence: 0.7,
        dominance: 0.6,
    });
    let start = std::time::Instant::now();
    let mut acc = 0.0f32;
    for i in 0..n {
        if i == n / 2 {
            clock.set_target(Pad {
                arousal: 0.12,
                valence: 0.2,
                dominance: 0.4,
            });
        }
        let params = clock.tick();
        acc += params.exaggeration + params.length_scale + params.energy + params.pre_pause_ms;
    }
    std::hint::black_box(acc);
    start.elapsed()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_matches_the_heartbeat() {
        assert_eq!(TICK_MS, 80);
        assert_eq!(SAMPLE_RATE_HZ, 24_000);
        assert_eq!(TICK_SAMPLES, 1_920);
        assert!((TICK_S - 0.08).abs() < 1e-6);
    }
}
