// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Fluid emotion state, kept apart from the stable speaker baseline.
//!
//! Dimensional numbers come from an existing model when one was run
//! (audeering MSP-dim via `galion-emotion`, or a per-tick JSON dump).
//! This file does not train or reimplement that model. With no model
//! output it only moves arousal with the loudness and F0 already measured
//! by praatfan, and says so in `source`.
//!
//! Blend weights are neutral plus the twelve Cowen, Laukka, Elfenbein, Liu
//! and Keltner (2019) prosody categories. Laughter, sigh and breath are vocal
//! bursts, not blend weights. `seductive` is the render alias of `desire`.
//! No coordinate is copied from a figure. A requested category gets the
//! intensity envelope; arousal moves only where ARTICULATION-V2-SPEC.md
//! cites a direction (anger, fear, sadness high/low; desire a small F0 drop).

use crate::Articulation;
use serde::Serialize;
use std::collections::BTreeMap;

/// Cowen et al. 2019 prosody categories, in the owner's order.
pub const COWEN: [&str; 12] = [
    "adoration",
    "amusement",
    "anger",
    "awe",
    "confusion",
    "contempt",
    "desire",
    "disappointment",
    "distress",
    "fear",
    "interest",
    "sadness",
];

pub const BLEND_IDS: [&str; 13] = [
    "neutral",
    "adoration",
    "amusement",
    "anger",
    "awe",
    "confusion",
    "contempt",
    "desire",
    "disappointment",
    "distress",
    "fear",
    "interest",
    "sadness",
];

/// Not emotions. A separate detector track.
pub const BURSTS: [&str; 3] = ["laughter", "sigh", "breath"];

/// `seductive` is desire. Bursts are refused here.
pub fn canonical_emotion(name: &str) -> Result<&'static str, String> {
    if BURSTS.contains(&name) {
        return Err(format!("{name} is a vocal burst, not an emotion"));
    }
    if name == "seductive" {
        return Ok("desire");
    }
    BLEND_IDS
        .iter()
        .find(|id| **id == name)
        .copied()
        .ok_or_else(|| format!("unknown emotion {name}"))
}

