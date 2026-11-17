// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Integrator for the mouth-and-expressions stack (Swarm 13, S8).
//!
//! This crate does not implement the contract, the feeling, the throat,
//! the mouth, the nose, the cloner adapters, or the closed eval. Those are
//! S1–S7. Until their crates land, `run` calls the drafts already on this
//! branch: lungs (#195), articulation decode (#191), and emotion-render (#192).
//!
//! S1 owns `galion-mne-contract`. This binary's `--text` and `--pin` flags
//! are a temporary stand-in for that contract, not a second schema.

#![forbid(unsafe_code)]

mod stimulus;

use std::path::{Path, PathBuf};

use galion_articulation::closed_loop::{self, LoopReport};
use galion_emotion_render::{
    clone_request_patch, load_scales, render_tick, Avd, TargetTick, Track,
};
use galion_lungs::plan::{self, Plan, Request as BreathRequest};
use galion_lungs::synth::{self, Rendered as BreathAudio};
use galion_lungs::{config::Config, inventory::Inventory};
use serde::Serialize;
use serde_json::Value;

pub const TICK_MS: u32 = 80;

/// Where each layer owner is expected to land a crate. Paths are relative to the repo root.
pub const SLOTS: &[LayerSlot] = &[
    LayerSlot {
        owner: "S1",
        role: "contract and fixtures",
        crate_name: "galion-mne-contract",
        path: "tools/symphony/mne-contract",
    },
    LayerSlot {
        owner: "S2",
        role: "feeling",
        crate_name: "galion-mne-feeling",
        path: "tools/symphony/mne-feeling",
    },
    LayerSlot {
        owner: "S3",
        role: "throat",
        crate_name: "galion-mne-throat",
        path: "tools/symphony/mne-throat",
    },
    LayerSlot {
        owner: "S4",
        role: "mouth",
        crate_name: "galion-mne-mouth",
        path: "tools/symphony/mne-mouth",
    },
    LayerSlot {
        owner: "S5",
        role: "nose and wordless sounds",
        crate_name: "galion-mne-nose",
        path: "tools/symphony/mne-nose",
    },
    LayerSlot {
        owner: "S6",
        role: "sound out",
        crate_name: "galion-mne-sound",
        path: "tools/symphony/mne-sound",
    },
    LayerSlot {
        owner: "S7",
        role: "listening back",
        crate_name: "galion-mne-listen",
        path: "tools/symphony/mne-listen",
    },
];

#[derive(Clone, Copy, Debug, Serialize)]
pub struct LayerSlot {
    pub owner: &'static str,
    pub role: &'static str,
    pub crate_name: &'static str,
    pub path: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct LayerStatus {
    pub owner: &'static str,
    pub role: &'static str,
    pub crate_name: &'static str,
    pub path: &'static str,
    /// `waiting` until that owner's Cargo.toml is in the tree. `landed` once it is.
    pub state: &'static str,
    /// What this integrator calls today for that role.
    pub wired_to: &'static str,
}

pub fn layer_status(repo: &Path) -> Vec<LayerStatus> {
    SLOTS
        .iter()
        .map(|slot| {
            let landed = repo.join(slot.path).join("Cargo.toml").is_file();
            LayerStatus {
                owner: slot.owner,
                role: slot.role,
                crate_name: slot.crate_name,
                path: slot.path,
                state: if landed { "landed" } else { "waiting" },
                wired_to: wired_today(slot.owner),
            }
        })
        .collect()
}

fn wired_today(owner: &str) -> &'static str {
    match owner {
        "S1" => "CLI flags --text and --pin, until galion-mne-contract lands",
        "S2" => "galion-emotion-render (PR #192); no mood model in this crate",
        "S3" => "galion-articulation throat tracks (PR #191)",
        "S4" => "galion-articulation mouth tracks (PR #191)",
        "S5" => "galion-articulation nose section (PR #191); wordless sounds not invented here",
        "S6" => "galion-articulation closed_loop stand-in renderer, until galion-mne-sound lands",
        "S7" => "the same closed_loop read-back, one extra pass (max_iter = 2)",
        _ => "none",
    }
}

