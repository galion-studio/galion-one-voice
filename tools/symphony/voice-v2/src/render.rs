// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Render. The existing Chatterbox path is `cbx speak` (tools/cloning-v2/pipeline/cbx-v2).
//! It is spawned only when the binary, a voice, and a local fp32 snapshot are
//! already on disk. This module never downloads.
//!
//! Without those files the stand-in is a synthetic vowel at the target pitch.
//! It under-applies that pitch on purpose, so the one correction pass has an
//! error to close in CI. That undershoot is not a model of Chatterbox.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::audio::{read_wav, vowel};
use crate::device::DeviceChoice;
use crate::emotion::Feeling;
use crate::fluid::{TargetState, NEUTRAL_AROUSAL};
use crate::profile::{load_param_row, VoiceFile};

/// Semitones of pitch per unit of arousal away from neutral. Pipeline default.
const PITCH_PER_AROUSAL: f64 = 11.0;
/// Synthetic stand-in applies this fraction of the requested semitones.
const SYNTH_UNDERSHOOT: f64 = 0.5;
pub const SYNTH_SR: u32 = 16_000;
pub const SYNTH_SECS: f64 = 1.6;
/// `cbx speak --pitch-semitones` is documented as a mild shift, |st| <= 2.
const CBX_PITCH_LIMIT: f64 = 2.0;

