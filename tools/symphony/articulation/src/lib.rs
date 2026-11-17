// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Symphony articulation layer (throat / mouth / nose).
//!
//! Reused bricks, not re-implemented here:
//! - `praatfan_rust` (MIT, Rust, in process): pitch, Burg formants, HNR, intensity.
//! - SPARC (MIT, Python, out of process): 12-channel EMA articulator traces at 50 Hz.
//! - DisVoice (MIT, Python, out of process): glottal (IAIF/GCI) and phonation statics.
//! - Phonet (MIT, Python, out of process, via DisVoice): per-frame phonological posteriors.
//! - Rhubarb Lip Sync (MIT, C++, out of process): mouth shapes A-H, X.
//!
//! Our own glue: everything is put on the 80 ms Symphony tick (LOCKS clock.frame_ms),
//! summarised as quantiles, and compared with TractPrint (see [`score`]).
//! Voice-agnostic: no voice name or voice-specific constant appears in this crate.

pub mod acoustic;
pub mod score;
pub mod workers;

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Symphony heartbeat (LOCKS.toml clock.frame_ms = 80).
pub const TICK_S: f64 = 0.080;

/// Quantiles kept per feature (5..95 %).
pub const QUANTILES: [f64; 9] = [0.05, 0.10, 0.25, 0.40, 0.50, 0.60, 0.75, 0.90, 0.95];

/// Which part of the vocal system a feature describes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Section {
    Throat,
    Mouth,
    Nose,
}

/// A named per-tick track. `None` = not measured in that tick (unvoiced, silent, or no worker).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Track {
    pub section: Section,
    pub unit: String,
    pub source: String,
    pub ticks: Vec<Option<f64>>,
}

/// Quantile summary of one track (only measured ticks).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Summary {
    pub section: Section,
    pub unit: String,
    pub source: String,
    pub n: usize,
    pub mean: f64,
    pub q: Vec<f64>,
}

/// The `articulation` section of a stage-0 voice card.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Articulation {
    pub schema: u32,
    pub tick_ms: u32,
    pub duration_s: f64,
    pub n_ticks: usize,
    /// Per-tick tracks (the 80 ms data contract).
    pub tracks: BTreeMap<String, Track>,
    /// Per-tick mouth shape from Rhubarb ("A".."H", "X"), if run.
    pub visemes: Option<Vec<String>>,
    /// Whole-clip scalars (DisVoice static glottal / phonation, VTL estimate).
    pub statics: BTreeMap<String, (Section, f64)>,
    pub summary: BTreeMap<String, Summary>,
    /// Which bricks produced data, with licence, for the PR licence record.
    pub bricks: Vec<Brick>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Brick {
    pub name: String,
    pub version: String,
    pub licence: String,
    pub lang: String,
    pub process: String,
}

/// Put a (time_s, value) series on the 80 ms grid: median of finite values per tick.
pub fn to_ticks(times: &[f64], values: &[f64], n_ticks: usize) -> Vec<Option<f64>> {
    let mut bins: Vec<Vec<f64>> = vec![Vec::new(); n_ticks];
    for (t, v) in times.iter().zip(values) {
        if !v.is_finite() || *t < 0.0 {
            continue;
        }
        let i = (t / TICK_S).floor() as usize;
        if i < n_ticks {
            bins[i].push(*v);
        }
    }
    bins.into_iter().map(|mut b| median(&mut b)).collect()
}

/// Same, for a series sampled at a fixed rate starting at t=0 (e.g. SPARC 50 Hz).
pub fn rate_to_ticks(rate_hz: f64, values: &[f64], n_ticks: usize) -> Vec<Option<f64>> {
    let times: Vec<f64> = (0..values.len()).map(|i| i as f64 / rate_hz).collect();
    to_ticks(&times, values, n_ticks)
}

fn median(v: &mut [f64]) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = v.len();
    Some(if n % 2 == 1 { v[n / 2] } else { 0.5 * (v[n / 2 - 1] + v[n / 2]) })
}

/// Linear-interpolated quantile of a sorted slice.
pub fn quantile_sorted(s: &[f64], p: f64) -> f64 {
    if s.len() == 1 {
        return s[0];
    }
    let x = p * (s.len() - 1) as f64;
    let i = x.floor() as usize;
    let f = x - i as f64;
    if i + 1 >= s.len() {
        s[s.len() - 1]
    } else {
        s[i] * (1.0 - f) + s[i + 1] * f
    }
}

pub fn summarise(t: &Track) -> Option<Summary> {
    let mut v: Vec<f64> = t.ticks.iter().flatten().copied().filter(|x| x.is_finite()).collect();
    if v.is_empty() {
        return None;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    Some(Summary {
        section: t.section,
        unit: t.unit.clone(),
        source: t.source.clone(),
        n: v.len(),
        mean: v.iter().sum::<f64>() / v.len() as f64,
        q: QUANTILES.iter().map(|p| quantile_sorted(&v, *p)).collect(),
    })
}

impl Articulation {
    pub fn new(duration_s: f64) -> Self {
        let n_ticks = (duration_s / TICK_S).ceil().max(1.0) as usize;
        Articulation {
            schema: 1,
            tick_ms: 80,
            duration_s,
            n_ticks,
            tracks: BTreeMap::new(),
            visemes: None,
            statics: BTreeMap::new(),
            summary: BTreeMap::new(),
            bricks: Vec::new(),
        }
    }

    pub fn add(&mut self, name: &str, section: Section, unit: &str, source: &str, ticks: Vec<Option<f64>>) {
        let mut ticks = ticks;
        ticks.resize(self.n_ticks, None);
        self.tracks.insert(
            name.to_string(),
            Track { section, unit: unit.into(), source: source.into(), ticks },
        );
    }

    pub fn finish(&mut self) {
        self.summary = self
            .tracks
            .iter()
            .filter_map(|(k, t)| summarise(t).map(|s| (k.clone(), s)))
            .collect();
    }
}
