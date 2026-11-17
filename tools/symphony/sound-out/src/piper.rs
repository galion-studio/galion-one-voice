// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Piper adapter.
//!
//! CI uses a stand-in vowel. It applies the same knobs the existing mouth
//! applies after Piper: `length_scale` is `1/rate`, pitch is a resample of
//! the tone's F0, energy is amplitude, `pre_pause_ms` is leading silence.
//! Piper itself reads `length_scale` once at spawn (`galion-ml-voice`
//! `PiperWorker`). The stand-in does not spawn that process and does not
//! load an ONNX pack.
//!
//! `PiperBackend::real_if_present` looks for a binary and a voice pack that
//! are already on disk. It does not download them.

use crate::backend::{emit_frames, Capabilities, ClonerBackend, PcmChunk, SpeakMeta, SAMPLE_RATE};
use crate::request::CloneRequest;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

pub struct PiperBackend {
    real: Option<RealPaths>,
}

struct RealPaths {
    runtime: PathBuf,
    model: PathBuf,
    config: PathBuf,
    sample_rate: u32,
}

impl PiperBackend {
    pub fn stand_in() -> Self {
        Self { real: None }
    }

    /// Real Piper only when `GALION_PIPER_RUNTIME`, `GALION_PIPER_MODEL` and
    /// `GALION_PIPER_CONFIG` already point at files. Otherwise an error.
    pub fn real_if_present() -> Result<Self, String> {
        let runtime = env_path("GALION_PIPER_RUNTIME")?;
        let model = env_path("GALION_PIPER_MODEL")?;
        let config = env_path("GALION_PIPER_CONFIG")?;
        if !runtime.is_file() || !model.is_file() || !config.is_file() {
            return Err(
                "piper runtime, model or config is not on disk (not downloaded)".into(),
            );
        }
        let sample_rate = read_sample_rate(&config).unwrap_or(22_050);
        Ok(Self {
            real: Some(RealPaths {
                runtime,
                model,
                config,
                sample_rate,
            }),
        })
    }
}

impl ClonerBackend for PiperBackend {
    fn id(&self) -> &'static str {
        if self.real.is_some() {
            "piper"
        } else {
            "piper-stand-in"
        }
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            realtime_on_cpu: true,
            streaming: true,
            weights_used: self.real.is_some(),
            stub: self.real.is_none(),
            note: if self.real.is_some() {
                "Piper VITS, out of process, CPU. Frames are handed off as they are read.".into()
            } else {
                "No ONNX pack. A vowel stands in for Piper and is realtime on CPU.".into()
            },
        }
    }

    fn speak_stream(
        &mut self,
        request: &CloneRequest,
        on_chunk: &mut dyn FnMut(PcmChunk) -> Result<(), String>,
    ) -> Result<SpeakMeta, String> {
        let req = request.clone().normalized();
        let samples = if let Some(paths) = &self.real {
            speak_real(paths, &req)?
        } else {
            stand_in_samples(&req)
        };
        emit_frames(&samples, on_chunk)?;
        Ok(SpeakMeta {
            backend: self.id().into(),
            weights_used: self.real.is_some(),
            stub: self.real.is_none(),
            realtime_on_cpu: true,
            controls_echo: None,
            note: if self.real.is_some() {
                "piper process".into()
            } else {
                "piper stand-in, no weights".into()
            },
        })
    }
}

/// Duration follows word count and `1/rate`. Pitch moves F0. Energy moves
/// amplitude. The pause is silence in front. Not a VITS sample.
pub fn stand_in_samples(req: &CloneRequest) -> Vec<f64> {
    let req = req.clone().normalized();
    let words = req
        .text
        .split_whitespace()
        .filter(|w| !w.is_empty())
        .count()
        .max(1);
    let length_scale = 1.0 / req.prosody.rate;
    let speech_s = words as f64 * 0.22 * length_scale;
    let n = (speech_s * SAMPLE_RATE as f64).round().max(1.0) as usize;
    let f0 = 180.0 * (req.prosody.pitch_st / 12.0).exp2();
    let amp = 0.2 * req.prosody.energy;
    let mut samples = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f64 / SAMPLE_RATE as f64;
        samples.push((2.0 * std::f64::consts::PI * f0 * t).sin() * amp);
    }
    let pause_n = ((req.prosody.pre_pause_ms as u64 * u64::from(SAMPLE_RATE)) / 1000) as usize;
    let mut out = vec![0.0; pause_n];
    out.append(&mut samples);
    out
}

