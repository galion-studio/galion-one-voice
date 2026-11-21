// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Linear map from normalised prosody to a PAD estimate, plus one named
//! post-rule. Coefficients are the model. Nothing here was fit on a corpus.
//!
//! Anchors are reference points on this scale, not population norms.
//! `n = clamp((x - lo) / (hi - lo), 0, 1)`.
//!
//! ```text
//! y = x W + b
//! x = [energy, f0, rate, f0_range, pause, zcr]
//! y = [arousal, valence, dominance]
//! ```
//!
//! `anger_press` then runs only when the window is loud, high, and flat:
//! valence loses up to 0.50 and dominance gains up to 0.12. A pure sine has
//! almost no pitch range, so a loud high sine takes this path. A wide pitch
//! range does not.
//!
//! Confidence stays at most 0.70. Prosody is not a full emotion classifier.
//! Below 0.20 the mood target is [`Pad::RESTING`], the same cutoff
//! `galion-emotion` uses before it will name an affect.
//!
//! The optional `onnx` feature loads these same weights from
//! `models/prosody-linear.onnx` (one Gemm). The post-rule stays in Rust.

use crate::pad::Pad;
use crate::prosody::Prosody;

pub const ENERGY_LO: f32 = 0.015;
pub const ENERGY_HI: f32 = 0.280;
pub const F0_LO: f32 = 85.0;
pub const F0_HI: f32 = 280.0;
pub const RATE_LO: f32 = 1.5;
pub const RATE_HI: f32 = 6.5;
pub const RANGE_LO: f32 = 8.0;
pub const RANGE_HI: f32 = 90.0;
pub const PAUSE_LO: f32 = 0.0;
pub const PAUSE_HI: f32 = 0.60;
pub const ZCR_LO: f32 = 0.02;
pub const ZCR_HI: f32 = 0.20;

/// Row-major. Row = feature, column = arousal, valence, dominance.
pub const W: [[f32; 3]; 6] = [
    [0.50, 0.05, 0.40],
    [0.22, 0.10, 0.05],
    [0.22, 0.12, 0.15],
    [0.08, 0.55, 0.05],
    [-0.28, -0.70, -0.30],
    [0.00, 0.25, 0.00],
];
pub const B: [f32; 3] = [0.05, -0.05, 0.25];

pub const FEATURES: [&str; 6] = ["energy", "f0", "rate", "f0_range", "pause", "zcr"];

