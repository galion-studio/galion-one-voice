// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! galion-mne layers
//! galion-mne run --text "..." --pin embarrassment --voice VOICE.toml --out DIR

use galion_mne::{layer_status, repo_root, run, write_wav, RunIn};
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

fn run_cli() -> Result<(), String> {
    let (cmd, m) = args();
    let repo = repo_root();
    match cmd.as_str() {
        "layers" => {
            let text = serde_json::to_string_pretty(&layer_status(&repo)).map_err(|e| e.to_string())?;
            println!("{text}");
            Ok(())
        }
        "run" => {
            let voice = PathBuf::from(need(&m, "voice")?);
            let report = run(&RunIn {
                repo: &repo,
                text: need(&m, "text")?,
                pin: m.get("pin").map(String::as_str).unwrap_or("neutral"),
                voice: &voice,
            })?;
            if let Some(dir) = m.get("out") {
                let dir = PathBuf::from(dir);
                std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                write_wav(&dir.join("mixed.wav"), &report.audio, report.sample_rate_hz)?;
                let json = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
                std::fs::write(dir.join("report.json"), json).map_err(|e| e.to_string())?;
            }
            println!(
                "ticks={} listen_passes={} pin={}",
                report.lungs.n_ticks,
                report.listen.iterations,
                report.clone_request_patch.get("emotion").and_then(|v| v.as_str()).unwrap_or("?")
            );
            Ok(())
        }
        _ => Err("usage: galion-mne layers | run --text TEXT --voice VOICE.toml [--pin NAME] [--out DIR]".into()),
    }
}

fn main() {
    if let Err(e) = run_cli() {
        eprintln!("galion-mne: {e}");
        std::process::exit(2);
    }
}
