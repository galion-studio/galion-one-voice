// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! `galion-presence [SPEC] [--out REPORT.json] [--json]`
//!
//! Exit codes: 0 green · 1 red (a `must` failed or budget exceeded) · 2 spec invalid / IO error.

use galion_presence::backend::{MockBackend, MockConfig};
use galion_presence::{run, spec, Verdict};
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut path = String::from("specs/presence.json");
    let mut out: Option<String> = None;
    let mut json = false;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--out" => out = args.next(),
            "--json" => json = true,
            "-h" | "--help" => {
                println!("usage: galion-presence [SPEC=specs/presence.json] [--out REPORT.json] [--json]\nbackend: mock (deterministic CPU). exit 0 green, 1 red, 2 invalid.");
                return ExitCode::SUCCESS;
            }
            p => path = p.to_string(),
        }
    }
    let src = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("galion-presence: cannot read {path}: {e}");
            return ExitCode::from(2);
        }
    };
    let base = std::path::Path::new(&path).parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let file = match spec::parse_presence_at(&src, &base) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("galion-presence: invalid {path}: {e}");
            return ExitCode::from(2);
        }
    };
    let mut backend = MockBackend::new(MockConfig::default());
    let report = run(&file, &mut backend);
    let body = report.to_json();
    if let Some(o) = out {
        if let Err(e) = std::fs::write(&o, format!("{body}\n")) {
            eprintln!("galion-presence: cannot write {o}: {e}");
            return ExitCode::from(2);
        }
    }
    if json {
        println!("{body}");
    } else {
        print!("galion-presence · backend=mock · {} specs · {path}\n{}", file.specs.len(), report.summary());
        println!("digest_fnv1a64={}", report.digest());
    }
    match report.verdict {
        Verdict::Green => ExitCode::SUCCESS,
        Verdict::Red => ExitCode::from(1),
    }
}