pub struct RunIn<'a> {
    pub repo: &'a Path,
    pub text: &'a str,
    pub pin: &'a str,
    pub voice: &'a Path,
}

#[derive(Serialize)]
pub struct RunReport {
    pub schema: &'static str,
    pub tick_ms: u32,
    pub layers: Vec<LayerStatus>,
    pub voice_f0_hz: f64,
    pub lungs: Plan,
    pub controls: Value,
    pub clone_request_patch: Value,
    pub listen: LoopReport,
    pub samples: usize,
    pub sample_rate_hz: u32,
    #[serde(skip)]
    pub audio: Vec<f64>,
}

pub fn run(input: &RunIn<'_>) -> Result<RunReport, String> {
    if input.text.trim().is_empty() {
        return Err("text is empty".into());
    }
    let voice_f0_hz = baseline_f0(input.voice)?;
    let cfg = Config::bundled()?;
    let inventory = Inventory::bundled()?;
    let breath_req = BreathRequest::constant(input.text, input.pin, 0.0, 0.5, 0.0);
    let lungs = plan::plan(&breath_req, &cfg, &inventory)?;
    let breath: BreathAudio = synth::render_track(&lungs, stimulus::SR, None, &cfg)?;

    let scales = load_scales(&input.repo.join("configs/symphony/emotion-render.toml"))?;
    let tick = TargetTick::from_name(
        input.pin,
        Some(Track::Prosody),
        1.0,
        Avd {
            arousal: 0.0,
            valence: 0.0,
            dominance: 0.0,
        },
    )
    .map_err(|e| e.to_string())?;
    let controls = render_tick(&tick, &scales).map_err(|e| e.to_string())?;
    let patch = clone_request_patch(&controls);

    let tone = stimulus::vowel(
        voice_f0_hz,
        700.0,
        1200.0,
        lungs.duration_s.max(0.8),
        stimulus::SR,
    );
    let mixed = mix_breath(&tone, &breath.samples);
    let listen = closed_loop::run_standin(
        &mixed,
        stimulus::SR,
        &closed_loop::Request {
            emotion: Some(input.pin),
            env_name: None,
            enrol: None,
            max_iter: 2,
            start_exaggeration: None,
        },
    )?;

    Ok(RunReport {
        schema: "galion-mne-run/1",
        tick_ms: TICK_MS,
        layers: layer_status(input.repo),
        voice_f0_hz,
        lungs,
        controls: serde_json::to_value(&controls).map_err(|e| e.to_string())?,
        clone_request_patch: patch,
        listen,
        samples: mixed.len(),
        sample_rate_hz: stimulus::SR.round() as u32,
        audio: mixed,
    })
}

fn baseline_f0(path: &Path) -> Result<f64, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let value: toml::Value =
        toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let f0 = value
        .get("baseline")
        .and_then(|b| b.get("f0_hz_mean"))
        .and_then(toml::Value::as_float)
        .ok_or_else(|| format!("{}: need baseline.f0_hz_mean", path.display()))?;
    if f0 < 40.0 || f0 > 500.0 {
        return Err(format!(
            "{}: baseline.f0_hz_mean {f0} is outside 40..500",
            path.display()
        ));
    }
    Ok(f0)
}

/// Speech tone everywhere the lungs track is silent. Breath samples where the
/// lungs renderer actually made air. Not a cloner.
fn mix_breath(speech: &[f64], breath: &[f64]) -> Vec<f64> {
    let n = speech.len().max(breath.len());
    let mut out = vec![0.0; n];
    for i in 0..n {
        let air = breath.get(i).copied().unwrap_or(0.0);
        let voice = speech.get(i).copied().unwrap_or(0.0);
        out[i] = if air.abs() > 1e-5 { air } else { voice };
    }
    out
}

pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

pub fn write_wav(path: &Path, samples: &[f64], sr: u32) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
    }
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: sr,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).map_err(|e| e.to_string())?;
    for s in samples {
        let v = (s.clamp(-1.0, 1.0) * 32767.0).round() as i16;
        w.write_sample(v).map_err(|e| e.to_string())?;
    }
    w.finalize().map_err(|e| e.to_string())
}
