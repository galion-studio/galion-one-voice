// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! Universal per-emotion scores. A projection of the moment, not the state.
//!
//! The twelve ids and the valence/arousal pairs are the design coordinates
//! in `emotion-v2/emotions.toml`, in the order `tools/emotion-v2/harness/emotions.py`
//! lists `EMOTIONS`. Arousal there is bipolar (−1..1). The moment stores
//! wire arousal (0..1). This file converts. It does not pick a winner, and
//! `EmotionCore::step` does not call it.

use crate::axes::{AROUSAL, VALENCE};
use crate::moment::MomentState;

pub const UNIVERSAL_LEN: usize = 12;

/// Gaussian width on the bipolar plane. A stand-in, not a fitted covariance.
pub const PROJECTION_SIGMA: f32 = 0.55;

pub struct UniversalPole {
    pub id: &'static str,
    pub valence: f32,
    /// Design arousal on −1..1, as written in `emotions.toml`.
    pub arousal_bipolar: f32,
}

pub const UNIVERSAL: [UniversalPole; UNIVERSAL_LEN] = [
    UniversalPole {
        id: "neutral",
        valence: 0.0,
        arousal_bipolar: 0.0,
    },
    UniversalPole {
        id: "calm",
        valence: 0.72,
        arousal_bipolar: -0.69,
    },
    UniversalPole {
        id: "happy",
        valence: 0.99,
        arousal_bipolar: 0.14,
    },
    UniversalPole {
        id: "amused",
        valence: 0.74,
        arousal_bipolar: 0.42,
    },
    UniversalPole {
        id: "excited",
        valence: 0.66,
        arousal_bipolar: 0.75,
    },
    UniversalPole {
        id: "tender",
        valence: 0.58,
        arousal_bipolar: -0.18,
    },
    UniversalPole {
        id: "sad",
        valence: -0.89,
        arousal_bipolar: -0.46,
    },
    UniversalPole {
        id: "disappointed",
        valence: -0.52,
        arousal_bipolar: -0.12,
    },
    UniversalPole {
        id: "angry",
        valence: -0.68,
        arousal_bipolar: 0.55,
    },
    UniversalPole {
        id: "fearful",
        valence: -0.42,
        arousal_bipolar: 0.78,
    },
    UniversalPole {
        id: "surprised",
        valence: -0.08,
        arousal_bipolar: 0.88,
    },
    UniversalPole {
        id: "disgusted",
        valence: -0.74,
        arousal_bipolar: 0.18,
    },
];

/// Wire arousal 0..1 to the bipolar axis used by the emotion-v2 inventory.
pub fn arousal_bipolar(wire_arousal: f32) -> f32 {
    wire_arousal * 2.0 - 1.0
}

/// Independent scores in (0, 1]. Not a distribution. They do not sum to 1.
pub fn project_universal(moment: &MomentState) -> [f32; UNIVERSAL_LEN] {
    let valence = moment.pos[VALENCE];
    let arousal = arousal_bipolar(moment.pos[AROUSAL]);
    let denom = 2.0 * PROJECTION_SIGMA * PROJECTION_SIGMA;
    let mut scores = [0.0; UNIVERSAL_LEN];
    for (i, pole) in UNIVERSAL.iter().enumerate() {
        let dv = valence - pole.valence;
        let da = arousal - pole.arousal_bipolar;
        let d2 = dv * dv + da * da;
        scores[i] = (-d2 / denom).exp();
    }
    scores
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::axes::{AROUSAL, DOMINANCE, VALENCE};

    #[test]
    fn origin_scores_neutral_at_one_and_is_not_a_simplex() {
        let moment = MomentState {
            t_ms: 0,
            tick: 0,
            pos: [0.0, 0.5, 0.5, 0.0],
            vel: [0.0; 4],
        };
        assert!((arousal_bipolar(moment.pos[AROUSAL])).abs() < 1e-6);
        let scores = project_universal(&moment);
        assert!((scores[0] - 1.0).abs() < 1e-5);
        let sum: f32 = scores.iter().sum();
        assert!(sum > 1.2, "sum {sum} collapsed toward a category");
        assert!(scores.iter().skip(1).any(|s| *s > 0.05));
    }

    #[test]
    fn dominance_is_not_part_of_the_projection() {
        let mut low = MomentState::at([0.1, 0.4, 0.2, -0.3]);
        low.pos[VALENCE] = 0.1;
        low.pos[AROUSAL] = 0.4;
        low.pos[DOMINANCE] = 0.2;
        let mut high = low;
        high.pos[DOMINANCE] = 0.9;
        let a = project_universal(&low);
        let b = project_universal(&high);
        for i in 0..UNIVERSAL_LEN {
            assert_eq!(a[i].to_bits(), b[i].to_bits());
        }
    }
}
