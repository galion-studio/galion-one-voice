// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! CPU sound for the two tracks. No model, no download.
//!
//! The murmur is a sinusoid at the config frequency, with RMS following
//! velum opening (House and Stevens: low-frequency energy appears when the
//! port opens). The hertz is a knob. A step each 80 ms is the control rate,
//! not a measured port trajectory.
//!
//! A burst is synthesized from its recipe, or taken from the speaker's own
//! samples when the caller passes them. Those samples are never stored here.
//! Speech is not on either buffer. The oral antiresonance is not applied.

use serde::Serialize;

use crate::bursts::BurstEvent;
use crate::config::Config;
use crate::plan::Plan;

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct RenderedEvent {
    pub gesture: String,
    pub start_s: f64,
    pub end_s: f64,
    /// `synthesized`, `synthesized-unpitched`, `reference`, or `silence`.
    pub source: String,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Rendered {
    pub sample_rate: f64,
    /// Nasal murmur. Silent where the velum is closed.
    pub nose: Vec<f64>,
    /// Wordless track. Silent outside burst events.
    pub bursts: Vec<f64>,
    pub burst_events: Vec<RenderedEvent>,
}

pub struct GestureRef<'a> {
    pub gesture: &'a str,
    pub samples: &'a [f64],
    pub sample_rate: f64,
}

