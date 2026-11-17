// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Vowel-to-vowel blend across the phone string.
//!
//! Ohman 1966: a consonant does not block the vowels on either side.
//! The weight is a configured fraction in 0..=1. It is not a hertz locus
//! from that paper. Weight `None` means the size is unknown and no blend
//! is applied. Weight 0 is a configured choice of no blend: vowels keep
//! their own units and consonants stay unspecified.

use crate::phone::Units;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Blend {
    pub jaw: Option<f64>,
    pub front: Option<f64>,
    pub height: Option<f64>,
    pub lips: Option<f64>,
}

impl Blend {
    fn none() -> Self {
        Self {
            jaw: None,
            front: None,
            height: None,
            lips: None,
        }
    }

    fn from_units(u: Units) -> Self {
        Self {
            jaw: Some(u.jaw),
            front: Some(u.front),
            height: Some(u.height),
            lips: Some(u.lips),
        }
    }
}

pub fn blend(vowels: &[Option<Units>], weight: Option<f64>) -> Vec<Blend> {
    let Some(w) = weight else {
        return vec![Blend::none(); vowels.len()];
    };
    let w = w.clamp(0.0, 1.0);
    vowels
        .iter()
        .enumerate()
        .map(|(i, own)| {
            let left = (0..i).rev().find_map(|j| vowels[j]);
            let right = (i + 1..vowels.len()).find_map(|j| vowels[j]);
            match (*own, left, right) {
                (Some(self_u), l, r) => {
                    let neighbor = match (l, r) {
                        (Some(a), Some(b)) => avg(a, b),
                        (Some(a), None) => a,
                        (None, Some(b)) => b,
                        (None, None) => self_u,
                    };
                    Blend::from_units(mix(self_u, neighbor, w))
                }
                (None, _, _) if w == 0.0 => Blend::none(),
                (None, Some(a), Some(b)) => Blend::from_units(scale(avg(a, b), w)),
                (None, Some(a), None) => Blend::from_units(scale(a, w)),
                (None, None, Some(b)) => Blend::from_units(scale(b, w)),
                (None, None, None) => Blend::none(),
            }
        })
        .collect()
}

fn avg(a: Units, b: Units) -> Units {
    Units {
        jaw: 0.5 * (a.jaw + b.jaw),
        front: 0.5 * (a.front + b.front),
        height: 0.5 * (a.height + b.height),
        lips: 0.5 * (a.lips + b.lips),
    }
}

fn mix(a: Units, b: Units, w: f64) -> Units {
    Units {
        jaw: (1.0 - w) * a.jaw + w * b.jaw,
        front: (1.0 - w) * a.front + w * b.front,
        height: (1.0 - w) * a.height + w * b.height,
        lips: (1.0 - w) * a.lips + w * b.lips,
    }
}

fn scale(a: Units, w: f64) -> Units {
    Units {
        jaw: w * a.jaw,
        front: w * a.front,
        height: w * a.height,
        lips: w * a.lips,
    }
}