const WEIGHT_MARKERS: [&str; 5] = [
    "tokenizer.json",
    "onnx/speech_encoder.onnx",
    "onnx/conditional_decoder.onnx",
    "onnx/embed_tokens.onnx",
    "onnx/language_model.onnx",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RendererKind {
    Synthetic,
    Chatterbox,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatterboxParams {
    /// Added on top of [`ChatterboxParams::plan_pitch_semitones`] by the correction.
    pub pitch_semitones: f32,
    pub plan_pitch_semitones: f32,
    pub pace: f32,
    pub temperature: f32,
    pub top_p: f32,
    pub repetition_penalty: f32,
    /// Passed through to `cbx`. Turbo ONNX ignores it; the CLI still accepts it.
    pub exaggeration: f32,
    pub source: String,
}

impl ChatterboxParams {
    pub fn requested_semitones(&self) -> f64 {
        self.plan_pitch_semitones as f64 + self.pitch_semitones as f64
    }
}

#[derive(Clone, Debug)]
pub struct Rendered {
    pub samples: Vec<f64>,
    pub sample_rate_hz: u32,
    pub kind: RendererKind,
    pub note: String,
    /// Pitch the renderer was asked for, before the synthetic undershoot.
    pub requested_f0_hz: f64,
}

pub fn goal_pitch_st(target: &TargetState) -> f64 {
    let delta = target.goal_avd().arousal - NEUTRAL_AROUSAL;
    (delta * PITCH_PER_AROUSAL).clamp(-6.0, 6.0)
}

pub fn f0_from_st(baseline_hz: f64, semitones: f64) -> f64 {
    baseline_hz * 2.0_f64.powf(semitones / 12.0)
}

pub fn params_for(voice: &VoiceFile, target: &TargetState) -> Result<ChatterboxParams, String> {
    let name = target.feeling.as_str();
    let row = load_param_row(&voice.chatterbox.params_json, name)?;
    let from_file = row.as_ref().and_then(|r| r.pitch_semitones);
    let (plan, source) = match from_file {
        Some(st) => (st as f64, "chatterbox-params.json"),
        None => (goal_pitch_st(target), "pipeline-default"),
    };
    let arousal = target.goal_avd().arousal as f32;
    let pace = row
        .as_ref()
        .and_then(|r| r.pace)
        .unwrap_or(0.85 + arousal * 0.30)
        .clamp(0.8, 1.2);
    Ok(ChatterboxParams {
        pitch_semitones: 0.0,
        plan_pitch_semitones: plan.clamp(-6.0, 6.0) as f32,
        pace,
        temperature: row.as_ref().and_then(|r| r.temperature).unwrap_or(0.0),
        top_p: row
            .as_ref()
            .and_then(|r| r.top_p)
            .unwrap_or(1.0)
            .clamp(0.0, 1.0),
        repetition_penalty: row
            .as_ref()
            .and_then(|r| r.repetition_penalty)
            .unwrap_or(1.2),
        exaggeration: row.as_ref().and_then(|r| r.exaggeration).unwrap_or(0.5),
        source: source.into(),
    })
}

/// The object PR #192 (`galion-emotion-render`) told this pipeline to merge
/// into the clone worker request. Same keys as `clone_request_patch` there.
pub fn clone_request_patch(target: &TargetState, params: &ChatterboxParams) -> serde_json::Value {
    let track = match target.track {
        crate::emotion::Track::Prosody => "prosody",
        crate::emotion::Track::Burst => "burst",
    };
    serde_json::json!({
        "emotion": target.feeling.as_str(),
        "track": track,
        "exaggeration": params.exaggeration,
        "cfg_weight": 0.0,
        "temperature": params.temperature,
        "prosody": {
            "pitch_st": params.plan_pitch_semitones,
            "range": 1.0,
            "rate": params.pace,
            "energy": 1.0,
            "pre_pause_ms": 0
        }
    })
}

/// When `GALION_EMOTION_RENDER` is the `galion-emotion-render` binary from
/// PR #192, its controls replace the pipeline-default pitch. A missing binary
/// leaves the stand-in in place. This never downloads.
pub fn apply_emotion_render(
    target: &TargetState,
    text: &str,
    params: &mut ChatterboxParams,
) -> bool {
    let Ok(bin) = std::env::var("GALION_EMOTION_RENDER") else {
        return false;
    };
    let bin = bin.trim();
    if bin.is_empty() || !std::path::Path::new(bin).is_file() {
        return false;
    }
    let avd = target.goal_avd();
    let track = match target.track {
        crate::emotion::Track::Prosody => "prosody",
        crate::emotion::Track::Burst => "burst",
    };
    let tick = serde_json::json!({
        "t_ms": 0,
        "track": track,
        "emotion": target.feeling.as_str(),
        "intensity": target.intensity.clamp(0.0, 1.0),
        "arousal": avd.arousal.clamp(-1.0, 1.0),
        "valence": avd.valence.clamp(-1.0, 1.0),
        "dominance": (avd.dominance * 2.0 - 1.0).clamp(-1.0, 1.0),
        "text": text,
    });
    let mut child = match std::process::Command::new(bin)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => return false,
    };
    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        if stdin.write_all(tick.to_string().as_bytes()).is_err() {
            let _ = child.kill();
            let _ = child.wait();
            return false;
        }
    }
    let output = match child.wait_with_output() {
        Ok(output) if output.status.success() => output,
        _ => return false,
    };
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&output.stdout) else {
        return false;
    };
    let controls = value.get("controls").unwrap_or(&value);
    let Some(pitch) = controls
        .pointer("/prosody/pitch_st")
        .and_then(|v| v.as_f64())
    else {
        return false;
    };
    let rate = controls
        .pointer("/prosody/rate")
        .and_then(|v| v.as_f64())
        .unwrap_or(1.0);
    params.plan_pitch_semitones = pitch.clamp(-6.0, 6.0) as f32;
    params.pace = (rate as f32).clamp(0.8, 1.2);
    if let Some(ex) = controls
        .pointer("/chatterbox/exaggeration")
        .and_then(|v| v.as_f64())
    {
        params.exaggeration = ex as f32;
    }
    if let Some(temp) = controls
        .pointer("/chatterbox/temperature")
        .and_then(|v| v.as_f64())
    {
        params.temperature = temp as f32;
    }
    params.source = "galion-emotion-render".into();
    true
}

pub fn correct_pitch(params: &ChatterboxParams, error_semitones: Option<f64>) -> ChatterboxParams {
    let mut next = params.clone();
    if let Some(err) = error_semitones {
        if err.is_finite() {
            let total = (params.requested_semitones() - err).clamp(-12.0, 12.0);
            next.pitch_semitones = (total - params.plan_pitch_semitones as f64) as f32;
        }
    }
    next
}

