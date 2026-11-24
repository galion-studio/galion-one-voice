// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! `galion-pin <role> [--] <program> [args...]`
//!
//! Pins this process to the core-map placement for `role` (bus, asr/vosk,
//! reasoning, piper_samantha, piper_jarvis, retrieval), then execs `program`.
//! Any argument equal to `{threads}` becomes the role's thread count, so an
//! in-house reasoning process starts with exactly its cores:
//!
//! ```text
//! GALION_REASONING_LOCAL=1 galion-pin reasoning -- <in-house S0 runner> --threads {threads}
//! ```
//!
//! With the map off it execs unpinned, and `{threads}` becomes
//! `GALION_PIN_DEFAULT_THREADS` (default 4). One stderr line names the placement.

use galion_cores::{active_map, pin_current_thread, placement, CoreRole};
use std::process::Command;

fn usage() -> ! {
    eprintln!("usage: galion-pin <bus|asr|reasoning|piper_samantha|piper_jarvis|retrieval> [--] <program> [args...]");
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
        eprintln!("galion-pin: pin {}: {e}", p.cpu_list());
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
    exec_program(&program, &rest);
}

/// Unix replaces this process. Windows has no `exec`, so the launcher waits
/// and forwards the child status.
#[cfg(unix)]
fn exec_program(program: &str, args: &[String]) -> ! {
    use std::os::unix::process::CommandExt;
    let err = Command::new(program).args(args).exec();
    eprintln!("galion-pin: exec {program}: {err}");
    std::process::exit(127);
}

#[cfg(not(unix))]
fn exec_program(program: &str, args: &[String]) -> ! {
    match Command::new(program).args(args).status() {
        Ok(status) => std::process::exit(status.code().unwrap_or(1)),
        Err(err) => {
            eprintln!("galion-pin: exec {program}: {err}");
            std::process::exit(127);
        }
    }
}
