// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! `galion-throat render` writes the cloner control packet.
//! `--source` also writes the LF derivative as a wav. That wav is synthetic.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use galion_throat::config;
use galion_throat::contract::{self, FeelingDrive};
use galion_throat::lungs_in;
use galion_throat::render::{self, Request};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    let cmd = args.first().ok_or("usage: galion-throat render --feeling F --lungs L --baseline-hz HZ --config C --out JSON [--source WAV] [--rd RD]")?;
    if cmd != "render" {
        return Err("only the render command is implemented".into());
    }
    let mut feeling: Option<PathBuf> = None;
    let mut lungs: Option<PathBuf> = None;
    let mut baseline: Option<f64> = None;
    let mut rd: Option<f64> = None;
    let mut config_path: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut source: Option<PathBuf> = None;
    let mut i = 1;
    while i < args.len() {
        let flag = args[i].as_str();
        let value = |flag: &str, i: &mut usize| -> Result<String, String> {
            *i += 1;
            args.get(*i)
                .cloned()
                .ok_or_else(|| format!("missing value for {flag}"))
        };
        match flag {
            "--feeling" => feeling = Some(PathBuf::from(value(flag, &mut i)?)),
            "--lungs" => lungs = Some(PathBuf::from(value(flag, &mut i)?)),
            "--baseline-hz" => {
                let s = value(flag, &mut i)?;
                baseline = Some(s.parse::<f64>().map_err(|_| format!("baseline F0 {s}"))?);
            }
            "--rd" => {
                let s = value(flag, &mut i)?;
                rd = Some(s.parse::<f64>().map_err(|_| format!("Rd {s}"))?);
            }
            "--config" => config_path = Some(PathBuf::from(value(flag, &mut i)?)),
            "--out" => out = Some(PathBuf::from(value(flag, &mut i)?)),
            "--source" => source = Some(PathBuf::from(value(flag, &mut i)?)),
            other => return Err(format!("unknown argument {other}")),
        }
        i += 1;
    }
    let feeling_path = feeling.ok_or("missing --feeling")?;
    let lungs_path = lungs.ok_or("missing --lungs")?;
    let baseline = baseline.ok_or("missing --baseline-hz")?;
    let config_path = config_path.unwrap_or_else(|| PathBuf::from("configs/symphony/throat.toml"));
    let out = out.ok_or("missing --out")?;
    let cfg = config::load_path(&config_path)?;
    let feeling_v: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&feeling_path).map_err(|e| format!("feeling file: {e}"))?,
    )
    .map_err(|e| format!("feeling json: {e}"))?;
    let lungs_v: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&lungs_path).map_err(|e| format!("lungs file: {e}"))?,
    )
    .map_err(|e| format!("lungs json: {e}"))?;
    let drive: FeelingDrive = contract::parse_feeling_drive(&feeling_v)?;
    let req = Request {
        speaker: contract::parse_speaker(baseline, rd)?,
        feeling: drive,
        lungs: lungs_in::parse_lungs(&lungs_v)?,
    };
    let (utterance, samples) = render::render(&req, &cfg, source.is_some())?;
    let json = serde_json::to_string_pretty(&utterance).map_err(|e| format!("json: {e}"))?;
    fs::write(&out, json).map_err(|e| format!("write {}: {e}", out.display()))?;
    if let (Some(path), Some(samples)) = (source, samples) {
        galion_throat::wav::write_wav(&path, &samples, cfg.source_hz)?;
    }
    Ok(())
}
