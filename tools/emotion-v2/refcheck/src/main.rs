// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! refcheck — Emotion V2 controlled-reference capture gate (432 Hz standard).
//!
//! Procedure: docs/emotion-v2/CAPTURE-PROTOCOL.md
//! Spec:      docs/emotion-v2/capture-spec.toml

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

use galion_refcheck::check::{self, Role};
use galion_refcheck::manifest;
use galion_refcheck::spec;

const HELP: &str = "\
refcheck check --wav FILE --spec FILE [--role emotion|calibration] [--out FILE]
refcheck manifest --dir DIR --out FILE [--spec FILE]
refcheck verify-manifest --manifest FILE --spec FILE [--require-session] [--allow-writable]

Emotion V2 capture gate, galion-voice-v2-432.
The WAV is opened read-only. Exit 0 is PASS, 1 is FAIL, 2 is an error.
";

fn main() -> ExitCode {
    match run(std::env::args().skip(1).collect()) {
        Ok(code) => code,
        Err(e) => {
            let body = serde_json::json!({"tool": "refcheck", "verdict": "ERROR", "error": e});
            println!("{body}");
            ExitCode::from(2)
        }
    }
}

fn run(args: Vec<String>) -> Result<ExitCode, String> {
    if args.is_empty() || args[0] == "--help" || args[0] == "-h" || args[0] == "help" {
        print!("{HELP}");
        return Ok(ExitCode::SUCCESS);
    }
    let cmd = args[0].as_str();
    let flags = parse_flags(&args[1..])?;
    match cmd {
        "check" => cmd_check(&flags),
        "manifest" => cmd_manifest(&flags),
        "verify-manifest" => cmd_verify(&flags),
        other => Err(format!("unknown command {other}\n{HELP}")),
    }
}

fn cmd_check(flags: &BTreeMap<String, String>) -> Result<ExitCode, String> {
    let wav = need(flags, "wav")?;
    let spec_path = need(flags, "spec")?;
    let role = Role::parse(flags.get("role").map(String::as_str).unwrap_or("emotion"))?;
    let report = check::check_file(Path::new(&wav), Path::new(&spec_path), role)?;
    let json = check::to_json(&report);
    println!("{json}");
    if let Some(out) = flags.get("out") {
        fs::write(out, json + "\n").map_err(|e| format!("out: {e}"))?;
    }
    Ok(if report.passed() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

fn cmd_manifest(flags: &BTreeMap<String, String>) -> Result<ExitCode, String> {
    let dir = need(flags, "dir")?;
    let out = need(flags, "out")?;
    let spec = match flags.get("spec") {
        Some(p) => Some(spec::load_spec(Path::new(p))?),
        None => None,
    };
    let manifest = manifest::build_manifest(Path::new(&dir), spec.as_ref())?;
    let json = manifest::to_json(&manifest)?;
    fs::write(&out, json.as_bytes()).map_err(|e| format!("out: {e}"))?;
    // The file write above does not add a trailing newline; stdout does, for a terminal.
    println!("{json}");
    Ok(ExitCode::SUCCESS)
}

fn cmd_verify(flags: &BTreeMap<String, String>) -> Result<ExitCode, String> {
    let manifest_path = need(flags, "manifest")?;
    let spec_path = need(flags, "spec")?;
    let spec = spec::load_spec(Path::new(&spec_path))?;
    let require = flags.contains_key("require-session");
    let allow = flags.contains_key("allow-writable");
    let report = manifest::verify_manifest(Path::new(&manifest_path), &spec, require, allow)?;
    println!("{}", manifest::to_json(&report)?);
    Ok(if report.verdict == "PASS" {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

fn need<'a>(flags: &'a BTreeMap<String, String>, key: &str) -> Result<&'a str, String> {
    flags
        .get(key)
        .map(String::as_str)
        .ok_or_else(|| format!("missing --{key}"))
}

fn parse_flags(args: &[String]) -> Result<BTreeMap<String, String>, String> {
    let mut map = BTreeMap::new();
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        let Some(name) = arg.strip_prefix("--") else {
            return Err(format!("unexpected argument {arg}"));
        };
        if name.is_empty() {
            return Err("unexpected argument --".into());
        }
        if name == "require-session" || name == "allow-writable" {
            map.insert(name.to_string(), "true".into());
            i += 1;
            continue;
        }
        let val = args
            .get(i + 1)
            .ok_or_else(|| format!("missing value for --{name}"))?;
        if val.starts_with("--") {
            return Err(format!("missing value for --{name}"));
        }
        map.insert(name.to_string(), val.clone());
        i += 2;
    }
    Ok(map)
}
