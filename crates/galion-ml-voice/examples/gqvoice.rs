// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! Compile one clone utterance into `voices/gq/<persona>.gqvoice`.
//!
//! Worker stdout is raw s16le. This program writes a Galion packet and
//! refuses to treat that stdout as the file the stack plays.
//!
//! ```text
//! GALION_CLONE_STUB_PCM=1 cargo run --offline -p galion-ml-voice --features speech --example gqvoice -- --persona samantha --text "Hello."
//! GALION_CLONE_STUB_PCM=1 cargo run --offline -p galion-ml-voice --features speech --example gqvoice -- --persona jarvis --text "Systems are ready."
//! ```
//!
//! `--pcm FILE` reads raw s16le already captured from the worker and does
//! not spawn it. Protocol-stub PCM is not a film voice and not a Flash result.

use galion_ml_voice::speech::clone_worker::{worker_script, CloneModel, CloneWorker};
use galion_ml_voice::speech::gqvoice::{self, GqVoice};
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut persona = String::from("samantha");
    let mut model = String::from("nano");
    let mut text = String::from("Hello.");
    let mut pcm: Option<String> = None;
    let mut out: Option<String> = None;
    let mut text_set = false;
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        let key = args[i].as_str();
        if key == "-h" || key == "--help" {
            eprintln!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        let Some(val) = args.get(i + 1) else {
            eprintln!("gqvoice: missing value for {key}\n{USAGE}");
            return ExitCode::from(64);
        };
        match key {
            "--persona" => persona = val.clone(),
            "--model" => model = val.clone(),
            "--text" => {
                text = val.clone();
                text_set = true;
            }
            "--pcm" => pcm = Some(val.clone()),
            "--out" => out = Some(val.clone()),
            other => {
                eprintln!("gqvoice: unknown argument {other}\n{USAGE}");
                return ExitCode::from(64);
            }
        }
        i += 2;
    }
    if pcm.is_some() && text_set {
        eprintln!("gqvoice: pass --text or --pcm, not both\n{USAGE}");
        return ExitCode::from(64);
    }
    if CloneModel::parse(&model).is_none() {
        eprintln!("gqvoice: --model is nano or turbo");
        return ExitCode::from(64);
    }
    let dir = out.map(PathBuf::from).unwrap_or_else(gqvoice::gqvoice_dir);
    let samples = match pcm {
        Some(path) => match read_s16le(&path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("gqvoice: {e}");
                return ExitCode::from(2);
            }
        },
        None => match speak(&persona, &model, &text) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("gqvoice: {e}");
                return ExitCode::from(2);
            }
        },
    };
    let raw = encode(&samples);
    if GqVoice::open_bytes(&raw).is_ok() {
        eprintln!("gqvoice: loader accepted raw chatterbox pcm");
        return ExitCode::from(2);
    }
    match gqvoice::handoff_at(&samples, &persona, &model, &dir) {
        Ok(audio) => {
            let path = gqvoice::asset_path(&dir, &persona);
            let packet = match GqVoice::open(&path) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("gqvoice: {e}");
                    return ExitCode::from(2);
                }
            };
            println!("persona={}", packet.persona_name());
            println!("model={}", packet.model_name());
            println!("samples={}", packet.sample_count());
            println!("sectors={}", packet.sector_count());
            println!("sector_ms={}", gqvoice::SECTOR_MS);
            println!("bytes={}", packet.as_bytes().len());
            println!("played={}", audio.samples.len());
            println!("raw_rejected=yes");
            println!("landed {}", path.display());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("gqvoice: {e}");
            ExitCode::from(2)
        }
    }
}

fn speak(persona: &str, model: &str, text: &str) -> Result<Vec<i16>, String> {
    let script = worker_script().ok_or("clone worker script missing")?;
    let model = CloneModel::parse(model).ok_or("model must be nano or turbo")?;
    let mut worker = CloneWorker::spawn(&script, model, persona).map_err(|e| e.to_string())?;
    worker.synthesize(text).map_err(|e| e.to_string())
}

fn read_s16le(path: &str) -> Result<Vec<i16>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    if bytes.len() < 2 || bytes.len() % 2 != 0 {
        return Err(format!("{path}: expected raw s16le from the worker"));
    }
    if bytes.starts_with(b"RIFF") || bytes.starts_with(b"GQVOICE") {
        return Err(format!(
            "{path}: --pcm is worker stdout, not a wav and not a gqvoice"
        ));
    }
    Ok(bytes
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]))
        .collect())
}

fn encode(samples: &[i16]) -> Vec<u8> {
    let mut raw = Vec::with_capacity(samples.len() * 2);
    for s in samples {
        raw.extend_from_slice(&s.to_le_bytes());
    }
    raw
}

const USAGE: &str = "\
usage: gqvoice [--persona samantha|jarvis] [--model nano|turbo]
               [--text TEXT | --pcm FILE.s16le] [--out DIR]

Compiles Chatterbox worker PCM into DIR/<persona>.gqvoice.
DIR defaults to voices/gq, or GALION_GQVOICE.
Set GALION_CLONE_STUB_PCM=1 for protocol PCM. That is not a film voice.
";
