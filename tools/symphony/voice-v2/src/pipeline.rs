// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! The one pipeline, and the call the Symphony stack makes.
//!
//! Write the feeling, render it, read the audio back with the same decoder,
//! then one correction. The 80 ms heartbeat does not call this. A turn does.
//! Inference stays out of `Heartbeat::step`.

use std::path::Path;
use std::process::Command;

use serde::Serialize;

use crate::audio::{apply_gain, read_wav, write_wav};
use crate::decode::{decode_samples, median_f0, DecodeIn};
use crate::device::{detect_with, DeviceChoice};
use crate::fluid::{FluidCore, TargetState, TickFeeling};
use crate::profile::VoiceFile;
use crate::profile::VoiceProfile;
use crate::render::{
    correct_pitch, diff_f0, params_for, render, ChatterboxParams, Diff, Rendered, RendererKind,
};
use crate::TICK_MS;

/// Text plus a target state in. Audio plus the read-back state out.
pub struct SpeakIn<'a> {
    pub text: &'a str,
    pub target: &'a TargetState,
    pub voice: &'a VoiceFile,
    /// `cpu`, `cuda`, `rocm`, `auto`, or None to detect.
    pub device: Option<&'a str>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SpeakOut {
    #[serde(skip)]
    pub samples: Vec<f32>,
    pub sample_rate_hz: u32,
    pub readback: VoiceProfile,
    pub write_ticks: Vec<TickFeeling>,
    pub diff_before: Diff,
    pub diff_after: Diff,
    pub device: DeviceChoice,
    pub renderer: RendererKind,
    pub renderer_note: String,
    pub params: ChatterboxParams,
    pub corrected_params: ChatterboxParams,
    pub stubs: Vec<String>,
}

pub fn speak(req: &SpeakIn<'_>) -> Result<SpeakOut, String> {
    let n_chars = req.text.chars().count();
    if n_chars == 0 || req.text.trim().is_empty() {
        return Err("speak: text is empty".into());
    }
    if n_chars > 600 {
        return Err(
            "speak: text is longer than 600 characters, the existing clone worker's limit".into(),
        );
    }
    let device = detect_with(req.device);
    let params = params_for(req.voice, req.target)?;
    // The feeling's pitch stays the target. The correction changes what we ask
    // the renderer for; it does not move the goal both passes are scored against.
    let goal_f0 = crate::render::f0_from_st(
        req.voice.baseline.f0_hz_mean,
        params.plan_pitch_semitones as f64,
    );
    let first = render_mix_read(req, &params, &device)?;
    let diff_before = diff_f0(
        goal_f0,
        median_f0(&first.readback),
        req.target,
        &first.readback,
    );
    let corrected = correct_pitch(&params, diff_before.error_semitones);
    let second = render_mix_read(req, &corrected, &device)?;
    let diff_after = diff_f0(
        goal_f0,
        median_f0(&second.readback),
        req.target,
        &second.readback,
    );
    let write_ticks = plan_ticks(req.voice, req.target, second.readback.n_ticks);
    let mut stubs = second.readback.stubs.clone();
    stubs.push(second.rendered.note.clone());
    if params.source == "pipeline-default" {
        stubs.push(
            "TODO(features): Chatterbox params are a pipeline default. Drop a galion-voice-v2-chatterbox-params file on chatterbox.params_json to replace them."
                .into(),
        );
    }
    stubs.push(
        "TODO(features): real-life isolate is the existing refprep plus DeepFilterNet, Demucs, pyannote and Silero VAD. Set GALION_REFPREP to a refprep binary. This speak() call does not clean."
            .into(),
    );
    Ok(SpeakOut {
        samples: second.samples.iter().map(|s| *s as f32).collect(),
        sample_rate_hz: second.rendered.sample_rate_hz,
        readback: second.readback,
        write_ticks,
        diff_before,
        diff_after,
        device,
        renderer: second.rendered.kind,
        renderer_note: second.rendered.note,
        params,
        corrected_params: corrected,
        stubs,
    })
}

struct Pass {
    samples: Vec<f64>,
    rendered: Rendered,
    readback: VoiceProfile,
}

