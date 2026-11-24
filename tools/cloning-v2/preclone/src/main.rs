// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! `galion-preclone --wav REF --out PROFILE.json [--target-f0 HZ] [--moments JSON]`
//!
//! Writes the pre-clone profile. Does not clone, does not download, does not
//! keep the reference.

#![forbid(unsafe_code)]

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use galion_preclone::{analyse, parse_moments, profile_bytes, read_wav, MomentFrame, Request};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("galion-preclone: {e}");
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
    let wav = flag(&args, "--wav")?;
    let out = flag(&args, "--out")?;
    let target = match optional(&args, "--target-f0")? {
        Some(s) => Some(
            s.parse::<f64>()
                .map_err(|_| "usage: --target-f0 wants a number".to_string())?,
        ),
        None => None,
    };
    let moments: Vec<MomentFrame> = match optional(&args, "--moments")? {
        Some(path) => {
            let text = fs::read_to_string(&path).map_err(|e| format!("moments: {e}"))?;
            parse_moments(&text)?
        }
        None => Vec::new(),
    };
    let (samples, sr) = read_wav(PathBuf::from(&wav).as_path())?;
    let profile = analyse(&Request {
        samples: &samples,
        sample_rate_hz: sr,
        target_f0_hz: target,
        moments: &moments,
    })?;
    let bytes = profile_bytes(&profile);
    fs::write(&out, bytes).map_err(|e| format!("out: {e}"))?;
    Ok(())
}

fn usage() -> String {
    "usage: galion-preclone --wav REF.wav --out PROFILE.json [--target-f0 HZ] [--moments FRAMES.json]".into()
}

fn flag(args: &[String], name: &str) -> Result<String, String> {
    optional(args, name)?.ok_or_else(|| format!("usage: missing {name}"))
}

fn optional(args: &[String], name: &str) -> Result<Option<String>, String> {
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
            && args[i] != "--wav"
            && args[i] != "--out"
            && args[i] != "--target-f0"
            && args[i] != "--moments"
        {
            return Err(format!("usage: unknown {}", args[i]));
        }
        i += 1;
    }
    Ok(found)
}