pub fn render(
    text: &str,
    voice: &VoiceFile,
    params: &ChatterboxParams,
    device: &DeviceChoice,
) -> Result<Rendered, String> {
    let requested_f0_hz = f0_from_st(voice.baseline.f0_hz_mean, params.requested_semitones());
    if let Some(rendered) = render_cbx(text, voice, params, device, requested_f0_hz)? {
        return Ok(rendered);
    }
    Ok(render_synthetic(voice, params, requested_f0_hz))
}

fn render_synthetic(
    voice: &VoiceFile,
    params: &ChatterboxParams,
    requested_f0_hz: f64,
) -> Rendered {
    let applied = params.requested_semitones() * SYNTH_UNDERSHOOT;
    let f0 = f0_from_st(voice.baseline.f0_hz_mean, applied).clamp(70.0, 500.0);
    let f1 = if voice.baseline.f1_hz_mean > 50.0 {
        voice.baseline.f1_hz_mean
    } else {
        700.0
    };
    let f2 = if voice.baseline.f2_hz_mean > 50.0 {
        voice.baseline.f2_hz_mean
    } else {
        1200.0
    };
    let samples = vowel(f0, f1, f2, SYNTH_SECS, SYNTH_SR as f64);
    Rendered {
        samples,
        sample_rate_hz: SYNTH_SR,
        kind: RendererKind::Synthetic,
        note: format!(
            "synthetic vowel, undershoot {SYNTH_UNDERSHOOT}. cbx was not started (no local binary, voice, and fp32 snapshot). No download."
        ),
        requested_f0_hz,
    }
}

fn render_cbx(
    text: &str,
    voice: &VoiceFile,
    params: &ChatterboxParams,
    device: &DeviceChoice,
    requested_f0_hz: f64,
) -> Result<Option<Rendered>, String> {
    let slot = &voice.chatterbox;
    let bin = slot.bin.trim();
    if bin.is_empty() {
        return Ok(None);
    }
    let bin_path = PathBuf::from(bin);
    if !bin_path.is_file() {
        return Ok(None);
    }
    let weights = match weights_dir(slot) {
        Some(dir) if weights_ready(&dir) => dir,
        _ => return Ok(None),
    };
    if slot.voice.trim().is_empty() && slot.voice_wav.trim().is_empty() {
        return Ok(None);
    }
    let out = std::env::temp_dir().join(format!("galion-voice-v2-cbx-{}.wav", std::process::id()));
    let pitch = params
        .requested_semitones()
        .clamp(-CBX_PITCH_LIMIT, CBX_PITCH_LIMIT);
    let mut cmd = Command::new(&bin_path);
    cmd.arg("--ep")
        .arg(&device.cbx_ep)
        .arg("speak")
        .arg("--repo-id")
        .arg(&slot.repo_id)
        .arg("--revision")
        .arg(&slot.revision)
        .arg("--dtype")
        .arg(&slot.dtype)
        .arg("--text")
        .arg(text)
        .arg("--out-wav")
        .arg(&out)
        .arg("--pace")
        .arg(format!("{:.4}", params.pace))
        .arg("--pitch-semitones")
        .arg(format!("{pitch:.4}"))
        .arg("--temperature")
        .arg(format!("{:.4}", params.temperature))
        .arg("--top-p")
        .arg(format!("{:.4}", params.top_p))
        .arg("--repetition-penalty")
        .arg(format!("{:.4}", params.repetition_penalty))
        .arg("--exaggeration")
        .arg(format!("{:.4}", params.exaggeration))
        .env("HF_HUB_OFFLINE", "1")
        .env("HF_HUB_DISABLE_TELEMETRY", "1")
        .env("GALION_CBX_WEIGHTS", &weights)
        .env("GALION_VOICE_DEVICE", &device.cbx_ep);
    if !slot.voice_wav.trim().is_empty() {
        cmd.arg("--voice-wav").arg(slot.voice_wav.trim());
    } else {
        cmd.arg("--voice").arg(slot.voice.trim());
    }
    let output = cmd.output().map_err(|e| format!("cbx: {e}"))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        let tail: String = err
            .chars()
            .rev()
            .take(800)
            .collect::<String>()
            .chars()
            .rev()
            .collect();
        let _ = std::fs::remove_file(&out);
        return Err(format!("cbx speak failed: {tail}"));
    }
    let (samples, sr) = read_wav(&out)?;
    let _ = std::fs::remove_file(&out);
    let note = if (params.requested_semitones() - pitch).abs() > 0.05 {
        format!(
            "cbx speak --ep {}. Pitch clamped from {:.2} st to {pitch:.2} st (|st| <= 2 on that CLI). Weights {}.",
            device.cbx_ep,
            params.requested_semitones(),
            weights.display()
        )
    } else {
        format!(
            "cbx speak --ep {}. Weights {}.",
            device.cbx_ep,
            weights.display()
        )
    };
    Ok(Some(Rendered {
        samples,
        sample_rate_hz: sr.round() as u32,
        kind: RendererKind::Chatterbox,
        note,
        requested_f0_hz,
    }))
}