pub const NOTES: [&str; 6] = [
    "mean RMS of voiced 10 ms hops; louder raises arousal and dominance",
    "median voiced F0; higher raises arousal",
    "syllable peaks per speaking second (pauses of 200 ms or more removed); faster raises arousal",
    "F0 p90 minus p10; wider raises valence",
    "share of the window spent in pauses of 200 ms or more; more lowers valence, arousal, and dominance",
    "zero-crossing rate of voiced frames, a brightness proxy; higher raises valence slightly",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Classifier {
    Linear,
    OnnxGemm,
}

impl Classifier {
    pub fn as_str(self) -> &'static str {
        match self {
            Classifier::Linear => "linear",
            Classifier::OnnxGemm => "onnx-gemm",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mapped {
    pub estimate: Pad,
    pub target: Pad,
    pub confidence: f32,
    pub norm: [f32; 6],
    pub anger_press: f32,
    pub classifier: Classifier,
}

impl Mapped {
    pub const REST: Mapped = Mapped {
        estimate: Pad::RESTING,
        target: Pad::RESTING,
        confidence: 0.0,
        norm: [0.0; 6],
        anger_press: 0.0,
        classifier: Classifier::Linear,
    };
}

fn norm(x: f32, lo: f32, hi: f32) -> f32 {
    if !x.is_finite() || hi <= lo {
        return 0.0;
    }
    ((x - lo) / (hi - lo)).clamp(0.0, 1.0)
}

pub fn features_of(p: &Prosody) -> [f32; 6] {
    [
        norm(p.rms, ENERGY_LO, ENERGY_HI),
        if p.voiced_frames >= 4 {
            norm(p.f0_hz, F0_LO, F0_HI)
        } else {
            0.0
        },
        norm(p.articulation_hz, RATE_LO, RATE_HI),
        if p.voiced_frames >= 4 {
            norm(p.f0_range_hz, RANGE_LO, RANGE_HI)
        } else {
            0.0
        },
        norm(p.pause_ratio, PAUSE_LO, PAUSE_HI),
        norm(p.zcr, ZCR_LO, ZCR_HI),
    ]
}

pub fn gemm(x: &[f32; 6]) -> [f32; 3] {
    let mut y = B;
    let mut i = 0;
    while i < 6 {
        let mut j = 0;
        while j < 3 {
            y[j] += x[i] * W[i][j];
            j += 1;
        }
        i += 1;
    }
    y
}

/// Loud, high, and flat. 0 when any leg is outside its band.
pub fn anger_press(n_energy: f32, n_range: f32, n_f0: f32) -> f32 {
    if n_energy <= 0.65 || n_range >= 0.30 || n_f0 <= 0.45 {
        return 0.0;
    }
    let e = (n_energy - 0.65) / 0.35;
    let r = (0.30 - n_range) / 0.30;
    let f = (n_f0 - 0.45) / 0.55;
    (e * r * f).clamp(0.0, 1.0)
}

pub fn confidence(p: &Prosody) -> f32 {
    if p.voiced_frames < 4 || p.voiced_ratio < 0.08 {
        return 0.0;
    }
    let c = 0.20 + 0.40 * p.voiced_ratio + 0.25 * p.harmonicity.clamp(0.0, 1.0);
    c.clamp(0.0, 0.70)
}

fn pad_from_y(y: [f32; 3], press: f32) -> Pad {
    Pad {
        arousal: y[0],
        valence: y[1] - 0.50 * press,
        dominance: y[2] + 0.12 * press,
    }
    .clamped()
}

/// `linear_only` forces the in-tree coefficients. The ONNX feature otherwise
/// runs the same Gemm from the model file and falls back to these coefficients
/// if that file does not match the graph this crate accepts.
pub fn map_prosody(p: &Prosody, linear_only: bool) -> Mapped {
    let x = features_of(p);
    let press = anger_press(x[0], x[3], x[1]);
    let (y, classifier) = affine(&x, linear_only);
    let estimate = pad_from_y(y, press);
    let confidence = confidence(p);
    let target = if confidence < 0.20 {
        Pad::RESTING
    } else {
        estimate
    };
    Mapped {
        estimate,
        target,
        confidence,
        norm: x,
        anger_press: press,
        classifier,
    }
}

fn affine(x: &[f32; 6], linear_only: bool) -> ([f32; 3], Classifier) {
    if linear_only {
        return (gemm(x), Classifier::Linear);
    }
    #[cfg(feature = "onnx")]
    {
        match crate::onnx::affine(x) {
            Ok(y) => return (y, Classifier::OnnxGemm),
            Err(_) => return (gemm(x), Classifier::Linear),
        }
    }
    #[cfg(not(feature = "onnx"))]
    {
        let _ = linear_only;
        (gemm(x), Classifier::Linear)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_energy_matches_the_first_row() {
        let x = [1.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let y = gemm(&x);
        assert!((y[0] - 0.55).abs() < 1e-5, "{y:?}");
        assert!((y[1] - 0.0).abs() < 1e-5, "{y:?}");
        assert!((y[2] - 0.65).abs() < 1e-5, "{y:?}");
    }

    #[test]
    fn anchors_saturate() {
        assert_eq!(norm(-1.0, 0.0, 1.0), 0.0);
        assert_eq!(norm(0.5, 0.0, 1.0), 0.5);
        assert_eq!(norm(2.0, 0.0, 1.0), 1.0);
        assert_eq!(norm(f32::NAN, 0.0, 1.0), 0.0);
    }

    #[test]
    fn low_confidence_target_is_resting() {
        let m = map_prosody(&Prosody::EMPTY, true);
        assert_eq!(m.confidence, 0.0);
        assert_eq!(m.target, Pad::RESTING);
        assert_eq!(m.classifier, Classifier::Linear);
    }

    #[test]
    fn press_requires_loud_high_and_flat() {
        assert!(anger_press(1.0, 0.0, 1.0) > 0.99);
        assert!(anger_press(0.9, 0.0, 0.8) > 0.4);
        assert_eq!(anger_press(0.9, 0.5, 0.8), 0.0);
        assert_eq!(anger_press(0.4, 0.0, 0.8), 0.0);
        assert_eq!(anger_press(0.9, 0.0, 0.2), 0.0);
    }
}
