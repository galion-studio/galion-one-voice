// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Phase-1 gate. Prints the domain 04 stamp line for a turn that did not run.
//!
//! No microphone, no model, no socket, no PCM, no words. Times are `absent`.
//! Lengths are `0`.

fn main() {
    let line = galion_cascade::dry_run_log();
    assert!(
        galion_cascade::stamp_log_clean(&line),
        "dry run log left the stamp vocabulary"
    );
    println!("{line}");
}
