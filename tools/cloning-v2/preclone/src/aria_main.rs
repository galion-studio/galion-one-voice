// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! `galion-aria-blend --samantha PROFILE.json --jarvis PROFILE.json --out SPEC.json`
//!
//! Designs the synthetic voice Aria from two `galion-preclone/1` profiles.
//! Does not read a microphone, does not render, and does not keep a copy of
//! a recording. The real pair of profiles is blended on the owner's laptop.

#![forbid(unsafe_code)]

use std::env;
use std::fs;
use std::process::ExitCode;

use galion_preclone::blend::{design_from_profiles, spec_bytes};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("galion-aria-blend: {e}");
            if e.starts_with("usage:") {
                ExitCode::from(2)
            } else {
                ExitCode::from(1)
            }
        }
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() || args.iter().any(|a| a == "--help" || a == "-h") {
        return Err(usage());
    }
    let warmth = flag(&args, "--samantha")?;
    let calm = flag(&args, "--jarvis")?;
    let out = flag(&args, "--out")?;
    let warmth_text = fs::read_to_string(&warmth).map_err(|e| format!("samantha: {e}"))?;
    let calm_text = fs::read_to_string(&calm).map_err(|e| format!("jarvis: {e}"))?;
    let spec = design_from_profiles(&warmth_text, &calm_text)?;
    fs::write(&out, spec_bytes(&spec)).map_err(|e| format!("out: {e}"))?;
    Ok(())
}

fn usage() -> String {
    "usage: galion-aria-blend --samantha PROFILE.json --jarvis PROFILE.json --out SPEC.json".into()
}

fn flag(args: &[String], name: &str) -> Result<String, String> {
    let mut found = None;
    let mut i = 0;
    while i < args.len() {
        if args[i] == name {
            let v = args
                .get(i + 1)
                .ok_or_else(|| format!("usage: {name} needs a value"))?;
            if v.starts_with("--") {
                return Err(format!("usage: {name} needs a value"));
            }
            if found.is_some() {
                return Err(format!("usage: {name} was given twice"));
            }
            found = Some(v.clone());
            i += 2;
            continue;
        }
        if args[i].starts_with("--")
            && args[i] != "--samantha"
            && args[i] != "--jarvis"
            && args[i] != "--out"
        {
            return Err(format!("usage: unknown {}", args[i]));
        }
        i += 1;
    }
    found.ok_or_else(|| format!("usage: missing {name}"))
}
