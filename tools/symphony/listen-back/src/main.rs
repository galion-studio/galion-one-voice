// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! galion-listen-back run [--out report.json] [--summary report.txt] [--render-cmd BIN]
//!
//! Synthetic fixtures, one correction, two scores. The rollout gate is
//! printed as recorded and is not measured.

use galion_listen_back::{report, run_all, Options};
use std::collections::HashMap;

fn args() -> (String, HashMap<String, String>) {
    let mut it = std::env::args().skip(1);
    let cmd = it.next().unwrap_or_else(|| "run".into());
    let mut m = HashMap::new();
    while let Some(k) = it.next() {
        if let Some(k) = k.strip_prefix("--") {
            m.insert(k.to_string(), it.next().unwrap_or_default());
        }
    }
    (cmd, m)
}

fn run() -> Result<(), String> {
    let (cmd, a) = args();
    if cmd != "run" && cmd != "smoke" {
        return Err("usage: galion-listen-back run|smoke [--out report.json] [--summary report.txt] [--render-cmd BIN]".into());
    }
    let mut opt = Options::from_env();
    if let Some(cmd) = a.get("render-cmd") {
        if !cmd.is_empty() {
            opt.render_cmd = Some(cmd.clone());
        }
    }
    let report = run_all(&opt)?;
    let text = report::summary(&report);
    let json = report::to_json(&report);
    if let Some(path) = a.get("out") {
        std::fs::write(path, &json).map_err(|e| format!("{path}: {e}"))?;
    }
    if let Some(path) = a.get("summary") {
        std::fs::write(path, &text).map_err(|e| format!("{path}: {e}"))?;
    }
    print!("{text}");
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("galion-listen-back: {e}");
        std::process::exit(2);
    }
}
