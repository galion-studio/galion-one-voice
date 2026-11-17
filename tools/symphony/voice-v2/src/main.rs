// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! galion-voice-v2 decode | speak | loop
//!
//!   decode --wav IN.wav --voice VOICE.toml --out profile.json
//!   speak  --text "..." --emotion embarrassment --voice VOICE.toml --out out.wav --readback read.json
//!   loop   --wav IN.wav --text "..." --emotion embarrassment --voice VOICE.toml --out DIR
//!
//! --track prosody|burst   (default prosody). Every one of the 24 is legal on prosody.
//! --device cpu|cuda|rocm|auto

use galion_voice_v2::audio::write_wav;
use galion_voice_v2::emotion::{parse_target, Track};
use galion_voice_v2::fluid::TargetState;
use galion_voice_v2::pipeline::{run_loop, speak, LoopIn, SpeakIn};
use galion_voice_v2::profile::VoiceFile;
use std::collections::HashMap;
use std::path::PathBuf;

fn args() -> (String, HashMap<String, String>) {
    let mut it = std::env::args().skip(1);
    let cmd = it.next().unwrap_or_default();
    let mut m = HashMap::new();
    while let Some(k) = it.next() {
        if let Some(k) = k.strip_prefix("--") {
            m.insert(k.to_string(), it.next().unwrap_or_default());
        }
    }
    (cmd, m)
}

fn need<'a>(m: &'a HashMap<String, String>, key: &str) -> Result<&'a String, String> {
    m.get(key).ok_or_else(|| format!("--{key} required"))
}

fn target_from(m: &HashMap<String, String>) -> Result<TargetState, String> {
    let name = need(m, "emotion")?;
    let track = match m.get("track").map(String::as_str).unwrap_or("prosody") {
        "prosody" => Track::Prosody,
        "burst" => Track::Burst,
        other => return Err(format!("--track {other} is not prosody or burst")),
    };
    let parsed = parse_target(name, track)?;
    let intensity: f64 = m
        .get("intensity")
        .map(|s| s.parse())
        .transpose()
        .map_err(|e| format!("--intensity: {e}"))?
        .unwrap_or(1.0);
    Ok(TargetState {
        feeling: parsed.feeling,
        track: parsed.track,
        intensity: intensity.clamp(0.0, 1.0),
        inner: Default::default(),
        outer: Default::default(),
    })
}

fn run() -> Result<(), String> {
    let (cmd, m) = args();
    match cmd.as_str() {
        "decode" => {
            let voice = VoiceFile::load(PathBuf::from(need(&m, "voice")?).as_path())?;
            let wav = PathBuf::from(need(&m, "wav")?);
            let (samples, sr) = galion_voice_v2::audio::read_wav(&wav)?;
            let profile =
                galion_voice_v2::decode::decode_samples(galion_voice_v2::decode::DecodeIn {
                    id: &voice.id,
                    role: "reference",
                    samples: &samples,
                    sample_rate_hz: sr,
                    voice: &voice,
                    outer: voice.baseline.conditions,
                    inner: Default::default(),
                })?;
            let text = serde_json::to_string_pretty(&profile).map_err(|e| e.to_string())?;
            match m.get("out") {
                Some(p) => std::fs::write(p, text).map_err(|e| format!("{p}: {e}")),
                None => {
                    println!("{text}");
                    Ok(())
                }
            }
        }
        "speak" => {
            let voice = VoiceFile::load(PathBuf::from(need(&m, "voice")?).as_path())?;
            let target = target_from(&m)?;
            let out = speak(&SpeakIn {
                text: need(&m, "text")?,
                target: &target,
                voice: &voice,
                device: m.get("device").map(String::as_str),
            })?;
            if let Some(p) = m.get("out") {
                let samples: Vec<f64> = out.samples.iter().map(|s| *s as f64).collect();
                write_wav(PathBuf::from(p).as_path(), &samples, out.sample_rate_hz)?;
            }
            if let Some(p) = m.get("readback") {
                let text =
                    serde_json::to_string_pretty(&out.readback).map_err(|e| e.to_string())?;
                std::fs::write(p, text).map_err(|e| format!("{p}: {e}"))?;
            }
            println!(
                "renderer={} device={} intended_f0={:.1} measured_f0={} error_st={}",
                match out.renderer {
                    galion_voice_v2::render::RendererKind::Synthetic => "synthetic",
                    galion_voice_v2::render::RendererKind::Chatterbox => "chatterbox",
                },
                out.device.cbx_ep,
                out.diff_after.intended_f0_hz,
                out.diff_after
                    .measured_f0_hz
                    .map(|h| format!("{h:.1}"))
                    .unwrap_or_else(|| "none".into()),
                out.diff_after
                    .error_semitones
                    .map(|e| format!("{e:.2}"))
                    .unwrap_or_else(|| "none".into()),
            );
            Ok(())
        }
        "loop" => {
            let voice = VoiceFile::load(PathBuf::from(need(&m, "voice")?).as_path())?;
            let target = target_from(&m)?;
            let report = run_loop(&LoopIn {
                reference_wav: PathBuf::from(need(&m, "wav")?).as_path(),
                text: need(&m, "text")?,
                target: &target,
                voice: &voice,
                out_dir: PathBuf::from(need(&m, "out")?).as_path(),
                device: m.get("device").map(String::as_str),
            })?;
            println!(
                "cleaner={} renderer={} ticks={} corrected_error_st={}",
                report.cleaner,
                match report.speak.renderer {
                    galion_voice_v2::render::RendererKind::Synthetic => "synthetic",
                    galion_voice_v2::render::RendererKind::Chatterbox => "chatterbox",
                },
                report.reference.n_ticks,
                report
                    .speak
                    .diff_after
                    .error_semitones
                    .map(|e| format!("{e:.2}"))
                    .unwrap_or_else(|| "none".into()),
            );
            Ok(())
        }
        _ => Err(
            "usage: galion-voice-v2 decode|speak|loop (see tools/symphony/voice-v2/README.md)"
                .into(),
        ),
    }
}

fn main() {
    if let Err(e) = run() {
        eprintln!("galion-voice-v2: {e}");
        std::process::exit(2);
    }
}
