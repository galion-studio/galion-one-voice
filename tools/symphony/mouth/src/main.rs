// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! galion-mouth plan --intent I.json --feeling F.json [--scales mouth.toml] [--out plan.json]
//! galion-mouth read --card C.json [--out read.json]
//! galion-mouth read --wav W.wav [--bin galion-articulation] [--out read.json]
//! galion-mouth listen --plan P.json --card C.json [--baseline B.json] [--out listen.json]
//!
//! `--wav` runs an already installed galion-articulation. It does not download.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use galion_mouth::{listen, load_scales, plan_value, read_json, read_via_binary, Scales};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("galion-mouth: {err}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let cmd = args.next().unwrap_or_default();
    let mut map = std::collections::HashMap::<String, String>::new();
    let rest: Vec<String> = args.collect();
    let mut i = 0;
    while i < rest.len() {
        if let Some(key) = rest[i].strip_prefix("--") {
            let val = rest.get(i + 1).cloned().unwrap_or_default();
            map.insert(key.to_string(), val);
            i += 2;
        } else {
            i += 1;
        }
    }
    match cmd.as_str() {
        "plan" => {
            let intent = read_json_file(req(&map, "intent")?)?;
            let feeling = read_json_file(req(&map, "feeling")?)?;
            let scales = scales_from(map.get("scales"))?;
            let planned = plan_value(&intent, &feeling, &scales).map_err(|e| e.to_string())?;
            write_out(
                map.get("out"),
                &serde_json::to_string_pretty(&planned).map_err(|e| e.to_string())?,
            )
        }
        "read" => {
            let read = if let Some(card) = map.get("card") {
                let text = fs::read_to_string(card).map_err(|e| format!("{card}: {e}"))?;
                read_json(&text).map_err(|e| e.to_string())?
            } else if let Some(wav) = map.get("wav") {
                let bin = map
                    .get("bin")
                    .cloned()
                    .or_else(|| env::var("GALION_ARTICULATION").ok())
                    .ok_or_else(|| {
                        "set --bin or GALION_ARTICULATION to an installed galion-articulation. Nothing was downloaded.".to_string()
                    })?;
                read_via_binary(PathBuf::from(bin).as_path(), PathBuf::from(wav).as_path())
                    .map_err(|e| e.to_string())?
            } else {
                return Err("read needs --card or --wav".into());
            };
            write_out(
                map.get("out"),
                &serde_json::to_string_pretty(&read).map_err(|e| e.to_string())?,
            )
        }
        "listen" => {
            let plan = read_json_file(req(&map, "plan")?)?;
            let plan: galion_mouth::MouthPlan =
                serde_json::from_value(plan).map_err(|e| e.to_string())?;
            let card = fs::read_to_string(req(&map, "card")?).map_err(|e| e.to_string())?;
            let read = read_json(&card).map_err(|e| e.to_string())?;
            let baseline = match map.get("baseline") {
                Some(path) => {
                    let text = fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
                    Some(read_json(&text).map_err(|e| e.to_string())?)
                }
                None => None,
            };
            let report = listen(&plan, &read, baseline.as_ref());
            write_out(
                map.get("out"),
                &serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?,
            )
        }
        _ => {
            Err("usage: galion-mouth plan|read|listen (see tools/symphony/mouth/README.md)".into())
        }
    }
}

fn req<'a>(
    map: &'a std::collections::HashMap<String, String>,
    key: &str,
) -> Result<&'a str, String> {
    map.get(key)
        .map(String::as_str)
        .ok_or_else(|| format!("--{key} is required"))
}

fn read_json_file(path: &str) -> Result<serde_json::Value, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))
}

fn scales_from(path: Option<&String>) -> Result<Scales, String> {
    match path {
        Some(path) => load_scales(PathBuf::from(path).as_path()).map_err(|e| e.to_string()),
        None => Ok(Scales::unknown(80)),
    }
}

fn write_out(path: Option<&String>, body: &str) -> Result<(), String> {
    match path {
        Some(path) => fs::write(path, body).map_err(|e| format!("{path}: {e}")),
        None => {
            println!("{body}");
            Ok(())
        }
    }
}
