// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! galion-mne layers
//! galion-mne fixtures

use galion_mne::{layer_status, repo_root, run_fixtures};

fn run() -> Result<(), String> {
    let cmd = std::env::args().nth(1).unwrap_or_else(|| "fixtures".into());
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
            let scores = run_fixtures(&repo)?;
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
        _ => Err("usage: galion-mne layers | fixtures".into()),
    }
}

fn main() {
    if let Err(e) = run() {
        eprintln!("galion-mne: {e}");
        std::process::exit(2);
    }
}
