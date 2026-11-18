// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Room, distance and a noise bed, taken from the moment fixture.
//!
//! Room gain and distance gain are the pipeline defaults from draft PR #194
//! (`OuterFactors::distance_gain`, `Room::gain`). They are not a cited
//! distance-to-decibel curve. The spec leaves that function unknown.
//!
//! `noise_db` above zero is dB above the fixture's quiet mark, not dB SPL.
//! The bed is a deterministic hash noise whose level never exceeds −24 dBFS.
//! Garnier, Henrich and Dubois (2010) measured Lombard loudness in dB SPL.
//! That slope is not copied here.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Room {
    Quiet,
    Small,
    Large,
    Outdoor,
}

impl Default for Room {
    fn default() -> Self {
        Self::Quiet
    }
}

impl Room {
    pub fn gain(self) -> f64 {
        match self {
            Self::Quiet | Self::Small => 1.0,
            Self::Large => 0.85,
            Self::Outdoor => 0.70,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Environment {
    #[serde(default)]
    pub room: Room,
    /// 0 is quiet: no bed is added. A positive number raises the bed.
    #[serde(default)]
    pub noise_db: f64,
    #[serde(default = "one_metre")]
    pub listener_distance_m: f64,
    #[serde(default = "one_metre")]
    pub baseline_distance_m: f64,
}

fn one_metre() -> f64 {
    1.0
}

impl Default for Environment {
    fn default() -> Self {
        Self {
            room: Room::Quiet,
            noise_db: 0.0,
            listener_distance_m: 1.0,
            baseline_distance_m: 1.0,
        }
    }
}

impl Environment {
    pub fn distance_gain(self) -> f64 {
        let ref_d = self.baseline_distance_m.max(0.3);
        let d = self.listener_distance_m.max(0.3);
        (ref_d / d).clamp(0.4, 1.8)
    }

    pub fn voice_gain(self) -> f64 {
        self.distance_gain() * self.room.gain()
    }

    /// RMS of the added bed. 0 when `noise_db` is 0 or negative.
    pub fn noise_rms(self) -> f64 {
        if !self.noise_db.is_finite() || self.noise_db <= 0.0 {
            return 0.0;
        }
        let dbfs = (-60.0 + self.noise_db).clamp(-80.0, -24.0);
        10f64.powf(dbfs / 20.0)
    }
}

/// A vocal burst from the nose layer (agent S5). This stage places the
/// samples. It does not synthesise the category.
///
/// TODO(S5): the nose crate should own `samples`. `synthetic_burst` exists
/// so the mixer can be tested before that crate is in the tree.
#[derive(Clone, Debug, PartialEq)]
pub struct Burst {
    pub category: String,
    pub start_s: f64,
    pub samples: Vec<f64>,
    pub sample_rate: f64,
}

/// Short deterministic noise. Not a laugh, not a sigh, not a recording.
pub fn synthetic_burst(category: &str, start_s: f64, secs: f64) -> Burst {
    let n = (secs * crate::SAMPLE_RATE as f64).round().max(1.0) as usize;
    let mut samples = Vec::with_capacity(n);
    for i in 0..n {
        let env = (std::f64::consts::PI * i as f64 / n as f64).sin();
        samples.push(hash_unit(10_000 + i) * 0.25 * env);
    }
    Burst {
        category: category.to_string(),
        start_s,
        samples,
        sample_rate: crate::SAMPLE_RATE as f64,
    }
}

pub fn hash_unit(index: usize) -> f64 {
    let mut x = 0x9E37_79B9u32
        .wrapping_add(index as u32)
        .wrapping_mul(0x045d_9f3b);
    x ^= x >> 16;
    (x as f64) / (u32::MAX as f64) * 2.0 - 1.0
}

pub fn burst_at(bursts: &[Burst], index: usize, sr: f64) -> f64 {
    let mut acc = 0.0;
    for burst in bursts {
        if !(burst.sample_rate.is_finite() && burst.sample_rate > 0.0) {
            continue;
        }
        let start = (burst.start_s * sr).round();
        if !start.is_finite() {
            continue;
        }
        let start = start.max(0.0) as usize;
        if index < start {
            continue;
        }
        let pos = (index - start) as f64 * burst.sample_rate / sr;
        if pos < 0.0 || pos + 1.0 >= burst.samples.len() as f64 && pos >= burst.samples.len() as f64
        {
            continue;
        }
        acc += interp(&burst.samples, pos);
    }
    acc
}

fn interp(samples: &[f64], pos: f64) -> f64 {
    if samples.is_empty() || !pos.is_finite() || pos < 0.0 {
        return 0.0;
    }
    let i0 = pos.floor() as usize;
    if i0 >= samples.len() {
        return 0.0;
    }
    let frac = pos - i0 as f64;
    let a = samples[i0];
    let b = samples.get(i0 + 1).copied().unwrap_or(a);
    a + (b - a) * frac
}

pub fn wet_sample(dry: f64, index: usize, env: Environment, bursts: &[Burst], sr: f64) -> f64 {
    let gain = env.voice_gain();
    let noise_amp = env.noise_rms() * 3f64.sqrt();
    let noise = hash_unit(index) * noise_amp;
    let burst = burst_at(bursts, index, sr);
    ((dry + burst) * gain + noise).clamp(-1.0, 1.0)
}
