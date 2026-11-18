// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Audible breaths on the planned timeline.
//!
//! The noise source is the heartbeat breath: the same LCG as
//! `crates/galion-heartbeat/src/presence.rs`, whose unwindowed breath sits at
//! RMS 0.003. A Hann envelope keeps the block from clicking. The block is then
//! rescaled to the loudness the plan already computed. That envelope is not a
//! published breath spectrum.
//!
//! When the caller passes the speaker's own samples and the features agent's
//! cues, the inhale timbre is that cue, fitted to the planned duration and
//! the planned level. The samples are never written by this module.

use crate::config::Config;
use crate::plan::Plan;
use crate::stage::{self, BreathCue};
use serde::Serialize;

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct RenderedEvent {
    pub kind: String,
    pub form: String,
    pub start_s: f64,
    pub end_s: f64,
    pub loudness_dbfs: Option<f64>,
    /// `silence`, `synthesized`, or `reference`.
    pub source: String,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Rendered {
    pub sample_rate: f64,
    pub samples: Vec<f64>,
    pub events: Vec<RenderedEvent>,
}

pub struct Reference<'a> {
    pub samples: &'a [f64],
    pub sample_rate: f64,
    pub cues: &'a [BreathCue],
}

/// Breath track only. Speech segments are silent so a voice renderer can fill them.
pub fn render_track(
    plan: &Plan,
    sr: f64,
    reference: Option<&Reference<'_>>,
    cfg: &Config,
) -> Result<Rendered, String> {
    if !(sr.is_finite() && sr > 0.0) {
        return Err("sample rate must be a positive finite number".into());
    }
    let n = (plan.duration_s * sr).round().max(0.0) as usize;
    let mut samples = vec![0.0; n];
    let mut events = Vec::new();
    let mut rng = Lcg(cfg.audible.seed);
    let reference_inhale = reference.and_then(|r| {
        let cue = stage::primary_cue(r.cues)?;
        stage::cue_samples(r.samples, r.sample_rate, cue)
    });

    for seg in &plan.segments {
        let a = (seg.start_s * sr).round().max(0.0) as usize;
        let b = (seg.end_s * sr).round().max(a as f64) as usize;
        let b = b.min(n);
        let len = b.saturating_sub(a);
        let source = match seg.kind.as_str() {
            "inhale" | "exhale" => {
                let target = db_to_rms(seg.loudness_dbfs.unwrap_or(-120.0));
                if seg.kind == "inhale" {
                    if let Some(cue) = &reference_inhale {
                        let mut block = fit(cue, len);
                        scale_rms(&mut block, target);
                        samples[a..b].copy_from_slice(&block);
                        "reference"
                    } else {
                        let mut block = noise(&mut rng, len);
                        hann(&mut block);
                        scale_rms(&mut block, target);
                        samples[a..b].copy_from_slice(&block);
                        "synthesized"
                    }
                } else {
                    let mut block = noise(&mut rng, len);
                    hann(&mut block);
                    scale_rms(&mut block, target);
                    samples[a..b].copy_from_slice(&block);
                    "synthesized"
                }
            }
            _ => "silence",
        };
        events.push(RenderedEvent {
            kind: seg.kind.clone(),
            form: seg.form.clone(),
            start_s: seg.start_s,
            end_s: seg.end_s,
            loudness_dbfs: seg.loudness_dbfs,
            source: source.to_string(),
        });
    }
    Ok(Rendered {
        sample_rate: sr,
        samples,
        events,
    })
}

/// Place each phrase's samples into the speech spans and the breaths around them.
/// Phrase audio that is longer than the span is trimmed. Shorter audio is padded
/// with silence. This does not time-scale the voice.
pub fn mix_phrases(
    phrases: &[Vec<f64>],
    plan: &Plan,
    sr: f64,
    reference: Option<&Reference<'_>>,
    cfg: &Config,
) -> Result<Rendered, String> {
    if phrases.len() != plan.phrases.len() {
        return Err(format!(
            "got {} phrase buffers for {} planned phrases",
            phrases.len(),
            plan.phrases.len()
        ));
    }
    let mut rendered = render_track(plan, sr, reference, cfg)?;
    for seg in &plan.segments {
        if seg.kind != "speech" {
            continue;
        }
        let a = (seg.start_s * sr).round().max(0.0) as usize;
        let b = ((seg.end_s * sr).round() as usize).min(rendered.samples.len());
        if b <= a {
            continue;
        }
        let dst = &mut rendered.samples[a..b];
        let src = phrases.get(seg.phrase).map(|p| p.as_slice()).unwrap_or(&[]);
        let n = dst.len().min(src.len());
        dst[..n].copy_from_slice(&src[..n]);
    }
    Ok(rendered)
}

fn db_to_rms(dbfs: f64) -> f64 {
    10f64.powf(dbfs / 20.0)
}

fn scale_rms(x: &mut [f64], target: f64) {
    if x.is_empty() || !(target.is_finite() && target >= 0.0) {
        return;
    }
    let rms = rms(x);
    if rms < 1e-12 {
        return;
    }
    let g = target / rms;
    for s in x.iter_mut() {
        *s *= g;
    }
}

pub fn rms(x: &[f64]) -> f64 {
    if x.is_empty() {
        return 0.0;
    }
    let e = x.iter().map(|v| v * v).sum::<f64>() / x.len() as f64;
    e.sqrt()
}

fn fit(src: &[f64], n: usize) -> Vec<f64> {
    if n == 0 {
        return Vec::new();
    }
    if src.is_empty() {
        return vec![0.0; n];
    }
    if src.len() == 1 || n == 1 {
        return vec![src[0]; n];
    }
    (0..n)
        .map(|i| {
            let x = i as f64 * (src.len() - 1) as f64 / (n - 1) as f64;
            let j = x.floor() as usize;
            let f = x - j as f64;
            let a = src[j];
            let b = src[(j + 1).min(src.len() - 1)];
            a * (1.0 - f) + b * f
        })
        .collect()
}

fn hann(x: &mut [f64]) {
    let n = x.len();
    if n < 2 {
        return;
    }
    for (i, s) in x.iter_mut().enumerate() {
        let w = 0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / (n - 1) as f64).cos();
        *s *= w;
    }
}

fn noise(rng: &mut Lcg, n: usize) -> Vec<f64> {
    (0..n).map(|_| rng.next()).collect()
}

/// Deterministic LCG from `galion-heartbeat` `presence.rs`.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 40) as f64 / (1u64 << 24) as f64) * 2.0 - 1.0
    }
}
