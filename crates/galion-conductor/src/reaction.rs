// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! Wordless reaction. Generated in memory. Silence, or one sine.
//!
//! Nothing here reads a file. Nothing here is a recording of a person.
//! One buffer is one 80 ms frame at 24 kHz: 1920 samples, the same size
//! as the heartbeat frame. This crate does not call that step.

use crate::clock::FRAME_MS;

/// 24 kHz, the you/me rate already used by the heartbeat frame.
pub const REACTION_RATE: u32 = 24_000;

/// Samples in one reaction. `REACTION_RATE * FRAME_MS / 1000`.
pub const REACTION_SAMPLES: usize = (REACTION_RATE as usize * FRAME_MS as usize) / 1000;

const _: () = assert!(REACTION_SAMPLES == 1_920);

/// Which buffer `tick` hands out on eager end-of-turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReactionKind {
    Silence,
    Synthetic,
}

/// A wordless buffer. `peak` is the largest absolute sample.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reaction {
    pub kind: ReactionKind,
    pub samples: usize,
    pub peak: i16,
}

impl Reaction {
    pub fn from_kind(kind: ReactionKind) -> Self {
        let pcm = clip(kind);
        let peak = pcm.iter().fold(0i16, |acc, sample| acc.max(sample.abs()));
        Self {
            kind,
            samples: pcm.len(),
            peak,
        }
    }

    pub fn pcm(&self) -> Vec<i16> {
        clip(self.kind)
    }
}

pub fn clip(kind: ReactionKind) -> Vec<i16> {
    match kind {
        ReactionKind::Silence => vec![0; REACTION_SAMPLES],
        ReactionKind::Synthetic => synthetic_clip(),
    }
}

/// 220 Hz sine, amplitude 1000. Deterministic. Not a voice.
pub fn synthetic_clip() -> Vec<i16> {
    const HZ: f64 = 220.0;
    const AMP: f64 = 1000.0;
    let rate = f64::from(REACTION_RATE);
    let mut out = Vec::with_capacity(REACTION_SAMPLES);
    for n in 0..REACTION_SAMPLES {
        let phase = 2.0 * std::f64::consts::PI * HZ * (n as f64) / rate;
        let sample = (AMP * phase.sin()).round();
        out.push(sample as i16);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_is_one_frame_of_zeros_and_the_sine_is_bounded() {
        let silence = clip(ReactionKind::Silence);
        assert_eq!(silence.len(), 1920);
        assert!(silence.iter().all(|sample| *sample == 0));

        let tone = synthetic_clip();
        assert_eq!(tone.len(), 1920);
        assert!(tone.iter().any(|sample| *sample != 0));
        assert!(tone.iter().all(|sample| sample.abs() <= 1000));
        let reaction = Reaction::from_kind(ReactionKind::Synthetic);
        assert_eq!(reaction.pcm(), tone);
        assert!(reaction.peak > 0);
        assert!(reaction.peak <= 1000);
    }
}
