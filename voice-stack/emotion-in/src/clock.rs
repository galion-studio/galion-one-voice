// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Heartbeat clock. Same quantum as `LOCKS.toml` `[clock]` and
//! `galion-emotion` `frame::live_frame`: 80 ms, 12.5 Hz, 24 kHz, 1920 samples.
//! This crate does not edit the lock.

/// Heartbeat period. Mood and prosody both step on this quantum.
pub const FRAME_MS: u32 = 80;
/// Analysis rate after any input resample. Matches the you/me/feeling bands.
pub const SAMPLE_RATE_HZ: u32 = 24_000;
/// `SAMPLE_RATE_HZ * FRAME_MS / 1000`.
pub const FRAME_SAMPLES: usize = (SAMPLE_RATE_HZ as usize * FRAME_MS as usize) / 1000;
/// 12.5 Hz in millihertz.
pub const FRAME_MHZ: u32 = 12_500;
/// Streaming bar for one stage, in microseconds. A measurement has to come
/// from a run; this constant is only the bar those runs are compared with.
pub const STAGE_BUDGET_US: u32 = 80_000;
/// Highest input rate the framer will hold. One input frame then fits in
/// [`MAX_INPUT_SAMPLES`].
pub const MAX_INPUT_RATE_HZ: u32 = 48_000;
/// Samples in one 80 ms frame at [`MAX_INPUT_RATE_HZ`].
pub const MAX_INPUT_SAMPLES: usize = (MAX_INPUT_RATE_HZ as usize * FRAME_MS as usize) / 1000;

/// Samples in one 80 ms frame at `sample_rate_hz`, when that lands on a sample.
pub fn samples_per_frame(sample_rate_hz: u32) -> Option<usize> {
    if sample_rate_hz == 0 || sample_rate_hz > MAX_INPUT_RATE_HZ {
        return None;
    }
    let ticks = u64::from(sample_rate_hz) * u64::from(FRAME_MS);
    if ticks % 1000 != 0 {
        None
    } else {
        Some((ticks / 1000) as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_voice_heartbeat() {
        assert_eq!(FRAME_MS, 80);
        assert_eq!(SAMPLE_RATE_HZ, 24_000);
        assert_eq!(FRAME_SAMPLES, 1920);
        assert_eq!(samples_per_frame(SAMPLE_RATE_HZ), Some(1920));
        assert_eq!(2 * 1000, 25 * FRAME_MS);
        assert_eq!(FRAME_MHZ, 12_500);
        assert_eq!(samples_per_frame(44_100), Some(3_528));
        assert_eq!(samples_per_frame(16_000), Some(1_280));
        assert_eq!(samples_per_frame(48_000), Some(3_840));
        assert_eq!(samples_per_frame(24_001), None);
        assert_eq!(samples_per_frame(96_000), None);
    }
}
