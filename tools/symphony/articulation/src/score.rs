// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! TractPrint v2.
//!
//! v1 folded every track into one geometric mean. SPARC and Phonet then
//! pulled a male control up next to same-speaker clones (0.795 vs ~0.83).
//! v2 keeps two numbers. `speaker` is only identity: F0 baseline, vocal-tract
//! length, and F1–F3, plus an optional embedding cosine from galion-ecapa.
//! `expression` is everything else. A matched mouth-shape track cannot
//! hide an 8-semitone F0 gap. Scales are in the weights below.

use crate::{Articulation, Section};
use serde::Serialize;
use std::collections::BTreeMap;

const SKIP_EXPRESSION: [&str; 6] = ["f0_hz", "f0_st", "vtl_cm", "f1_hz", "f2_hz", "f3_hz"];

#[derive(Debug, Serialize, Clone)]
pub struct FeatureScore {
    pub name: String,
    pub section: Section,
    pub part: String,
    pub kind: &'static str,
    pub sim: f64,
    pub weight: f64,
    pub ref_median: Option<f64>,
    pub clone_median: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct TractScore {
    pub schema: u32,
    pub method: &'static str,
    pub total: f64,
    pub speaker: f64,
    pub expression: f64,
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

fn median_of(a: &Articulation, name: &str) -> Option<f64> {
    a.summary
        .get(name)
        .and_then(|s| s.q.get(4).copied())
        .filter(|v| v.is_finite())
}

fn sim_abs(a: f64, b: f64, scale: f64) -> f64 {
    1.0 / (1.0 + (a - b).abs() / scale.max(1e-6))
}

/// Semitone distance. 2.5 st is one step on the scale, so a male-range
/// drop (~8–10 st against a higher-pitched reference) lands well below 0.4
/// and a 2 Hz same-speaker drift stays above 0.9.
fn sim_f0(a: f64, b: f64) -> f64 {
    if a <= 0.0 || b <= 0.0 {
        return 0.0;
    }
    let st = (12.0 * (a / b).log2()).abs();
    1.0 / (1.0 + st / 2.5)
}

fn static_scored(name: &str) -> bool {
    !(name.contains("kurtosis") || name.contains("skewness"))
}

fn wmean(rows: &[(f64, f64)]) -> f64 {
    let (s, w) = rows
        .iter()
        .fold((0.0, 0.0), |(s, w), (v, wt)| (s + v * wt, w + wt));
    if w == 0.0 {
        0.0
    } else {
        s / w
    }
}

pub fn compare(r: &Articulation, c: &Articulation) -> TractScore {
    compare_with(r, c, None)
}

pub fn compare_with(r: &Articulation, c: &Articulation, embedding_cos: Option<f64>) -> TractScore {
    let mut feats = Vec::new();
    let mut speaker_rows = Vec::new();

    if let (Some(a), Some(b)) = (median_of(r, "f0_hz"), median_of(c, "f0_hz")) {
        let sim = sim_f0(a, b);
        speaker_rows.push((sim, 5.0));
        feats.push(FeatureScore {
            name: "f0_baseline_hz".into(),
            section: Section::Throat,
            part: "speaker".into(),
            kind: "baseline",
            sim,
            weight: 5.0,
            ref_median: Some(a),
            clone_median: Some(b),
        });
    }
    if let (Some(a), Some(b)) = (median_of(r, "vtl_cm"), median_of(c, "vtl_cm")) {
        if (10.0..22.0).contains(&a) && (10.0..22.0).contains(&b) {
            let sim = sim_abs(a, b, 1.2);
            speaker_rows.push((sim, 4.0));
            feats.push(FeatureScore {
                name: "vtl_cm".into(),
                section: Section::Mouth,
                part: "speaker".into(),
                kind: "baseline",
                sim,
                weight: 4.0,
                ref_median: Some(a),
                clone_median: Some(b),
            });
        }
    }
    for (name, scale, weight) in [
        ("f1_hz", 80.0, 2.0),
        ("f2_hz", 150.0, 2.0),
        ("f3_hz", 150.0, 1.0),
    ] {
        if let (Some(a), Some(b)) = (median_of(r, name), median_of(c, name)) {
            let sim = sim_abs(a, b, scale);
            speaker_rows.push((sim, weight));
            feats.push(FeatureScore {
                name: name.into(),
                section: Section::Mouth,
                part: "speaker".into(),
                kind: "baseline",
                sim,
                weight,
                ref_median: Some(a),
                clone_median: Some(b),
            });
        }
    }
    if let Some(cos) = embedding_cos {
        let sim = cos.clamp(0.0, 1.0);
        speaker_rows.push((sim, 6.0));
        feats.push(FeatureScore {
            name: "speaker_embedding_cos".into(),
            section: Section::Throat,
            part: "speaker".into(),
            kind: "embedding",
            sim,
            weight: 6.0,
            ref_median: None,
            clone_median: Some(sim),
        });
    }

    for (k, rs) in &r.summary {
        if SKIP_EXPRESSION.contains(&k.as_str()) {
            continue;
        }
        let Some(cs) = c.summary.get(k) else { continue };
        if rs.n < 3 || cs.n < 3 {
            continue;
        }
        let w =
            rs.q.iter()
                .zip(&cs.q)
                .map(|(a, b)| (a - b).abs())
                .sum::<f64>()
                / rs.q.len() as f64;
        let iqr = rs.q[6] - rs.q[2];
        let scale = iqr.max(0.05 * rs.q[4].abs()).max(1e-6);
        feats.push(FeatureScore {
            name: k.clone(),
            section: rs.section,
            part: "expression".into(),
            kind: "distribution",
            sim: 1.0 / (1.0 + w / scale),
            weight: 1.0,
            ref_median: Some(rs.q[4]),
            clone_median: Some(cs.q[4]),
        });
    }
    for (k, (sec, a)) in r.statics.iter().filter(|(k, _)| static_scored(k)) {
        let Some((_, b)) = c.statics.get(k) else {
            continue;
        };
        let d = a.abs() + b.abs();
        let sim = if d == 0.0 {
            1.0
        } else {
            1.0 - (a - b).abs() / d
        };
        feats.push(FeatureScore {
            name: k.clone(),
            section: *sec,
            part: "expression".into(),
            kind: "static",
            sim,
            weight: 1.0,
            ref_median: Some(*a),
            clone_median: Some(*b),
        });
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
        let tv: f64 = 0.5
            * keys
                .iter()
                .map(|k| (hr.get(*k).unwrap_or(&0.0) - hc.get(*k).unwrap_or(&0.0)).abs())
                .sum::<f64>();
        feats.push(FeatureScore {
            name: "mouth_shapes".into(),
            section: Section::Mouth,
            part: "expression".into(),
            kind: "histogram",
            sim: 1.0 - tv,
            weight: 1.0,
            ref_median: None,
            clone_median: None,
        });
    }

    let speaker = wmean(&speaker_rows);
    let expr: Vec<f64> = feats
        .iter()
        .filter(|f| f.part == "expression")
        .map(|f| f.sim)
        .collect();
    let expression = if expr.is_empty() {
        1.0
    } else {
        expr.iter().sum::<f64>() / expr.len() as f64
    };
    let total = if speaker_rows.is_empty() {
        expression
    } else {
        speaker.powf(0.65) * expression.max(1e-9).powf(0.35)
    };
    let mut sections: BTreeMap<String, (f64, usize)> = BTreeMap::new();
    for f in feats.iter().filter(|f| f.part == "expression") {
        let e = sections.entry(sec_name(f.section).into()).or_default();
        e.0 += f.sim;
        e.1 += 1;
    }
    let mut sections: BTreeMap<String, f64> = sections
        .into_iter()
        .map(|(k, (s, n))| (k, s / n as f64))
        .collect();
    sections.insert("speaker".into(), speaker);
    sections.insert("expression".into(), expression);
    let mut order: Vec<usize> = (0..feats.len()).collect();
    order.sort_by(|a, b| feats[*a].sim.partial_cmp(&feats[*b].sim).unwrap());
    let worst = order.iter().take(5).map(|i| feats[*i].clone()).collect();
    TractScore {
        schema: 2,
        method: "TractPrint v2 (speaker: F0 baseline, VTL, F1-F3, optional embedding; expression: the rest)",
        total,
        speaker,
        expression,
        sections,
        n_features: feats.len(),
        worst,
        features: feats,
    }
}
