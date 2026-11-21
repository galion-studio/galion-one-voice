// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! 80 ms frames at 24 kHz. The heartbeat is not called from this crate.
//! A player pulls whole frames. The tail is whatever samples remain.

/// Galion frame, milliseconds.
pub const FRAME_MS: u32 = 80;
/// Samantha output rate.
pub const SAMPLE_RATE_HZ: u32 = 24_000;
/// `SAMPLE_RATE_HZ * FRAME_MS / 1000`.
pub const FRAME_SAMPLES: usize = 1_920;

pub fn frames_of(samples: &[i16]) -> impl Iterator<Item = &[i16]> {
    samples.chunks_exact(FRAME_SAMPLES)
}

pub fn tail_of(samples: &[i16]) -> &[i16] {
    let n = samples.len() % FRAME_SAMPLES;
    if n == 0 {
        &[]
    } else {
        &samples[samples.len() - n..]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eighty_milliseconds_is_exactly_1920_samples() {
        assert_eq!(SAMPLE_RATE_HZ * FRAME_MS / 1000, FRAME_SAMPLES as u32);
        assert_eq!(FRAME_SAMPLES, 1_920);
    }

    #[test]
    fn frames_leave_a_short_tail() {
        let samples = vec![1i16; FRAME_SAMPLES * 2 + 10];
        assert_eq!(frames_of(&samples).count(), 2);
        assert_eq!(tail_of(&samples).len(), 10);
        assert!(tail_of(&vec![0i16; FRAME_SAMPLES]).is_empty());
    }
}