fn weights_dir(slot: &crate::profile::ChatterboxSlot) -> Option<PathBuf> {
    let raw = if slot.weights.trim().is_empty() {
        std::env::var("GALION_CBX_WEIGHTS").unwrap_or_default()
    } else {
        slot.weights.clone()
    };
    let raw = raw.trim();
    if raw.is_empty() {
        None
    } else {
        Some(PathBuf::from(raw))
    }
}

fn weights_ready(dir: &Path) -> bool {
    WEIGHT_MARKERS.iter().all(|rel| dir.join(rel).is_file())
}

/// What the diff compares. `measured` is the decoder's median F0.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Diff {
    pub intended_f0_hz: f64,
    pub measured_f0_hz: Option<f64>,
    pub error_semitones: Option<f64>,
    pub intended: Feeling,
    pub intended_track: crate::emotion::Track,
    pub read_prosody: Option<crate::emotion::EmotionId>,
    pub read_burst: Option<crate::emotion::EmotionId>,
}

pub fn diff_f0(
    intended_f0_hz: f64,
    measured_f0_hz: Option<f64>,
    target: &TargetState,
    profile: &crate::profile::VoiceProfile,
) -> Diff {
    let error_semitones = measured_f0_hz.and_then(|m| {
        if m > 1.0 && intended_f0_hz > 1.0 {
            Some(12.0 * (m / intended_f0_hz).log2())
        } else {
            None
        }
    });
    Diff {
        intended_f0_hz,
        measured_f0_hz,
        error_semitones,
        intended: target.feeling,
        intended_track: target.track,
        read_prosody: profile.modal_prosody(),
        read_burst: profile.modal_burst(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emotion::EmotionId;
    use crate::fluid::BaselineVoice;

    #[test]
    fn one_correction_closes_the_synthetic_undershoot() {
        let voice = VoiceFile::synthetic();
        let target = TargetState::spoken(EmotionId::Embarrassment, 1.0);
        let params = params_for(&voice, &target).unwrap();
        assert_eq!(params.source, "pipeline-default");
        let patch = clone_request_patch(&target, &params);
        assert_eq!(patch["emotion"], "embarrassment");
        assert_eq!(patch["track"], "prosody");
        assert!(patch["prosody"]["pitch_st"].is_number());
        let requested = params.requested_semitones();
        assert!(requested > 3.0, "{requested}");
        let measured_st = requested * SYNTH_UNDERSHOOT;
        let measured_hz = f0_from_st(BaselineVoice::default().f0_hz_mean, measured_st);
        let intended_hz = f0_from_st(BaselineVoice::default().f0_hz_mean, requested);
        let err = 12.0 * (measured_hz / intended_hz).log2();
        let next = correct_pitch(&params, Some(err));
        let requested2 = next.requested_semitones();
        let measured2 = f0_from_st(
            BaselineVoice::default().f0_hz_mean,
            requested2 * SYNTH_UNDERSHOOT,
        );
        let before = (measured_hz - intended_hz).abs();
        let after = (measured2 - intended_hz).abs();
        assert!(after + 5.0 < before, "before {before} after {after}");
    }
}