#[derive(Clone, Debug, Serialize)]
pub struct BlendWeight {
    pub id: String,
    pub w: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct TickState {
    pub t_ms: u32,
    pub arousal: f64,
    pub valence: f64,
    pub dominance: f64,
    pub intensity: f64,
    pub blend: Vec<BlendWeight>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SpeakerBaseline {
    pub f0_hz: f64,
    pub vtl_cm: Option<f64>,
    pub formant_dispersion_hz: Option<f64>,
    /// F0 band used to pick the male/female column in emotion-labels-v2.
    /// 165 Hz is the same cut the formant ceiling already uses. It is a
    /// prior for that file, not a claim about the speaker.
    pub f0_band: String,
    pub labels_column: String,
}

#[derive(Clone, Debug)]
pub struct Avd {
    pub arousal: f64,
    pub valence: f64,
    pub dominance: f64,
}

fn clamp(x: f64) -> f64 {
    x.clamp(-1.0, 1.0)
}

fn median(v: &mut [f64]) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = v.len();
    if n % 2 == 1 { v[n / 2] } else { 0.5 * (v[n / 2 - 1] + v[n / 2]) }
}

fn track_values(art: &Articulation, name: &str) -> Vec<f64> {
    art.tracks
        .get(name)
        .map(|t| t.ticks.iter().flatten().copied().filter(|x| x.is_finite()).collect())
        .unwrap_or_default()
}

pub fn speaker_baseline(art: &Articulation) -> SpeakerBaseline {
    let mut f0 = track_values(art, "f0_hz");
    let f0_hz = if f0.is_empty() { 0.0 } else { median(&mut f0) };
    let mut vtl = track_values(art, "vtl_cm");
    vtl.retain(|v| (10.0..22.0).contains(v));
    let vtl_cm = if vtl.is_empty() { None } else { Some(median(&mut vtl)) };
    let mut f1 = track_values(art, "f1_hz");
    let mut f4 = track_values(art, "f4_hz");
    let dispersion = if f1.is_empty() || f4.is_empty() {
        None
    } else {
        let a = median(&mut f1);
        let b = median(&mut f4);
        if b > a { Some((b - a) / 3.0) } else { None }
    };
    let high = f0_hz >= 165.0;
    SpeakerBaseline {
        f0_hz,
        vtl_cm,
        formant_dispersion_hz: dispersion,
        f0_band: if high { "high" } else { "low" }.into(),
        labels_column: if high { "female" } else { "male" }.into(),
    }
}

/// Mass `intensity` on `emotion`, the rest on neutral. Other categories stay 0.
/// This is not a circumplex fitted to Cowen. The spec publishes no coordinates.
pub fn named_blend(emotion: &str, intensity: f64) -> Vec<BlendWeight> {
    let intensity = if emotion == "neutral" { 0.0 } else { intensity.clamp(0.0, 1.0) };
    BLEND_IDS
        .iter()
        .map(|id| {
            let w = if *id == emotion {
                intensity
            } else if *id == "neutral" {
                1.0 - intensity
            } else {
                0.0
            };
            BlendWeight { id: (*id).into(), w }
        })
        .collect()
}

/// Arousal sign only, where the V2 spec cites a direction for that family.
/// Desire's small negative arousal is the Hughes F0 drop, not a valence.
/// Every other category stays at 0 until a z exists.
fn render_prior(emotion: &str) -> (f64, f64, f64) {
    match emotion {
        "anger" => (0.70, 0.0, 0.0),
        "fear" => (0.60, 0.0, 0.0),
        "sadness" => (-0.50, 0.0, 0.0),
        "desire" => (-0.10, 0.0, 0.0),
        _ => (0.0, 0.0, 0.0),
    }
}

pub fn top_blend(blend: &[BlendWeight]) -> String {
    blend
        .iter()
        .max_by(|a, b| a.w.partial_cmp(&b.w).unwrap())
        .map(|b| b.id.clone())
        .unwrap_or_else(|| "neutral".into())
}

fn envelope(i: usize, n: usize) -> f64 {
    if n <= 1 {
        return 1.0;
    }
    let x = i as f64 / (n - 1) as f64;
    let bell = 0.5 - 0.5 * (2.0 * std::f64::consts::PI * x).cos();
    0.35 + 0.65 * bell
}

fn measured_tick(t_ms: u32, v: f64, a: f64, d: f64) -> TickState {
    TickState {
        t_ms,
        arousal: a,
        valence: v,
        dominance: d,
        intensity: (v * v + a * a).sqrt().clamp(0.0, 1.0),
        blend: named_blend("neutral", 0.0),
    }
}

/// One requested emotion as a trajectory: intensity eases in and out.
/// `seductive` resolves to `desire`. A burst name is an error.
pub fn target_trajectory(emotion: &str, n_ticks: usize) -> Result<Vec<TickState>, String> {
    let name = canonical_emotion(emotion)?;
    let (pa, pv, pd) = render_prior(name);
    Ok((0..n_ticks)
        .map(|i| {
            let env = if name == "neutral" { 0.0 } else { envelope(i, n_ticks) };
            TickState {
                t_ms: (i as u32) * 80,
                arousal: pa * env,
                valence: pv * env,
                dominance: pd * env,
                intensity: env,
                blend: named_blend(name, env),
            }
        })
        .collect())
}

fn mad(vals: &[f64]) -> f64 {
    if vals.is_empty() {
        return 1.0;
    }
    let mut c = vals.to_vec();
    let m = median(&mut c);
    let mut d: Vec<f64> = vals.iter().map(|x| (x - m).abs()).collect();
    (1.4826 * median(&mut d)).max(0.5)
}

fn z_at(series: &[Option<f64>], i: usize, med: f64, scale: f64) -> f64 {
    match series.get(i).copied().flatten() {
        Some(v) if v.is_finite() => (v - med) / scale,
        _ => 0.0,
    }
}

/// Per-tick state. `center` is the model's utterance A/V/D on the −1..1 scale.
/// Loudness and F0 only push the centre around; they are not a new emotion model.
pub fn measure_fluid(art: &Articulation, center: Avd) -> (Vec<TickState>, String) {
    let n = art.n_ticks;
    let rms = art.tracks.get("rms_dbfs").map(|t| t.ticks.clone()).unwrap_or_default();
    let f0 = art.tracks.get("f0_hz").map(|t| t.ticks.clone()).unwrap_or_default();
    let rms_v: Vec<f64> = rms.iter().flatten().copied().filter(|x| x.is_finite() && *x > -100.0).collect();
    let f0_v: Vec<f64> = f0.iter().flatten().copied().filter(|x| x.is_finite()).collect();
    let mut rms_c = rms_v.clone();
    let mut f0_c = f0_v.clone();
    let rms_med = if rms_c.is_empty() { -40.0 } else { median(&mut rms_c) };
    let f0_med = if f0_c.is_empty() { 0.0 } else { median(&mut f0_c) };
    let rms_mad = mad(&rms_v);
    let f0_mad = mad(&f0_v).max(5.0);
    let ticks = (0..n)
        .map(|i| {
            let ez = z_at(&rms, i, rms_med, rms_mad);
            let fz = z_at(&f0, i, f0_med, f0_mad);
            let a = clamp(center.arousal + 0.25 * ez);
            let v = clamp(center.valence + 0.05 * fz);
            let d = clamp(center.dominance + 0.10 * ez);
            measured_tick((i as u32) * 80, v, a, d)
        })
        .collect();
    (ticks, "contour".into())
}

pub fn unit_to_bipolar(x: f64) -> f64 {
    clamp(2.0 * x - 1.0)
}

/// MSP-dim / emotion JSON. `scale = "unit"` is the model's 0..1 range.
pub fn center_from_json(v: &serde_json::Value) -> Result<(Avd, String, Option<Vec<Avd>>), String> {
    let scale = v.get("scale").and_then(|s| s.as_str()).unwrap_or("bipolar");
    let conv = |x: f64| if scale == "unit" { unit_to_bipolar(x) } else { clamp(x) };
    let num = |k: &str| -> Result<f64, String> {
        v.get(k).and_then(|x| x.as_f64()).ok_or_else(|| format!("emotion json missing {k}"))
    };
    let center = Avd { arousal: conv(num("arousal")?), valence: conv(num("valence")?), dominance: conv(num("dominance")?) };
    let source = v.get("source").and_then(|s| s.as_str()).unwrap_or("emotion-json").to_string();
    let ticks = v.get("ticks").and_then(|t| t.as_array()).map(|arr| {
        arr.iter()
            .filter_map(|row| {
                Some(Avd {
                    arousal: conv(row.get("arousal")?.as_f64()?),
                    valence: conv(row.get("valence")?.as_f64()?),
                    dominance: conv(row.get("dominance")?.as_f64()?),
                })
            })
            .collect::<Vec<_>>()
    });
    Ok((center, source, ticks))
}

pub fn fluid_from_model(art: &Articulation, center: Avd, per_tick: Option<Vec<Avd>>, source: String) -> (Vec<TickState>, String) {
    if let Some(rows) = per_tick {
        if rows.len() == art.n_ticks {
            let ticks = rows
                .iter()
                .enumerate()
                .map(|(i, c)| measured_tick((i as u32) * 80, c.valence, c.arousal, c.dominance))
                .collect();
            return (ticks, source);
        }
    }
    let (ticks, _) = measure_fluid(art, center);
    (ticks, source)
}

pub fn summary_top(ticks: &[TickState]) -> BTreeMap<String, f64> {
    let mut acc: BTreeMap<String, f64> = BTreeMap::new();
    if ticks.is_empty() {
        return acc;
    }
    for t in ticks {
        for b in &t.blend {
            *acc.entry(b.id.clone()).or_default() += b.w;
        }
    }
    let n = ticks.len() as f64;
    for v in acc.values_mut() {
        *v /= n;
    }
    acc
}
