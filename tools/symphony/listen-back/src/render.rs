// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Sound-out stand-in (layer S6) for CI. A synthetic vowel plus optional
//! noise inhalations. The first pass applies part of the asked pitch and
//! drops the inhalations, so the correction pass has something to close.
//! That undershoot is not a model of Chatterbox.
//!
//! A real cloner is `--render-cmd` / `GALION_RENDER_CMD`. It receives the
//! request JSON and writes a wav. Nothing is downloaded.

use std::path::Path;
use std::process::Command;

use serde::Serialize;

use crate::contract::Intent;
use crate::fixture::BreathSpan;
use crate::round4;

pub const SR: f64 = 16_000.0;

/// Fraction of the asked semitone offset the first stand-in pass actually
/// sings. The rest is what the one correction is for.
pub const STANDIN_PITCH_FRACTION: f64 = 0.55;

#[derive(Clone, Debug)]
pub struct Control {
    pub pitch_st: f64,
    pub place_breaths: bool,
    pub renderer: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct RenderRequest {
    pub schema: String,
    pub text: String,
    pub track: String,
    pub category: String,
    pub baseline_f0_hz: f64,
    pub pitch_st: f64,
    pub f1_hz: f64,
    pub f2_hz: f64,
    pub f3_hz: f64,
    pub speech_s: f64,
    pub breaths: Vec<BreathSpan>,
    pub sample_rate_hz: u32,
}

impl Control {
    pub fn standin_first(intent: &Intent) -> Self {
        Control {
            pitch_st: round4(intent.pitch_st * STANDIN_PITCH_FRACTION),
            place_breaths: false,
            renderer: "stand-in".into(),
        }
    }
}

pub fn request_of(intent: &Intent, control: &Control) -> RenderRequest {
    RenderRequest {
        schema: "galion-listen-back-render/1".into(),
        text: intent.text.clone(),
        track: intent.track.clone(),
        category: intent.category.clone(),
        baseline_f0_hz: intent.baseline_f0_hz,
        pitch_st: round4(control.pitch_st),
        f1_hz: intent.f1_hz,
        f2_hz: intent.f2_hz,
        f3_hz: intent.f3_hz,
        speech_s: intent.speech_s,
        breaths: if control.place_breaths {
            intent.breaths.clone()
        } else {
            Vec::new()
        },
        sample_rate_hz: SR as u32,
    }
}

pub fn hz(baseline_hz: f64, pitch_st: f64) -> f64 {
    baseline_hz * 2f64.powf(pitch_st / 12.0)
}

/// Reference vowel at the baseline pitch, no breath. Articulation and
/// speaker scores compare against this, not against a second person.
pub fn reference(intent: &Intent) -> Vec<f64> {
    vowel(
        intent.baseline_f0_hz,
        intent.f1_hz,
        intent.f2_hz,
        intent.speech_s,
        SR,
    )
}

pub fn synthesize(intent: &Intent, control: &Control) -> Vec<f64> {
    let f0 = hz(intent.baseline_f0_hz, control.pitch_st);
    let speech = vowel(f0, intent.f1_hz, intent.f2_hz, intent.speech_s, SR);
    if !control.place_breaths || intent.breaths.is_empty() {
        return speech;
    }
    let mut out = Vec::new();
    for breath in &intent.breaths {
        let n = (breath.dur_s * SR).round().max(1.0) as usize;
        out.extend(breath_noise(n, 0.2));
    }
    out.extend(speech);
    out
}

pub fn render_external(
    cmd: &str,
    intent: &Intent,
    control: &Control,
    dir: &Path,
) -> Result<(Vec<f64>, f64), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let req_path = dir.join("request.json");
    let wav_path = dir.join("out.wav");
    let body =
        serde_json::to_string_pretty(&request_of(intent, control)).map_err(|e| e.to_string())?;
    std::fs::write(&req_path, body).map_err(|e| format!("{}: {e}", req_path.display()))?;
    let out = Command::new(cmd)
        .arg("--request")
        .arg(&req_path)
        .arg("--out")
        .arg(&wav_path)
        .env("HF_HUB_OFFLINE", "1")
        .env("HF_HUB_DISABLE_TELEMETRY", "1")
        .env("GALION_LISTEN_REQUEST", &req_path)
        .output()
        .map_err(|e| format!("render-cmd: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(format!("render-cmd failed: {err}"));
    }
    let (samples, sr) =
        galion_articulation::acoustic::read_wav_mono(&wav_path.display().to_string())?;
    let _ = std::fs::remove_dir_all(dir);
    Ok((samples, sr))
}

/// Glottal-like pulse train through two resonators. Same fixture shape as
/// the articulation contract tests. Not a voice.
fn vowel(f0: f64, f1: f64, f2: f64, secs: f64, sr: f64) -> Vec<f64> {
    let n = (secs * sr).round().max(1.0) as usize;
    let mut y = vec![0.0; n];
    let period = (sr / f0).round().max(1.0) as usize;
    for i in (0..n).step_by(period) {
        y[i] = 1.0;
    }
    for (f, bw) in [(f1, 80.0), (f2, 120.0)] {
        let r = (-std::f64::consts::PI * bw / sr).exp();
        let c = 2.0 * r * (2.0 * std::f64::consts::PI * f / sr).cos();
        let (mut y1, mut y2) = (0.0, 0.0);
        for v in y.iter_mut() {
            let o = *v + c * y1 - r * r * y2;
            y2 = y1;
            y1 = o;
            *v = o;
        }
    }
    let m = y.iter().fold(0.0f64, |a, b| a.max(b.abs()));
    if m <= 1e-12 {
        return y;
    }
    y.iter().map(|v| 0.5 * v / m).collect()
}

fn breath_noise(n: usize, amp: f64) -> Vec<f64> {
    let mut s: u64 = 13;
    let fade = ((0.005 * SR) as usize).clamp(1, n.max(1) / 4);
    (0..n)
        .map(|i| {
            s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let u = ((s >> 33) as f64) / (1u64 << 31) as f64;
            let env = if i < fade {
                i as f64 / fade as f64
            } else if i + fade >= n {
                (n - 1 - i) as f64 / fade as f64
            } else {
                1.0
            };
            (u * 2.0 - 1.0) * amp * env.max(0.0)
        })
        .collect()
}
