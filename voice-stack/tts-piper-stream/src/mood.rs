// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Pad -> PCM. Piper reads length, noise, and sentence silence once, at spawn.
//! Changing them would kill the warm process. Pitch, gain, and a leading pause
//! are applied to the samples, the same knobs as `galion-ml-voice` `EmotiveShaper`
//! (ratio `2^(st/12)`, cubic knee above 0.8 full scale). Resting Pad is a copy.

use crate::tts::Pad;

/// PCM-side mood for one utterance. Not a line of text.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Prosody {
    pub pitch_st: f32,
    pub energy: f32,
    pub pre_pause_ms: u32,
}

impl Prosody {
    pub fn from_pad(pad: Pad) -> Self {
        let v = finite(pad.valence, 0.25);
        let a = finite(pad.arousal, 0.3);
        let d = finite(pad.dominance, 0.5);
        // Resting valence 0.25 -> 0 st. Full swing stays inside Samantha's ±3 st.
        let pitch_st = ((v - 0.25) * 4.0).clamp(-3.0, 3.0);
        // Resting arousal 0.3 -> gain 1.
        let energy = (1.0 + (a - 0.3) * 0.8).clamp(0.25, 1.6);
        let mut pause = if v < 0.15 {
            (((0.15 - v) / 0.15) * 250.0).round() as u32
        } else {
            0
        };
        if d < 0.5 {
            pause = pause.saturating_add(((0.5 - d) * 80.0).round() as u32);
        }
        Self {
            pitch_st,
            energy,
            pre_pause_ms: pause.min(250),
        }
    }

    pub fn is_identity(self) -> bool {
        self.pitch_st.abs() < 1.0e-3 && (self.energy - 1.0).abs() < 1.0e-3 && self.pre_pause_ms == 0
    }

    fn ratio(self) -> f32 {
        (self.pitch_st.clamp(-6.0, 6.0) / 12.0).exp2()
    }
}

fn finite(x: f32, rest: f32) -> f32 {
    if x.is_finite() {
        x.clamp(0.0, 1.0)
    } else {
        rest
    }
}

fn knee(x: f32, gain: f32) -> i16 {
    let y = x * gain.clamp(0.25, 2.0) / 32768.0;
    let a = y.abs();
    let s = if a <= 0.8 {
        y
    } else {
        let t = ((a - 0.8) / 0.6).min(1.0);
        (0.8 + 0.18 * (1.5 * t - 0.5 * t * t * t)) * y.signum()
    };
    (s * 32767.0).round().clamp(-32768.0, 32767.0) as i16
}

/// Shape one clause. `leading_pause` is true only for the first clause.
pub fn shape_pcm(input: &[i16], prosody: Prosody, rate: u32, leading_pause: bool) -> Vec<i16> {
    let mut body = if prosody.is_identity() || input.is_empty() {
        input.to_vec()
    } else {
        apply_pitch_gain(input, prosody)
    };
    if leading_pause && prosody.pre_pause_ms > 0 && rate > 0 {
        let n = ((u64::from(prosody.pre_pause_ms) * u64::from(rate)) / 1000) as usize;
        let mut out = vec![0i16; n];
        out.append(&mut body);
        out
    } else {
        body
    }
}

fn apply_pitch_gain(input: &[i16], prosody: Prosody) -> Vec<i16> {
    let ratio = prosody.ratio();
    if (ratio - 1.0).abs() < 1.0e-6 {
        return input
            .iter()
            .map(|&s| knee(s as f32, prosody.energy))
            .collect();
    }
    let mut out = Vec::new();
    let mut pos = 0.0f64;
    let last = input.len() - 1;
    while pos < input.len() as f64 {
        let i = pos.floor() as usize;
        let frac = (pos - i as f64) as f32;
        let a = input[i.min(last)] as f32;
        let b = input[(i + 1).min(last)] as f32;
        out.push(knee(a + (b - a) * frac, prosody.energy));
        pos += f64::from(ratio);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resting_pad_is_a_copy() {
        let p = Prosody::from_pad(Pad::RESTING);
        assert!(p.is_identity(), "{p:?}");
        let input = [0, 100, -100, 32767, -32768];
        assert_eq!(shape_pcm(&input, p, 22_050, true), input);
        let messy = Pad {
            arousal: f32::NAN,
            valence: f32::NAN,
            dominance: f32::NAN,
        };
        assert!(Prosody::from_pad(messy).is_identity());
    }

    #[test]
    fn valence_moves_pitch_and_low_valence_pauses() {
        let high = Prosody::from_pad(Pad {
            arousal: 0.3,
            valence: 1.0,
            dominance: 0.5,
        });
        assert!((high.pitch_st - 3.0).abs() < 1.0e-4);
        assert_eq!(high.pre_pause_ms, 0);
        let low = Prosody::from_pad(Pad {
            arousal: 0.3,
            valence: 0.0,
            dominance: 0.5,
        });
        assert!((low.pitch_st + 1.0).abs() < 1.0e-4);
        assert_eq!(low.pre_pause_ms, 250);
        let n = shape_pcm(&[1000, 1000], low, 22_050, true);
        assert_eq!(
            n.len(),
            5512 + shape_pcm(&[1000, 1000], low, 22_050, false).len()
        );
        assert!(n[..5512].iter().all(|&s| s == 0));
    }

    #[test]
    fn arousal_moves_gain() {
        let loud = Prosody::from_pad(Pad {
            arousal: 1.0,
            valence: 0.25,
            dominance: 0.5,
        });
        assert!(loud.energy > 1.2);
        let quiet = shape_pcm(
            &[1000],
            Prosody::from_pad(Pad {
                arousal: 0.0,
                valence: 0.25,
                dominance: 0.5,
            }),
            22_050,
            false,
        );
        let base = shape_pcm(&[1000], Prosody::from_pad(Pad::RESTING), 22_050, false);
        assert!(quiet[0] < base[0]);
    }
}
