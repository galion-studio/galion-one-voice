// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Read-back. Pitch, voicing, HNR and formants come from
//! `galion-articulation` (`acoustic::analyse`, praatfan), which is the
//! analyser PR #191's `decode` profile is built on. This file does not
//! recompute those tracks.
//!
//! When a `galion-voice-decode/2` document is supplied (the `decode`
//! command on that draft), the feeling label and the per-tick state are
//! taken from it. The acoustic tracks stay the analyser's.

use galion_articulation::{acoustic, Articulation};
use serde::Serialize;

use crate::round4;

pub const TICK_S: f64 = galion_articulation::TICK_S;

#[derive(Clone, Debug, Serialize)]
pub struct DecodedFeeling {
    pub t_ms: u32,
    pub arousal: f64,
    pub valence: f64,
    pub dominance: f64,
    pub intensity: f64,
}

#[derive(Clone, Debug)]
pub struct ReadBack {
    pub art: Articulation,
    pub decoder: String,
    pub n_ticks: usize,
    pub f0_hz_median: f64,
    pub f0_hz_ticks: Vec<Option<f64>>,
    pub voicing_ticks: Vec<f64>,
    pub rms_db_ticks: Vec<f64>,
    pub f1_hz: Option<f64>,
    pub f2_hz: Option<f64>,
    pub f3_hz: Option<f64>,
    pub vtl_cm: Option<f64>,
    pub hnr_db: Option<f64>,
    pub feeling_top: Option<String>,
    pub feeling_track: Option<String>,
    pub feeling_ticks: Vec<DecodedFeeling>,
    pub profile_speaker_f0_hz: Option<f64>,
}

pub fn analyse(samples: &[f64], sr: f64) -> Result<ReadBack, String> {
    if samples.is_empty() || sr <= 0.0 {
        return Err("decode: empty audio".into());
    }
    let (mut art, _) = acoustic::analyse(samples, sr);
    art.finish();
    let n = art.n_ticks;
    let f0_hz_ticks = track_opts(&art, "f0_hz");
    let mut voiced: Vec<f64> = f0_hz_ticks
        .iter()
        .flatten()
        .copied()
        .filter(|h| *h > 0.0)
        .collect();
    let f0_hz_median = if voiced.is_empty() {
        0.0
    } else {
        median(&mut voiced)
    };
    Ok(ReadBack {
        n_ticks: n,
        f0_hz_median: round4(f0_hz_median),
        f0_hz_ticks,
        voicing_ticks: track_opts(&art, "voicing")
            .into_iter()
            .map(|v| v.unwrap_or(0.0))
            .collect(),
        rms_db_ticks: rms_db(samples, sr, n),
        f1_hz: med_track(&art, "f1_hz"),
        f2_hz: med_track(&art, "f2_hz"),
        f3_hz: med_track(&art, "f3_hz"),
        vtl_cm: med_track(&art, "vtl_cm").filter(|v| (10.0..22.0).contains(v)),
        hnr_db: med_track(&art, "hnr_db"),
        feeling_top: None,
        feeling_track: None,
        feeling_ticks: Vec::new(),
        profile_speaker_f0_hz: None,
        decoder: "galion-articulation analyse (praatfan_rust)".into(),
        art,
    })
}

/// Fold a PR #191 profile into an acoustic read-back. Acoustic tracks are
/// left as the analyser measured them.
pub fn merge_profile(read: &mut ReadBack, value: &serde_json::Value) -> Result<(), String> {
    let schema = value.get("schema").and_then(|v| v.as_str()).unwrap_or("");
    if schema != "galion-voice-decode/2" {
        return Err(format!(
            "decode profile schema is {schema:?}, want galion-voice-decode/2"
        ));
    }
    let measured = value
        .get("measured")
        .ok_or("decode profile missing measured")?;
    read.feeling_top = measured
        .get("top")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    read.feeling_track = measured
        .get("track")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    read.feeling_ticks = measured
        .get("ticks")
        .and_then(|v| v.as_array())
        .map(|rows| {
            rows.iter()
                .enumerate()
                .map(|(i, row)| DecodedFeeling {
                    t_ms: row
                        .get("t_ms")
                        .and_then(|v| v.as_u64())
                        .unwrap_or((i as u64) * 80) as u32,
                    arousal: row.get("arousal").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    valence: row.get("valence").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    dominance: row.get("dominance").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    intensity: row.get("intensity").and_then(|v| v.as_f64()).unwrap_or(0.0),
                })
                .collect()
        })
        .unwrap_or_default();
    read.profile_speaker_f0_hz = value
        .pointer("/speaker/f0_hz")
        .and_then(|v| v.as_f64())
        .filter(|h| h.is_finite() && *h > 0.0);
    if !read.decoder.contains("galion-voice-decode/2") {
        read.decoder.push_str(" + galion-voice-decode/2");
    }
    Ok(())
}

pub fn rms_db(samples: &[f64], sr: f64, n: usize) -> Vec<f64> {
    let hop = (sr * TICK_S).round().max(1.0) as usize;
    (0..n)
        .map(|i| {
            let a = i * hop;
            let b = ((i + 1) * hop).min(samples.len());
            if b <= a {
                return -120.0;
            }
            let e = samples[a..b].iter().map(|s| s * s).sum::<f64>() / (b - a) as f64;
            round4(if e <= 1e-12 { -120.0 } else { 10.0 * e.log10() })
        })
        .collect()
}

fn track_opts(art: &Articulation, name: &str) -> Vec<Option<f64>> {
    art.tracks
        .get(name)
        .map(|t| t.ticks.clone())
        .unwrap_or_default()
}

fn med_track(art: &Articulation, name: &str) -> Option<f64> {
    let q = art.summary.get(name)?.q.get(4).copied()?;
    if q.is_finite() && q > 0.0 {
        Some(round4(q))
    } else {
        None
    }
}

fn median(v: &mut [f64]) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = v.len();
    if n % 2 == 1 {
        v[n / 2]
    } else {
        0.5 * (v[n / 2 - 1] + v[n / 2])
    }
}
