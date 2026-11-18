// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Voice-decode profile: what a cloner reads as conditioning metadata.
//!
//! Speaker baseline stays put. The fluid trajectory is the emotion.
//! Chatterbox takes one exaggeration / cfg / speed for a whole utterance
//! (the median of the trajectory) and the per-tick trajectory for a renderer
//! that can follow it. `cfg` is Chatterbox `cfg_weight`. `speed` is a
//! time-scale; the Nano worker in this repo has no speed argument.

use crate::breath::{self, Lungs};
use crate::environment::{self, Environment};
use crate::labels::Labels;
use crate::state::{self, SpeakerBaseline, TickState};
use crate::{summarise, Articulation, Brick};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Serialize)]
pub struct RenderParams {
    pub exaggeration: f64,
    pub cfg: f64,
    pub speed: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct TickParams {
    pub t_ms: u32,
    pub emotion: String,
    pub intensity: f64,
    pub exaggeration: f64,
    pub cfg: f64,
    pub speed: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct SectionSnap {
    pub f0_hz: Option<f64>,
    pub hnr_db: Option<f64>,
    pub h1_h2_db: Option<f64>,
    pub f1_hz: Option<f64>,
    pub f2_hz: Option<f64>,
    pub f3_hz: Option<f64>,
    pub f4_hz: Option<f64>,
    pub vtl_cm: Option<f64>,
    pub phon_nasal: Option<f64>,
    pub a1_p0: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct FluidSide {
    pub source: String,
    /// `prosody`, `burst`, or `baseline`.
    pub track: String,
    pub top: String,
    pub ticks: Vec<TickState>,
}

#[derive(Clone, Debug, Serialize)]
pub struct BurstTrack {
    /// Mean weight of the wordless 24 across the target ticks. Same ids as prosody.
    pub weights: Vec<state::BlendWeight>,
    pub top: String,
    pub source: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct VoiceProfile {
    pub schema: String,
    pub speaker: SpeakerBaseline,
    pub measured: FluidSide,
    pub target: FluidSide,
    pub throat: SectionSnap,
    pub mouth: SectionSnap,
    pub nose: SectionSnap,
    pub lungs: Lungs,
    pub burst: BurstTrack,
    pub environment: Environment,
    pub chatterbox: RenderParams,
    pub trajectory: Vec<TickParams>,
    pub emotion_match: BTreeMap<String, f64>,
    pub egemaps: Option<BTreeMap<String, f64>>,
    pub gaps: Vec<String>,
    pub bricks: Vec<Brick>,
}

pub struct Opts<'a> {
    pub emotion: Option<&'a str>,
    pub env_name: Option<&'a str>,
    pub emotion_json: Option<&'a Value>,
    pub egemaps: Option<&'a Value>,
    pub neutral_egemaps: Option<&'a Value>,
    pub labels: Option<&'a Labels>,
    pub cpps_db: Option<f64>,
    pub breath_cues: Option<Vec<breath::BreathCue>>,
    pub breath_source: Option<&'a str>,
}

fn r4(x: f64) -> f64 {
    (x * 1e4).round() / 1e4
}

fn med_track(art: &Articulation, name: &str) -> Option<f64> {
    let t = art.tracks.get(name)?;
    let s = summarise(t)?;
    s.q.get(4).copied()
}

fn snap(art: &Articulation) -> (SectionSnap, SectionSnap, SectionSnap) {
    let s = SectionSnap {
        f0_hz: med_track(art, "f0_hz"),
        hnr_db: med_track(art, "hnr_db"),
        h1_h2_db: breath::h1_h2(art),
        f1_hz: med_track(art, "f1_hz"),
        f2_hz: med_track(art, "f2_hz"),
        f3_hz: med_track(art, "f3_hz"),
        f4_hz: med_track(art, "f4_hz"),
        vtl_cm: med_track(art, "vtl_cm").filter(|v| (10.0..22.0).contains(v)),
        phon_nasal: med_track(art, "phon_nasal"),
        a1_p0: None,
    };
    let throat = SectionSnap {
        f0_hz: s.f0_hz,
        hnr_db: s.hnr_db,
        h1_h2_db: s.h1_h2_db,
        f1_hz: None,
        f2_hz: None,
        f3_hz: None,
        f4_hz: None,
        vtl_cm: None,
        phon_nasal: None,
        a1_p0: None,
    };
    let mouth = SectionSnap {
        f0_hz: None,
        hnr_db: None,
        h1_h2_db: None,
        f1_hz: s.f1_hz,
        f2_hz: s.f2_hz,
        f3_hz: s.f3_hz,
        f4_hz: s.f4_hz,
        vtl_cm: s.vtl_cm,
        phon_nasal: None,
        a1_p0: None,
    };
    let nose = SectionSnap {
        f0_hz: None,
        hnr_db: None,
        h1_h2_db: None,
        f1_hz: None,
        f2_hz: None,
        f3_hz: None,
        f4_hz: None,
        vtl_cm: None,
        phon_nasal: s.phon_nasal,
        a1_p0: None,
    };
    (throat, mouth, nose)
}

fn burst_summary(ticks: &[TickState]) -> BurstTrack {
    let mut acc: BTreeMap<String, f64> = BTreeMap::new();
    for id in state::COWEN {
        acc.insert(id.into(), 0.0);
    }
    if !ticks.is_empty() {
        for t in ticks {
            for b in &t.burst {
                *acc.entry(b.id.clone()).or_default() += b.w;
            }
        }
        let n = ticks.len() as f64;
        for v in acc.values_mut() {
            *v /= n;
        }
    }
    let top = acc
        .iter()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
        .filter(|(_, w)| **w > 0.0)
        .map(|(k, _)| k.clone())
        .unwrap_or_else(|| "none".into());
    let weights = state::COWEN
        .iter()
        .map(|id| state::BlendWeight {
            id: (*id).into(),
            w: acc.get(*id).copied().unwrap_or(0.0),
        })
        .collect();
    BurstTrack {
        weights,
        top,
        source: "same-24-as-prosody".into(),
    }
}

fn top_name(weights: &BTreeMap<String, f64>) -> String {
    weights
        .iter()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(k, _)| k.clone())
        .unwrap_or_else(|| "neutral".into())
}

pub fn params_for(tick: &TickState) -> RenderParams {
    RenderParams {
        exaggeration: r4((0.35 + 0.85 * tick.intensity).clamp(0.0, 1.5)),
        cfg: r4((0.55 - 0.08 * tick.arousal.abs()).clamp(0.3, 0.85)),
        speed: r4((1.0 + 0.20 * tick.arousal).clamp(0.80, 1.25)),
    }
}

fn median_params(xs: &[RenderParams]) -> RenderParams {
    fn med(v: &mut [f64]) -> f64 {
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let n = v.len();
        if n == 0 {
            return 0.0;
        }
        if n % 2 == 1 {
            v[n / 2]
        } else {
            0.5 * (v[n / 2 - 1] + v[n / 2])
        }
    }
    let mut e: Vec<f64> = xs.iter().map(|p| p.exaggeration).collect();
    let mut c: Vec<f64> = xs.iter().map(|p| p.cfg).collect();
    let mut s: Vec<f64> = xs.iter().map(|p| p.speed).collect();
    RenderParams {
        exaggeration: r4(med(&mut e)),
        cfg: r4(med(&mut c)),
        speed: r4(med(&mut s)),
    }
}

fn trajectory(ticks: &[TickState]) -> Vec<TickParams> {
    ticks
        .iter()
        .map(|t| {
            let p = params_for(t);
            let burst = t
                .burst
                .iter()
                .max_by(|a, b| a.w.partial_cmp(&b.w).unwrap_or(std::cmp::Ordering::Equal));
            let emotion = match burst {
                Some(b) if b.w > 0.0 => b.id.clone(),
                _ => state::top_blend(&t.blend),
            };
            TickParams {
                t_ms: t.t_ms,
                emotion,
                intensity: r4(t.intensity),
                exaggeration: p.exaggeration,
                cfg: p.cfg,
                speed: p.speed,
            }
        })
        .collect()
}

fn feature_map(v: &Value) -> Option<BTreeMap<String, f64>> {
    let obj = v.get("features")?.as_object()?;
    Some(
        obj.iter()
            .filter_map(|(k, x)| x.as_f64().map(|n| (k.clone(), n)))
            .collect(),
    )
}

pub fn profile(
    art: &Articulation,
    samples: &[f64],
    sr: f64,
    opts: &Opts,
) -> Result<VoiceProfile, String> {
    let speaker = state::speaker_baseline(art);
    let (measured_ticks, measured_source) = if let Some(js) = opts.emotion_json {
        let (center, source, per) = state::center_from_json(js)?;
        state::fluid_from_model(art, center, per, source)
    } else {
        let (ticks, src) = state::measure_fluid(
            art,
            state::Avd {
                arousal: 0.0,
                valence: 0.0,
                dominance: 0.0,
            },
        );
        (ticks, src)
    };
    let (target_ticks, target_source, target_track, target_name) = if let Some(name) = opts.emotion
    {
        let resolved = state::resolve(name)?;
        (
            state::target_trajectory(name, art.n_ticks)?,
            format!("target:{}:{}", resolved.track, resolved.category),
            resolved.track.to_string(),
            resolved.category.to_string(),
        )
    } else {
        (
            measured_ticks.clone(),
            measured_source.clone(),
            "prosody".into(),
            "measured".into(),
        )
    };
    let traj = trajectory(&target_ticks);
    let chatterbox = median_params(
        &traj
            .iter()
            .map(|t| RenderParams {
                exaggeration: t.exaggeration,
                cfg: t.cfg,
                speed: t.speed,
            })
            .collect::<Vec<_>>(),
    );
    let mut lungs = breath::measure(art, opts.cpps_db);
    if let Some(cues) = opts.breath_cues.clone() {
        breath::apply_cues(
            &mut lungs,
            cues,
            opts.breath_source.unwrap_or("external-breath"),
            art.duration_s,
        );
    }
    let env = environment::measure(samples, sr, opts.env_name)?;
    let egemaps = opts.egemaps.and_then(feature_map);
    let mut emotion_match = BTreeMap::new();
    let mut gaps = vec![
        "nose.a1_p0: no open MIT A1-P0 / nasal-formant implementation wired; phon_nasal is a Phonet class posterior, not nasal resonance".into(),
    ];
    if lungs.breathiness_cpps_db.is_none() {
        gaps.push(
            "lungs.breathiness_cpps_db: CPPS not measured (Praat CPPS is GPL; no MIT brick ran)"
                .into(),
        );
    }
    if measured_source == "contour" {
        gaps.push(
            "emotion: no MSP-dim / emotion2vec pass; arousal follows praatfan loudness and F0 only"
                .into(),
        );
    }
    if egemaps.is_none() {
        gaps.push("egemaps: openSMILE eGeMAPSv02 did not run (workers/egemaps_worker.py)".into());
    }
    if target_track != "burst" {
        gaps.push("burst: no wordless detector ran; the 24 burst weights stay 0 unless the render target is a burst".into());
    }
    let burst = burst_summary(&target_ticks);
    if let (Some(labels), Some(clip), Some(neutral_v)) =
        (opts.labels, egemaps.as_ref(), opts.neutral_egemaps)
    {
        if let Some(neutral) = feature_map(neutral_v) {
            let z = crate::labels::z_scores(clip, &neutral, labels);
            emotion_match = crate::labels::match_emotions(&z, labels, &speaker.labels_column);
        }
    } else if opts.labels.is_some() {
        gaps.push("emotion-labels: need the clip and a neutral eGeMAPS pass to score z against emotion-labels-v2".into());
    }
    let (throat, mouth, nose) = snap(art);
    let measured_top = top_name(&state::summary_top(&measured_ticks));
    Ok(VoiceProfile {
        schema: "galion-voice-decode/2".into(),
        speaker,
        measured: FluidSide {
            source: measured_source,
            track: "prosody".into(),
            top: measured_top,
            ticks: measured_ticks,
        },
        target: FluidSide {
            source: target_source,
            track: target_track,
            top: target_name,
            ticks: target_ticks,
        },
        throat,
        mouth,
        nose,
        lungs,
        burst,
        environment: env,
        chatterbox,
        trajectory: traj,
        emotion_match,
        egemaps,
        gaps,
        bricks: art.bricks.clone(),
    })
}

pub fn sample_rms_db(x: &[f64]) -> f64 {
    if x.is_empty() {
        return -120.0;
    }
    let e = x.iter().map(|v| v * v).sum::<f64>() / x.len() as f64;
    if e <= 1e-12 {
        -120.0
    } else {
        10.0 * e.log10()
    }
}

/// One gain and one time-scale from the utterance params.
/// The per-tick trajectory stays in the profile for a renderer that can
/// follow it. This stand-in does not synthesise a voice.
pub fn apply_render(
    samples: &[f64],
    _sr: f64,
    utterance: RenderParams,
    _traj: &[TickParams],
) -> Vec<f64> {
    let gain = 10f64.powf((utterance.exaggeration - 0.5) * 12.0 / 20.0);
    let y: Vec<f64> = samples
        .iter()
        .map(|s| (s * gain).clamp(-1.0, 1.0))
        .collect();
    time_scale(&y, utterance.speed)
}

fn time_scale(x: &[f64], speed: f64) -> Vec<f64> {
    let speed = speed.clamp(0.5, 2.0);
    if (speed - 1.0).abs() < 1e-3 || x.is_empty() {
        return x.to_vec();
    }
    let n = ((x.len() as f64) / speed).round().max(1.0) as usize;
    (0..n)
        .map(|i| {
            let src = i as f64 * speed;
            let j = src.floor() as usize;
            let f = src - j as f64;
            let a = x.get(j).copied().unwrap_or(0.0);
            let b = x.get(j + 1).copied().unwrap_or(a);
            a * (1.0 - f) + b * f
        })
        .collect()
}
