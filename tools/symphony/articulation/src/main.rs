// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! galion-articulation
//!   analyze --wav IN.wav [--sparc S.json] [--disvoice D.json] [--rhubarb R.json] --out CARD.json
//!   compare --ref A.json --clone B.json [--out SCORE.json]

use galion_articulation::{acoustic, score, workers, Articulation};
use std::collections::HashMap;

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

fn read_json(p: &str) -> Result<serde_json::Value, String> {
    let s = std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"))?;
    serde_json::from_str(&s).map_err(|e| format!("{p}: {e}"))
}

fn write(out: Option<&String>, s: String) -> Result<(), String> {
    match out {
        Some(p) => std::fs::write(p, s).map_err(|e| format!("{p}: {e}")),
        None => {
            println!("{s}");
            Ok(())
        }
    }
}

fn run() -> Result<(), String> {
    let (cmd, a) = args();
    match cmd.as_str() {
        "analyze" => {
            let wav = a.get("wav").ok_or("--wav required")?;
            let (x, sr) = acoustic::read_wav_mono(wav)?;
            let (mut art, settings) = acoustic::analyse(&x, sr);
            if let Some(p) = a.get("sparc") {
                workers::add_sparc(&mut art, &read_json(p)?)?;
            }
            if let Some(p) = a.get("disvoice") {
                workers::add_disvoice(&mut art, &read_json(p)?)?;
            }
            if let Some(p) = a.get("rhubarb") {
                workers::add_rhubarb(&mut art, &read_json(p)?)?;
            }
            art.finish();
            let card = serde_json::json!({ "articulation": art, "settings": settings });
            write(a.get("out"), serde_json::to_string_pretty(&card).unwrap())
        }
        "compare" => {
            let load = |k: &str| -> Result<Articulation, String> {
                let v = read_json(a.get(k).ok_or(format!("--{k} required"))?)?;
                serde_json::from_value(v["articulation"].clone()).map_err(|e| e.to_string())
            };
            let s = score::compare(&load("ref")?, &load("clone")?);
            write(a.get("out"), serde_json::to_string_pretty(&s).unwrap())
        }
        _ => Err("usage: galion-articulation analyze|compare (see src/main.rs)".into()),
    }
}

fn main() {
    if let Err(e) = run() {
        eprintln!("galion-articulation: {e}");
        std::process::exit(2);
    }
}