pub fn render(
    plan: &Plan,
    sr: f64,
    refs: &[GestureRef<'_>],
    cfg: &Config,
) -> Result<Rendered, String> {
    if !(sr.is_finite() && sr > 0.0) {
        return Err("sample rate must be a positive finite number".into());
    }
    let n = (plan.duration_s * sr).round().max(0.0) as usize;
    let nose = murmur(plan, n, sr, cfg);
    let mut bursts = vec![0.0; n];
    let mut rng = Lcg(cfg.level.seed);
    let mut burst_events = Vec::new();
    for ev in &plan.bursts {
        let a = (ev.start_s * sr).round().max(0.0) as usize;
        let b = (ev.end_s * sr).round().max(a as f64) as usize;
        let b = b.min(n);
        let len = b.saturating_sub(a);
        let reference = refs.iter().find(|r| r.gesture == ev.gesture);
        let (mut block, source) = if let Some(r) = reference {
            let fitted = fit(r.samples, r.sample_rate, sr, len);
            (fitted, "reference")
        } else {
            (
                recipe(ev, len, sr, cfg, &mut rng),
                if ev.f0_hz > 0.0 && is_voiced(ev) {
                    "synthesized"
                } else if is_voiced(ev) {
                    "synthesized-unpitched"
                } else {
                    "synthesized"
                },
            )
        };
        scale_rms(&mut block, db_to_rms(ev.level_dbfs));
        if a < n {
            for (i, s) in block.into_iter().enumerate() {
                if a + i < n {
                    bursts[a + i] += s;
                }
            }
        }
        burst_events.push(RenderedEvent {
            gesture: ev.gesture.clone(),
            start_s: ev.start_s,
            end_s: ev.end_s,
            source: source.into(),
        });
    }
    Ok(Rendered {
        sample_rate: sr,
        nose,
        bursts,
        burst_events,
    })
}

fn murmur(plan: &Plan, n: usize, sr: f64, cfg: &Config) -> Vec<f64> {
    let mut out = vec![0.0; n];
    if plan.ticks.is_empty() || n == 0 {
        return out;
    }
    let tick = plan.tick_ms as f64 / 1000.0;
    let amp_scale = cfg.murmur.rms_at_open * std::f64::consts::SQRT_2;
    for i in 0..n {
        let t = i as f64 / sr;
        let idx = ((t / tick) as usize).min(plan.ticks.len() - 1);
        let velum = plan.ticks[idx].velum_open;
        if velum <= 0.0 {
            continue;
        }
        let phase = std::f64::consts::TAU * cfg.murmur.hz * t;
        out[i] = amp_scale * velum * phase.sin();
    }
    out
}

fn is_voiced(ev: &BurstEvent) -> bool {
    matches!(ev.gesture.as_str(), "laugh" | "moan" | "hum" | "mm")
}

fn recipe(ev: &BurstEvent, len: usize, sr: f64, cfg: &Config, rng: &mut Lcg) -> Vec<f64> {
    if len == 0 {
        return Vec::new();
    }
    match ev.gesture.as_str() {
        "laugh" => laugh_block(ev, len, sr, rng),
        "gasp" => gasp_block(len, cfg.gasp.attack_fraction, cfg.gasp.decay, rng),
        "sigh" | "moan" | "hum" | "mm" => {
            let mut block = if ev.f0_hz > 0.0 && is_voiced(ev) {
                tone(ev.f0_hz, len, sr)
            } else {
                noise(rng, len)
            };
            hann(&mut block);
            block
        }
        _ => noise(rng, len),
    }
}

fn laugh_block(ev: &BurstEvent, len: usize, sr: f64, rng: &mut Lcg) -> Vec<f64> {
    let mut out = vec![0.0; len];
    let calls = ev.calls.unwrap_or(1).max(1) as usize;
    let call_s = ev.call_s.unwrap_or(0.08).max(0.0);
    let gap_s = ev.gap_s.unwrap_or(0.0).max(0.0);
    let call_n = ((call_s * sr).round() as usize).max(1);
    let gap_n = (gap_s * sr).round() as usize;
    let mut cursor = 0usize;
    for _ in 0..calls {
        if cursor >= len {
            break;
        }
        let n = call_n.min(len - cursor);
        let mut call = if ev.f0_hz > 0.0 {
            tone(ev.f0_hz, n, sr)
        } else {
            noise(rng, n)
        };
        hann(&mut call);
        out[cursor..cursor + n].copy_from_slice(&call);
        cursor += n + gap_n;
    }
    out
}

fn gasp_block(len: usize, attack_fraction: f64, decay: f64, rng: &mut Lcg) -> Vec<f64> {
    let mut block = noise(rng, len);
    let n = block.len().max(1);
    let attack_fraction = attack_fraction.clamp(1e-3, 0.99);
    for (i, s) in block.iter_mut().enumerate() {
        let t = i as f64 / n as f64;
        let attack = if t < attack_fraction {
            t / attack_fraction
        } else {
            1.0
        };
        *s *= attack * (-decay * t).exp();
    }
    block
}

fn tone(hz: f64, len: usize, sr: f64) -> Vec<f64> {
    (0..len)
        .map(|i| (std::f64::consts::TAU * hz * i as f64 / sr).sin())
        .collect()
}

/// Stretch a runtime reference onto the planned event. This is a length fit,
/// not a model of the vocal tract. Sample rates are accepted so the caller
/// can pass the file it read; they do not change the stretch.
fn fit(src: &[f64], src_sr: f64, dst_sr: f64, len: usize) -> Vec<f64> {
    let _ = (src_sr, dst_sr);
    if len == 0 {
        return Vec::new();
    }
    if src.is_empty() {
        return vec![0.0; len];
    }
    if src.len() == 1 || len == 1 {
        return vec![src[0]; len];
    }
    (0..len)
        .map(|i| {
            let x = i as f64 * (src.len() - 1) as f64 / (len - 1) as f64;
            let j = x.floor() as usize;
            let frac = x - j as f64;
            let a = src[j];
            let b = src[(j + 1).min(src.len() - 1)];
            a * (1.0 - frac) + b * frac
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

pub fn db_to_rms(dbfs: f64) -> f64 {
    10f64.powf(dbfs / 20.0)
}

pub fn scale_rms(x: &mut [f64], target: f64) {
    if x.is_empty() || !(target.is_finite() && target >= 0.0) {
        return;
    }
    let now = rms(x);
    if now < 1e-12 {
        return;
    }
    let g = target / now;
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