fn speak_real(paths: &RealPaths, req: &CloneRequest) -> Result<Vec<f64>, String> {
    let length_scale = format!("{:.4}", 1.0 / req.prosody.rate);
    let silence = format!("{:.4}", req.prosody.pre_pause_ms as f64 / 1000.0);
    let mut child = Command::new(&paths.runtime)
        .arg("--model")
        .arg(&paths.model)
        .arg("--config")
        .arg(&paths.config)
        .arg("--output_raw")
        .arg("--json-input")
        .arg("--length_scale")
        .arg(&length_scale)
        .arg("--sentence_silence")
        .arg(&silence)
        .arg("--noise_scale")
        .arg("0")
        .arg("--noise_w")
        .arg("0")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("piper spawn: {e}"))?;
    {
        let mut stdin = child.stdin.take().ok_or("piper stdin missing")?;
        let line = serde_json::json!({"text": req.text}).to_string();
        stdin
            .write_all(line.as_bytes())
            .and_then(|_| stdin.write_all(b"\n"))
            .map_err(|e| format!("piper stdin: {e}"))?;
    }
    let mut stdout = child.stdout.take().ok_or("piper stdout missing")?;
    let mut pcm = Vec::new();
    stdout
        .read_to_end(&mut pcm)
        .map_err(|e| format!("piper stdout: {e}"))?;
    let status = child.wait().map_err(|e| format!("piper wait: {e}"))?;
    if !status.success() {
        return Err(format!("piper exited {status}"));
    }
    let mut samples = s16le_to_f64(&pcm);
    if paths.sample_rate != SAMPLE_RATE && paths.sample_rate > 0 {
        samples = resample(&samples, paths.sample_rate, SAMPLE_RATE);
    }
    apply_pitch_and_energy(&mut samples, req.prosody.pitch_st, req.prosody.energy);
    Ok(samples)
}

fn apply_pitch_and_energy(samples: &mut Vec<f64>, pitch_st: f64, energy: f64) {
    if (energy - 1.0).abs() > 1e-4 {
        for s in samples.iter_mut() {
            *s = (*s * energy).clamp(-1.0, 1.0);
        }
    }
    if pitch_st.abs() > 1e-4 && samples.len() > 1 {
        let ratio = (pitch_st / 12.0).exp2();
        if ratio.is_finite() && (ratio - 1.0).abs() > 1e-4 {
            let n = ((samples.len() as f64) / ratio).floor().max(1.0) as usize;
            let mut out = Vec::with_capacity(n);
            let last = samples.len() - 1;
            for i in 0..n {
                let pos = i as f64 * ratio;
                let i0 = (pos.floor() as usize).min(last);
                let frac = pos - i0 as f64;
                let a = samples[i0];
                let b = samples.get(i0 + 1).copied().unwrap_or(a);
                out.push(a + (b - a) * frac);
            }
            *samples = out;
        }
    }
}

fn resample(input: &[f64], from: u32, to: u32) -> Vec<f64> {
    if input.is_empty() || from == 0 {
        return Vec::new();
    }
    let n = ((input.len() as u64 * u64::from(to)) / u64::from(from)).max(1) as usize;
    let mut out = Vec::with_capacity(n);
    let last = input.len() - 1;
    for i in 0..n {
        let pos = i as f64 * from as f64 / to as f64;
        let i0 = (pos.floor() as usize).min(last);
        let frac = pos - i0 as f64;
        let a = input[i0];
        let b = input.get(i0 + 1).copied().unwrap_or(a);
        out.push(a + (b - a) * frac);
    }
    out
}

fn s16le_to_f64(pcm: &[u8]) -> Vec<f64> {
    let n = pcm.len() / 2;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let sample = i16::from_le_bytes([pcm[i * 2], pcm[i * 2 + 1]]);
        out.push(sample as f64 / 32768.0);
    }
    out
}

fn env_path(key: &str) -> Result<PathBuf, String> {
    match std::env::var(key) {
        Ok(v) if !v.trim().is_empty() => Ok(PathBuf::from(v.trim())),
        _ => Err(format!("{key} is unset")),
    }
}

fn read_sample_rate(config: &PathBuf) -> Option<u32> {
    let text = std::fs::read_to_string(config).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    v.get("audio")
        .and_then(|a| a.get("sample_rate"))
        .and_then(|n| n.as_u64())
        .map(|n| n as u32)
}
