// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Energy voice activity, the fallback in `voice-stack/vad`.
//!
//! `galion-sam-vad` links ONNX Runtime and declares Rust 1.92, so this smoke
//! does not take that crate as a dependency. The floor, the +12 dB margin,
//! and the −48 dB clamp are the same numbers as `voice-stack/vad/src/energy.rs`.
//! Silero is not linked and is not reported as a measured backend.

/// 20 ms at 16 kHz. Same hop as `galion-sam-vad`.
pub const FRAME_SAMPLES: usize = 320;

#[derive(Debug, Clone)]
pub struct Energy {
    floor_db: f64,
    on_db: f64,
}

impl Default for Energy {
    fn default() -> Self {
        Self {
            floor_db: -60.0,
            on_db: 12.0,
        }
    }
}

impl Energy {
    /// `true` when the frame sits above the adaptive floor.
    pub fn push_i16(&mut self, frame: &[i16]) -> bool {
        let db = rms_db(frame);
        let voiced = db > (self.floor_db + self.on_db).max(-48.0);
        if !voiced {
            self.floor_db = 0.95 * self.floor_db + 0.05 * db.max(-90.0);
        }
        voiced
    }
}

pub fn rms_db(samples: &[i16]) -> f64 {
    if samples.is_empty() {
        return -120.0;
    }
    let energy = samples
        .iter()
        .map(|sample| {
            let unit = f64::from(*sample) / 32768.0;
            unit * unit
        })
        .sum::<f64>()
        / samples.len() as f64;
    10.0 * energy.max(1e-12).log10()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameCounts {
    pub voiced: u32,
    pub quiet: u32,
}

/// Count 20 ms frames. A short tail is quiet and is not scored.
pub fn count_frames(pcm: &[i16]) -> FrameCounts {
    let mut energy = Energy::default();
    let mut counts = FrameCounts {
        voiced: 0,
        quiet: 0,
    };
    for frame in pcm.chunks_exact(FRAME_SAMPLES) {
        if energy.push_i16(frame) {
            counts.voiced = counts.voiced.saturating_add(1);
        } else {
            counts.quiet = counts.quiet.saturating_add(1);
        }
    }
    counts
}

/// An 80 ms duplex frame is voiced when any of its 20 ms parts is voiced.
pub fn eighty_ms_voiced(energy: &mut Energy, frame: &[i16]) -> bool {
    frame
        .chunks_exact(FRAME_SAMPLES)
        .any(|part| energy.push_i16(part))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(n: usize, amp: i16) -> Vec<i16> {
        (0..n)
            .map(|i| ((i as f64 * 0.4).sin() * f64::from(amp)) as i16)
            .collect()
    }

    #[test]
    fn a_loud_tone_is_voice_and_silence_is_not() {
        let mut energy = Energy::default();
        assert!(energy.push_i16(&tone(FRAME_SAMPLES, 8000)));
        assert!(!energy.push_i16(&vec![0i16; FRAME_SAMPLES]));
        let counts = count_frames(&tone(FRAME_SAMPLES * 2, 8000));
        assert_eq!(counts.voiced, 2);
        assert_eq!(counts.quiet, 0);
    }
}
