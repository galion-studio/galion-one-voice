// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! `galion-heartbeat-presence [SPEC=specs/presence.json] [--json]`
//! Runs presence.json through the real heartbeat with Rika ON (v0 thresholds)
//! and prints the report. Exit 0 green, 1 red, 2 invalid/IO.
use galion_heartbeat::presence::RikaReplayBackend;
use galion_presence::{run, spec, Verdict};
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut path = String::from("specs/presence.json");
    let mut json = false;
    for a in std::env::args().skip(1) {
        match a.as_str() {
            "--json" => json = true,
            p => path = p.to_string(),
        }
    }
    let src = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("galion-heartbeat-presence: cannot read {path}: {e}");
            return ExitCode::from(2);
        }
    };
    let file = match spec::parse_presence(&src) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("galion-heartbeat-presence: invalid {path}: {e}");
            return ExitCode::from(2);
        }
    };
    let mut backend = RikaReplayBackend::new();
    let report = run(&file, &mut backend);
    if json {
        println!("{}", report.to_json());
    } else {
        print!("galion-heartbeat-presence · backend=mock+heartbeat-rika · {} specs · {path}\n{}", file.specs.len(), report.summary());
    }
    match report.verdict {
        Verdict::Green => ExitCode::SUCCESS,
        Verdict::Red => ExitCode::from(1),
    }
}