fn render_mix_read(
    req: &SpeakIn<'_>,
    params: &ChatterboxParams,
    device: &DeviceChoice,
) -> Result<Pass, String> {
    let rendered = render(req.text, req.voice, params, device)?;
    let mut samples = rendered.samples.clone();
    let gain = req
        .target
        .outer
        .distance_gain(req.voice.baseline.conditions)
        * req.target.outer.room.gain();
    apply_gain(&mut samples, gain);
    let readback = decode_samples(DecodeIn {
        id: &req.voice.id,
        role: "read",
        samples: &samples,
        sample_rate_hz: rendered.sample_rate_hz as f64,
        voice: req.voice,
        outer: req.target.outer,
        inner: req.target.inner,
    })?;
    Ok(Pass {
        samples,
        rendered,
        readback,
    })
}

fn plan_ticks(voice: &VoiceFile, target: &TargetState, n: usize) -> Vec<TickFeeling> {
    let mut core = FluidCore::new(voice.baseline.clone());
    (0..n.max(1))
        .map(|i| core.step((i as u32).saturating_mul(TICK_MS), target))
        .collect()
}

pub struct LoopIn<'a> {
    pub reference_wav: &'a Path,
    pub text: &'a str,
    pub target: &'a TargetState,
    pub voice: &'a VoiceFile,
    pub out_dir: &'a Path,
    pub device: Option<&'a str>,
}

#[derive(Serialize)]
pub struct LoopReport {
    pub schema: u32,
    pub cleaner: String,
    pub reference: VoiceProfile,
    pub speak: SpeakOut,
    pub stubs: Vec<String>,
}

pub fn run_loop(req: &LoopIn<'_>) -> Result<LoopReport, String> {
    std::fs::create_dir_all(req.out_dir).map_err(|e| format!("{}: {e}", req.out_dir.display()))?;
    let cleaned = req.out_dir.join("cleaned.wav");
    let cleaner = clean(req.reference_wav, &cleaned)?;
    let (samples, sr) = read_wav(&cleaned)?;
    let reference = decode_samples(DecodeIn {
        id: &req.voice.id,
        role: "reference",
        samples: &samples,
        sample_rate_hz: sr,
        voice: req.voice,
        outer: req.voice.baseline.conditions,
        inner: req.target.inner,
    })?;
    write_json(&req.out_dir.join("profile.json"), &reference)?;
    let speak_out = speak(&SpeakIn {
        text: req.text,
        target: req.target,
        voice: req.voice,
        device: req.device,
    })?;
    write_wav(
        &req.out_dir.join("corrected.wav"),
        &f64_from(&speak_out.samples),
        speak_out.sample_rate_hz,
    )?;
    write_json(&req.out_dir.join("readback.json"), &speak_out.readback)?;
    write_json(
        &req.out_dir.join("write-ticks.json"),
        &speak_out.write_ticks,
    )?;
    let mut stubs = Vec::new();
    if cleaner.starts_with("passthrough") {
        stubs.push(
            "TODO(features): cleaner is a passthrough. Wire GALION_REFPREP at the existing refprep binary; DeepFilterNet, Demucs, pyannote and Silero VAD stay on that agent's isolate command."
                .into(),
        );
    }
    stubs.extend(speak_out.stubs.clone());
    stubs.extend(reference.stubs.clone());
    let report = LoopReport {
        schema: 1,
        cleaner,
        reference,
        speak: speak_out,
        stubs,
    };
    write_json(&req.out_dir.join("report.json"), &report)?;
    Ok(report)
}

fn f64_from(samples: &[f32]) -> Vec<f64> {
    samples.iter().map(|s| *s as f64).collect()
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
}

fn clean(input: &Path, output: &Path) -> Result<String, String> {
    match std::env::var("GALION_REFPREP") {
        Ok(bin) if !bin.trim().is_empty() => run_refprep(bin.trim(), input, output),
        _ => {
            let (samples, sr) = read_wav(input)?;
            write_wav(output, &samples, sr.round() as u32)?;
            Ok("passthrough".into())
        }
    }
}

fn run_refprep(bin: &str, input: &Path, output: &Path) -> Result<String, String> {
    let meta = output.with_extension("refprep.json");
    let out = Command::new(bin)
        .args(["prep", "--input"])
        .arg(input)
        .arg("--output")
        .arg(output)
        .arg("--meta")
        .arg(&meta)
        .env("HF_HUB_OFFLINE", "1")
        .env("HF_HUB_DISABLE_TELEMETRY", "1")
        .output()
        .map_err(|e| format!("refprep: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(format!("refprep failed: {err}"));
    }
    Ok(format!("refprep {bin}"))
}
