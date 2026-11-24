// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Read domain-04 turn stamps.
//!
//! `--dry-run` (also the default) publishes an empty sheet and prints it.
//! Every stage is `absent`. Every length is `0`. The lines have no PCM and
//! no words. Later phases call [`galion_turn_stamps::TurnStamps::turn_gap_ms`]
//! on a filled sheet.

fn main() {
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--dry-run" => {}
            "-h" | "--help" => {
                eprintln!("horbm-stamp-bench [--dry-run]");
                std::process::exit(0);
            }
            _ => {
                eprintln!("unknown argument");
                std::process::exit(2);
            }
        }
    }
    let report = galion_turn_stamps::dry_run_report();
    if !galion_turn_stamps::log_is_lengths_only(&report) {
        std::process::exit(1);
    }
    print!("{report}");
}
