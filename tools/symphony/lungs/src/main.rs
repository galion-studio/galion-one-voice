// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! galion-lungs plan   --text "..." [--category sadness] [--arousal 0] [--energy 0.5] [--fatigue 0]
//!                     [--trajectory moments.json] [--config lungs.toml] [--emotions emotions.toml] [--out plan.json]
//! galion-lungs render --plan plan.json [--sr 24000] [--reference ref.wav] [--lungs cues.json] [--out breaths.wav]
//! galion-lungs mix    --plan plan.json --speech a.wav [--speech b.wav ...] [--sr 24000] [--reference ref.wav] [--lungs cues.json] [--out mixed.wav]
//!
//! A reference WAV is read at runtime and is not written back into the repository.
//! Breath audio is synthesized, or taken from that reference, and is not a committed asset.
#![forbid(unsafe_code)]

use galion_lungs::config::Config;
use galion_lungs::inventory::Inventory;
use galion_lungs::plan::{self, Moment, Request};
use galion_lungs::stage::{self, Lungs};
use galion_lungs::synth::{self, Reference};
use galion_lungs::wav;
use std::collections::HashMap;
use std::path::Path;

fn args() -> (String, HashMap<String, Vec<String>>) {
    let mut it = std::env::args().skip(1);
    let cmd = it.next().unwrap_or_default();
    let mut m: HashMap<String, Vec<String>> = HashMap::new();
    while let Some(k) = it.next() {
        if let Some(k) = k.strip_prefix("--") {
            let v = it.next().unwrap_or_default();
            m.entry(k.to_string()).or_default().push(v);
        }
    }
    (cmd, m)
}

fn one<'a>(m: &'a HashMap<String, Vec<String>>, k: &str) -> Option<&'a str> {
    m.get(k).and_then(|v| v.first()).map(|s| s.as_str())
}

fn read_text(path: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))
}

fn write_text(path: &str, body: &str) -> Result<(), String> {
    if let Some(parent) = Path::new(path).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
    }
    std::fs::write(path, body).map_err(|e| format!("{path}: {e}"))
}

fn load_config(m: &HashMap<String, Vec<String>>) -> Result<Config, String> {
    match one(m, "config") {
        Some(p) => Config::load(Path::new(p)),
        None => Config::bundled(),
    }
}

fn load_inventory(m: &HashMap<String, Vec<String>>) -> Result<Inventory, String> {
    match one(m, "emotions") {
        Some(p) => Inventory::load(Path::new(p)),
        None => Inventory::bundled(),
    }
}

fn load_lungs(path: &str) -> Result<Lungs, String> {
    let text = read_text(path)?;
    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))?;
    stage::parse_stage(&value)
}

