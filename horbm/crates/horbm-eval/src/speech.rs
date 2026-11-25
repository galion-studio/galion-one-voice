// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! User speech for one scenario, built in memory at test time.
//!
//! This is the synthetic stand-in for Piper. No model is in the tree, and
//! this crate does not download one. Nothing here opens a file or a socket.
//! A voiced frame is a sine above the duplex ear floor. A pause is digital
//! silence, which that ear treats as quiet.

use horbm_duplex::{FRAME_SAMPLES, SAMPLE_RATE};

/// Same clamp as `ProsodyEar`: voiced above −48 dBFS.
pub const EAR_FLOOR_DB: f32 = -48.0;

const AMP: f32 = 8_000.0;

/// One quiet 80 ms frame.
#[must_use]
pub fn silence() -> Vec<i16> {
    vec![0; FRAME_SAMPLES]
}

/// One voiced 80 ms frame at `hz`.
#[must_use]
pub fn tone(hz: f32) -> Vec<i16> {
    sine(hz, hz, AMP)
}

/// One voiced frame whose pitch falls across the frame.
///
/// A later end-of-turn stage can read the fall. The current conductor does
/// not. The frame stays above the ear floor for the whole 80 ms.
#[must_use]
pub fn falling() -> Vec<i16> {
    sine(200.0, 110.0, AMP)
}

/// RMS of `frame` in dBFS. Empty input is the silence floor.
#[must_use]
pub fn rms_db(frame: &[i16]) -> f32 {
    if frame.is_empty() {
        return -120.0;
    }
    let mut energy = 0.0f64;
    for sample in frame {
        let x = f64::from(*sample) / 32_768.0;
        energy += x * x;
    }
    let mean = energy / frame.len() as f64;
    (10.0 * mean.max(1.0e-12).log10()) as f32
}

fn sine(start_hz: f32, end_hz: f32, amp: f32) -> Vec<i16> {
    let rate = SAMPLE_RATE as f32;
    let mut pcm = Vec::with_capacity(FRAME_SAMPLES);
    let mut phase = 0.0f32;
    let last = FRAME_SAMPLES.saturating_sub(1).max(1) as f32;
    for index in 0..FRAME_SAMPLES {
        let mix = index as f32 / last;
        let hz = start_hz + (end_hz - start_hz) * mix;
        phase += std::f32::consts::TAU * hz / rate;
        let sample = (phase.sin() * amp).round();
        pcm.push(sample.clamp(f32::from(i16::MIN), f32::from(i16::MAX)) as i16);
    }
    pcm
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_is_below_the_ear_and_the_tone_is_above_it() {
        let quiet = silence();
        let voiced = tone(180.0);
        let fall = falling();
        assert_eq!(quiet.len(), FRAME_SAMPLES);
        assert_eq!(voiced.len(), FRAME_SAMPLES);
        assert!(rms_db(&quiet) < EAR_FLOOR_DB);
        assert!(rms_db(&voiced) > EAR_FLOOR_DB);
        assert!(rms_db(&fall) > EAR_FLOOR_DB);
        assert!(quiet.iter().all(|sample| *sample == 0));
    }
}
