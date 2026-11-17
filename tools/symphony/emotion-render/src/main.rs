// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Read one target (or a list of 80 ms ticks) as JSON on stdin.
//! Write cloning controls as JSON on stdout. No audio.

use std::env;
use std::io::{self, Read};
use std::path::PathBuf;
use std::process::ExitCode;

use galion_emotion_render::{
    load_scales, render_tick, render_utterance, render_word, TargetTick, SCHEMA,
};
use serde_json::Value;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("galion-emotion-render: {err}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<(), String> {
    let mut scales_path: Option<PathBuf> = None;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--scales" => {
                scales_path = Some(PathBuf::from(args.next().ok_or("missing --scales path")?));
            }
            "--schema" => {
                println!("{SCHEMA}");
                return Ok(());
            }
            "-h" | "--help" => {
                eprintln!(
                    "usage: galion-emotion-render [--scales configs/symphony/emotion-render.toml] < ticks.json"
                );
                return Ok(());
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    let scales_path = scales_path
        .or_else(find_scales)
        .ok_or("pass --scales configs/symphony/emotion-render.toml")?;
    let scales = load_scales(&scales_path)?;
    let mut raw = String::new();
    io::stdin()
        .read_to_string(&mut raw)
        .map_err(|e| e.to_string())?;
    let value: Value = serde_json::from_str(&raw).map_err(|e| format!("json: {e}"))?;
    let out = match &value {
        Value::Array(rows) => {
            let ticks = rows
                .iter()
                .map(TargetTick::from_decoder_value)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;
            serde_json::to_value(render_utterance(&ticks, &scales).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?
        }
        Value::Object(_) if value.get("ticks").and_then(|t| t.as_array()).is_some() => {
            let rows = value["ticks"].as_array().unwrap();
            let ticks = rows
                .iter()
                .map(TargetTick::from_decoder_value)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;
            serde_json::to_value(render_utterance(&ticks, &scales).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?
        }
        Value::Object(_) => {
            let tick = TargetTick::from_decoder_value(&value).map_err(|e| e.to_string())?;
            let controls = render_tick(&tick, &scales).map_err(|e| e.to_string())?;
            if let Some(text) = value.get("text").and_then(|t| t.as_str()) {
                serde_json::to_value(render_word(text, &tick, &scales).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?
            } else {
                serde_json::to_value(controls).map_err(|e| e.to_string())?
            }
        }
        _ => return Err("stdin must be a tick object or a list of ticks".into()),
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&out).map_err(|e| e.to_string())?
    );
    Ok(())
}

fn find_scales() -> Option<PathBuf> {
    let mut dir = env::current_dir().ok()?;
    loop {
        let candidate = dir.join("configs/symphony/emotion-render.toml");
        if candidate.is_file() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}
