// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! TractPrint: our own reference-vs-clone articulation match score.
//!
//! Idea (new, not from an upstream brick): describe a voice by the *distribution* of
//! each throat / mouth / nose track over 80 ms ticks, and compare two voices with a
//! quantile-function distance (Wasserstein-1 over the 5..95 % quantiles), scaled by
//! the reference's own spread (IQR). It needs no time alignment and no same text, so a
//! clone saying new words can still be checked against the reference's articulation.
//! Per-feature similarity = 1 / (1 + W1 / IQR_ref); a section is the mean of its
//! features; the total is the geometric mean of the sections, so one badly-matched
//! part of the vocal system (e.g. no nasal resonance) cannot hide behind the others.

use crate::{Articulation, Section};
use serde::Serialize;
use std::collections::BTreeMap;

/// Tracks that duplicate another one on a different scale are not scored twice.
const SKIP: [&str; 1] = ["f0_hz"];

/// Skewness and kurtosis of a few seconds of glottal cycles are too unstable to
/// compare (their sign flips between takes of the same voice), so only the
/// mean and spread functionals of the static glottal / phonation sets are scored.
fn static_scored(name: &str) -> bool {
    !(name.contains("kurtosis") || name.contains("skewness"))
}

#[derive(Debug, Serialize)]
pub struct FeatureScore {
    pub name: String,
    pub section: Section,
    pub kind: &'static str,
    pub sim: f64,
    pub ref_median: Option<f64>,
    pub clone_median: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct TractScore {
    pub schema: u32,
    pub method: &'static str,
    pub total: f64,
    pub sections: BTreeMap<String, f64>,
    pub n_features: usize,
    pub worst: Vec<FeatureScore>,
    pub features: Vec<FeatureScore>,
}

fn sec_name(s: Section) -> &'static str {
    match s {
        Section::Throat => "throat",
        Section::Mouth => "mouth",
        Section::Nose => "nose",
    }
}

pub fn compare(r: &Articulation, c: &Articulation) -> TractScore {
    let mut feats = Vec::new();
    for (k, rs) in &r.summary {
        if SKIP.contains(&k.as_str()) {
            continue;
        }
        let Some(cs) = c.summary.get(k) else { continue };
        if rs.n < 3 || cs.n < 3 {
            continue;
        }
        let w: f64 = rs.q.iter().zip(&cs.q).map(|(a, b)| (a - b).abs()).sum::<f64>() / rs.q.len() as f64;
        let iqr = rs.q[6] - rs.q[2];
        let scale = iqr.max(0.05 * rs.q[4].abs()).max(1e-6);
        feats.push(FeatureScore {
            name: k.clone(),
            section: rs.section,
            kind: "distribution",
            sim: 1.0 / (1.0 + w / scale),
            ref_median: Some(rs.q[4]),
            clone_median: Some(cs.q[4]),
        });
    }
    for (k, (sec, a)) in r.statics.iter().filter(|(k, _)| static_scored(k)) {
        let Some((_, b)) = c.statics.get(k) else { continue };
        let d = a.abs() + b.abs();
        let sim = if d == 0.0 { 1.0 } else { 1.0 - (a - b).abs() / d };
        feats.push(FeatureScore { name: k.clone(), section: *sec, kind: "static", sim, ref_median: Some(*a), clone_median: Some(*b) });
    }
    if let (Some(rv), Some(cv)) = (&r.visemes, &c.visemes) {
        fn hist(v: &[String]) -> BTreeMap<String, f64> {
            let mut h: BTreeMap<String, f64> = BTreeMap::new();
            for x in v {
                *h.entry(x.clone()).or_default() += 1.0 / v.len() as f64;
            }
            h
        }
        let (hr, hc) = (hist(rv), hist(cv));
        let keys: std::collections::BTreeSet<&String> = hr.keys().chain(hc.keys()).collect();
        let tv: f64 = 0.5 * keys.iter().map(|k| (hr.get(*k).unwrap_or(&0.0) - hc.get(*k).unwrap_or(&0.0)).abs()).sum::<f64>();
        feats.push(FeatureScore { name: "mouth_shapes".into(), section: Section::Mouth, kind: "histogram", sim: 1.0 - tv, ref_median: None, clone_median: None });
    }
    let mut sections: BTreeMap<String, (f64, usize)> = BTreeMap::new();
    for f in &feats {
        let e = sections.entry(sec_name(f.section).into()).or_default();
        e.0 += f.sim;
        e.1 += 1;
    }
    let sections: BTreeMap<String, f64> = sections.into_iter().map(|(k, (s, n))| (k, s / n as f64)).collect();
    let total = if sections.is_empty() {
        0.0
    } else {
        (sections.values().map(|v| v.max(1e-9).ln()).sum::<f64>() / sections.len() as f64).exp()
    };
    let mut order: Vec<usize> = (0..feats.len()).collect();
    order.sort_by(|a, b| feats[*a].sim.partial_cmp(&feats[*b].sim).unwrap());
    let worst = order
        .iter()
        .take(5)
        .map(|i| {
            let f = &feats[*i];
            FeatureScore { name: f.name.clone(), section: f.section, kind: f.kind, sim: f.sim, ref_median: f.ref_median, clone_median: f.clone_median }
        })
        .collect();
    TractScore { schema: 1, method: "TractPrint v1 (quantile W1 / ref IQR, geometric mean of sections)", total, sections, n_features: feats.len(), worst, features: feats }
}
