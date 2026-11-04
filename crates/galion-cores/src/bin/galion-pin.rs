// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! `galion-pin <role> [--] <program> [args...]`
//!
//! Pins this process to the core-map placement for `role` (bus, asr/vosk,
//! llm, piper, retrieval), then execs `program`. Any argument equal to
//! `{threads}` becomes the role's thread count, so a local llama-server
//! starts with exactly its cores:
//!
//! ```text
//! GALION_LLM_LOCAL=1 galion-pin llm -- llama-server -m qwen.gguf -t {threads} --host 127.0.0.1 --port 18080
//! ```
//!
//! With the map off it execs unpinned, and `{threads}` becomes
//! `GALION_PIN_DEFAULT_THREADS` (default 4). One stderr line names the placement.

use galion_cores::{active_map, pin_current_thread, placement, CoreRole};
use std::os::unix::process::CommandExt;
use std::process::Command;

fn usage() -> ! {
    eprintln!("usage: galion-pin <bus|asr|llm|piper|retrieval> [--] <program> [args...]");
    std::process::exit(2);
}

fn main() {
    let mut args = std::env::args().skip(1);
    let role = args.next().and_then(|r| CoreRole::parse(&r)).unwrap_or_else(|| usage());
    let mut rest: Vec<String> = args.collect();
    if rest.first().is_some_and(|a| a == "--") {
        rest.remove(0);
    }
    if rest.is_empty() {
        usage();
    }
    let p = placement(role);
    let map = active_map(); // before the pin: afterwards this process sees fewer cores
    let threads = if p.threads > 0 {
        p.threads
    } else {
        std::env::var("GALION_PIN_DEFAULT_THREADS").ok().and_then(|t| t.parse().ok()).unwrap_or(4)
    };
    if let Err(e) = pin_current_thread(&p.cpus) {
        eprintln!("galion-pin: sched_setaffinity {}: {e}", p.cpu_list());
        std::process::exit(1);
    }
    eprintln!(
        "galion-pin: role={} map={} cpus={} threads={threads}",
        role.as_str(),
        map.as_str(),
        if p.pinned() { p.cpu_list() } else { "inherit".into() }
    );
    let program = rest.remove(0);
    let rest: Vec<String> =
        rest.into_iter().map(|a| if a == "{threads}" { threads.to_string() } else { a }).collect();
    let err = Command::new(&program).args(&rest).exec();
    eprintln!("galion-pin: exec {program}: {err}");
    std::process::exit(127);
}
