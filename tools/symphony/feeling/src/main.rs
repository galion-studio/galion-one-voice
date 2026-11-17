// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Read intent frames as JSON on stdin. Write the feeling run as JSON on
//! stdout. No audio.

use std::env;
use std::io::{self, Read};
use std::path::PathBuf;
use std::process::ExitCode;

use galion_feeling::{Config, Feeling, Inventory, SCHEMA};
use galion_mne_contract::parse_frames;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("galion-feeling: {err}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<(), String> {
    let mut config_path: Option<PathBuf> = None;
    let mut inventory_path: Option<PathBuf> = None;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config" => {
                config_path = Some(PathBuf::from(args.next().ok_or("missing --config path")?));
            }
            "--inventory" => {
                inventory_path = Some(PathBuf::from(
                    args.next().ok_or("missing --inventory path")?,
                ));
            }
            "--schema" => {
                println!("{SCHEMA}");
                return Ok(());
            }
            "-h" | "--help" => {
                eprintln!(
                    "usage: galion-feeling [--config configs/symphony/feeling.toml] [--inventory configs/emotions-v2.toml] < intent.json"
                );
                return Ok(());
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    let config_path = config_path
        .or_else(|| find("configs/symphony/feeling.toml"))
        .ok_or("pass --config configs/symphony/feeling.toml")?;
    let inventory_path = inventory_path
        .or_else(|| find("configs/emotions-v2.toml"))
        .ok_or("pass --inventory configs/emotions-v2.toml")?;
    let cfg = Config::load(&config_path)?;
    let inv = Inventory::load(&inventory_path)?;
    let mut raw = String::new();
    io::stdin()
        .read_to_string(&mut raw)
        .map_err(|e| e.to_string())?;
    if raw.trim().is_empty() {
        return Err("stdin is empty".into());
    }
    let frames = parse_frames(&raw)?;
    let run = Feeling::run(&frames, &cfg, &inv).map_err(|e| e.to_string())?;
    println!(
        "{}",
        serde_json::to_string_pretty(&run).map_err(|e| e.to_string())?
    );
    Ok(())
}

fn find(rel: &str) -> Option<PathBuf> {
    let mut dir = env::current_dir().ok()?;
    loop {
        let candidate = dir.join(rel);
        if candidate.is_file() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}
