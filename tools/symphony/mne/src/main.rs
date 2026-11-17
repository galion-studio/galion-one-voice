// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! galion-mne layers
//! galion-mne fixtures [--backend piper|chatterbox|moshi]
//!
//! Piper is the realtime CPU default. Chatterbox (offline) and Moshi (GPU)
//! run only when named. A missing Piper pack is the stand-in. No download.

use galion_mne::{layer_status, repo_root, run_fixtures_on};

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let cmd = args.next().unwrap_or_else(|| "fixtures".into());
    let mut backend = "piper".to_string();
    while let Some(arg) = args.next() {
        if arg == "--backend" {
            backend = args.next().unwrap_or_default();
        }
    }
    let repo = repo_root();
    match cmd.as_str() {
        "layers" => {
            println!(
                "{}",
                serde_json::to_string_pretty(&layer_status(&repo)).map_err(|e| e.to_string())?
            );
            Ok(())
        }
        "fixtures" | "run" => {
            let scores = run_fixtures_on(&repo, &backend)?;
            for row in &scores {
                println!(
                    "{}\tfeeling={:.3}\tpitch_err={}\tbreath={:.3}\tquality={:.3}\tbackend={}\tguard={}",
                    row.id,
                    row.feeling,
                    row.pitch_error_st.map(|e| format!("{e:.3}")).unwrap_or_else(|| "none".into()),
                    row.breath,
                    row.pitch_quality,
                    row.backend,
                    row.guard,
                );
            }
            Ok(())
        }
        _ => Err("usage: galion-mne layers | fixtures [--backend piper|chatterbox|moshi]".into()),
    }
}

fn main() {
    if let Err(e) = run() {
        eprintln!("galion-mne: {e}");
        std::process::exit(2);
    }
}
