// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! PCM conversions shared by capture, playback, and WAV.
//!
//! i16 → f32 divides by 32768 so a full-scale negative sample is -1.
//! f32 → i16 multiplies by 32767 so the result fits in i16. Non-finite
//! samples become silence.

/// f32 in roughly [-1, 1] to PCM16.
pub fn f32_to_i16(sample: f32) -> i16 {
    if !sample.is_finite() {
        return 0;
    }
    (sample.clamp(-1.0, 1.0) * 32_767.0).round() as i16
}

/// PCM16 to f32 in [-1, 1].
pub fn i16_to_f32(sample: i16) -> f32 {
    sample as f32 / 32_768.0
}

pub(crate) fn downmix_i16(interleaved: &[i16], channels: usize) -> Vec<i16> {
    if channels <= 1 {
        return interleaved.to_vec();
    }
    interleaved
        .chunks_exact(channels)
        .map(|frame| {
            let sum: i32 = frame.iter().map(|&s| i32::from(s)).sum();
            (sum / channels as i32) as i16
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_scale_round_trip_stays_in_range() {
        assert_eq!(f32_to_i16(1.0), 32_767);
        assert_eq!(f32_to_i16(-1.0), -32_767);
        assert_eq!(f32_to_i16(f32::NAN), 0);
        assert!(i16_to_f32(i16::MIN) >= -1.0);
        assert!(i16_to_f32(i16::MAX) <= 1.0);
    }

    #[test]
    fn stereo_downmix_averages_the_pair() {
        let mono = downmix_i16(&[100, 300, -100, -300], 2);
        assert_eq!(mono, vec![200, -200]);
    }
}