fn run() -> Result<(), String> {
    let (cmd, m) = args();
    match cmd.as_str() {
        "plan" => {
            let cfg = load_config(&m)?;
            let inventory = load_inventory(&m)?;
            let text = if let Some(p) = one(&m, "text-file") {
                read_text(p)?
            } else {
                one(&m, "text").unwrap_or("").to_string()
            };
            let trajectory = if let Some(p) = one(&m, "trajectory") {
                let text = read_text(p)?;
                serde_json::from_str::<Vec<Moment>>(&text).map_err(|e| format!("{p}: {e}"))?
            } else {
                Vec::new()
            };
            let req = Request {
                text,
                category: one(&m, "category").unwrap_or("neutral").to_string(),
                arousal: one(&m, "arousal")
                    .map(parse_f64)
                    .transpose()?
                    .unwrap_or(0.0),
                energy: one(&m, "energy")
                    .map(parse_f64)
                    .transpose()?
                    .unwrap_or(cfg.energy.at),
                fatigue: one(&m, "fatigue")
                    .map(parse_f64)
                    .transpose()?
                    .unwrap_or(0.0),
                trajectory,
            };
            let planned = plan::plan(&req, &cfg, &inventory)?;
            let body = serde_json::to_string_pretty(&planned).map_err(|e| e.to_string())?;
            if let Some(p) = one(&m, "out") {
                write_text(p, &body)?;
            } else {
                println!("{body}");
            }
            Ok(())
        }
        "render" => {
            let cfg = load_config(&m)?;
            let plan_path = one(&m, "plan").ok_or("--plan required")?;
            let planned: plan::Plan = serde_json::from_str(&read_text(plan_path)?)
                .map_err(|e| format!("{plan_path}: {e}"))?;
            let sr = one(&m, "sr")
                .map(parse_f64)
                .transpose()?
                .unwrap_or(24_000.0);
            let lungs = one(&m, "lungs").map(load_lungs).transpose()?;
            let wav_data = one(&m, "reference")
                .map(|p| wav::read_wav(Path::new(p)))
                .transpose()?;
            let reference = match (&wav_data, &lungs) {
                (Some(w), Some(l)) => Some(Reference { samples: &w.samples, sample_rate: w.sample_rate, cues: &l.cues }),
                (Some(_), None) => return Err("--reference needs --lungs so the inhale cue is the features agent's, not a guess".into()),
                _ => None,
            };
            let rendered = synth::render_track(&planned, sr, reference.as_ref(), &cfg)?;
            if let Some(p) = one(&m, "out") {
                wav::write_wav_mono_i16(Path::new(p), sr.round() as u32, &rendered.samples)?;
            }
            let summary = serde_json::json!({
                "samples": rendered.samples.len(),
                "sample_rate": sr,
                "events": rendered.events,
            });
            println!("{}", serde_json::to_string_pretty(&summary).unwrap());
            Ok(())
        }
        "mix" => {
            let cfg = load_config(&m)?;
            let plan_path = one(&m, "plan").ok_or("--plan required")?;
            let planned: plan::Plan = serde_json::from_str(&read_text(plan_path)?)
                .map_err(|e| format!("{plan_path}: {e}"))?;
            let sr = one(&m, "sr")
                .map(parse_f64)
                .transpose()?
                .unwrap_or(24_000.0);
            let paths = m.get("speech").cloned().unwrap_or_default();
            if paths.is_empty() {
                return Err("--speech required, once per planned phrase, in order".into());
            }
            let mut phrases = Vec::new();
            for p in &paths {
                let w = wav::read_wav(Path::new(p))?;
                if (w.sample_rate - sr).abs() > 0.5 {
                    return Err(format!(
                        "{p} is {} Hz; pass --sr {}",
                        w.sample_rate, w.sample_rate
                    ));
                }
                phrases.push(w.samples);
            }
            let lungs = one(&m, "lungs").map(load_lungs).transpose()?;
            let wav_data = one(&m, "reference")
                .map(|p| wav::read_wav(Path::new(p)))
                .transpose()?;
            let reference = match (&wav_data, &lungs) {
                (Some(w), Some(l)) => Some(Reference {
                    samples: &w.samples,
                    sample_rate: w.sample_rate,
                    cues: &l.cues,
                }),
                (Some(_), None) => return Err("--reference needs --lungs".into()),
                _ => None,
            };
            let mixed = synth::mix_phrases(&phrases, &planned, sr, reference.as_ref(), &cfg)?;
            let out = one(&m, "out").ok_or("--out required")?;
            wav::write_wav_mono_i16(Path::new(out), sr.round() as u32, &mixed.samples)?;
            println!(
                "{{\"samples\":{},\"phrases\":{}}}",
                mixed.samples.len(),
                phrases.len()
            );
            Ok(())
        }
        _ => Err("usage: galion-lungs plan|render|mix (see src/main.rs)".into()),
    }
}

fn parse_f64(s: &str) -> Result<f64, String> {
    s.parse::<f64>().map_err(|_| format!("not a number: {s}"))
}

fn main() {
    if let Err(e) = run() {
        eprintln!("galion-lungs: {e}");
        std::process::exit(2);
    }
}
