// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Energy gate used when the Silero model is missing or will not load.
//! The floor, the +12 dB margin, and the -48 dB clamp are the same numbers as
//! `archive/2026-11-24/crates/galion-v2-live/src/vad.rs`.

use crate::audio;

#[derive(Debug, Clone)]
pub struct Energy {
    /// Adaptive noise floor, dBFS. Starts at -60, same as the live VAD.
    pub floor_db: f64,
    /// How far above the floor a frame must sit to count as voice.
    pub on_db: f64,
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
    pub fn push_i16(&mut self, frame: &[i16]) -> f32 {
        let db = audio::rms_db(frame);
        let voiced = db > (self.floor_db + self.on_db).max(-48.0);
        if !voiced {
            self.floor_db = 0.95 * self.floor_db + 0.05 * db.max(-90.0);
        }
        if voiced {
            1.0
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Energy;

    fn tone(n: usize, amp: i16) -> Vec<i16> {
        (0..n)
            .map(|i| ((i as f64 * 0.4).sin() * amp as f64) as i16)
            .collect()
    }

    #[test]
    fn loud_tone_is_voice_and_does_not_lift_the_floor() {
        let mut e = Energy::default();
        let p = e.push_i16(&tone(320, 8000));
        assert_eq!(p, 1.0);
        assert!((e.floor_db - -60.0).abs() < 1e-9);
    }

    #[test]
    fn digital_silence_is_not_voice_and_the_floor_drops() {
        let mut e = Energy::default();
        let p = e.push_i16(&[0i16; 320]);
        assert_eq!(p, 0.0);
        assert!(e.floor_db < -60.0);
    }

    #[test]
    fn quiet_noise_stays_under_the_minus_48_clamp() {
        let mut e = Energy::default();
        let p = e.push_i16(&tone(320, 40));
        assert_eq!(p, 0.0);
    }
}
