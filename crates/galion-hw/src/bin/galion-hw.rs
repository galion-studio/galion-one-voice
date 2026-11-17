// G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network
// Developed and distributed by Galion Labs
// Perpetual Open MIT License
// Copyright © 2026 Galion Labs. All rights reserved.
//! `galion-hw [--cpu] [--facts] [--simulate <facts.json>] [--out <profile.json>]`
//!
//! Prints the device profile as JSON. `--facts` prints only what the bricks saw.
//! `--simulate` skips detection and builds the profile from a facts file
//! (see crates/galion-hw/sim/). `--cpu` = `GALION_ACCEL=cpu`.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |n: &str| args.iter().any(|a| a == n);
    let val = |n: &str| args.iter().position(|a| a == n).and_then(|i| args.get(i + 1)).cloned();
    let facts = match val("--simulate") {
        Some(p) => {
            let s = std::fs::read_to_string(&p).unwrap_or_else(|e| die(&format!("{p}: {e}")));
            serde_json::from_str(&s).unwrap_or_else(|e| die(&format!("{p}: {e}")))
        }
        None => galion_hw::detect(),
    };
    let policy = if flag("--cpu") { "cpu".to_string() } else { galion_hw::policy_from_env() };
    let json = if flag("--facts") {
        serde_json::to_string_pretty(&facts)
    } else {
        serde_json::to_string_pretty(&galion_hw::profile(&facts, &policy))
    }
    .expect("serialise");
    match val("--out") {
        Some(p) => std::fs::write(&p, json + "\n").unwrap_or_else(|e| die(&format!("{p}: {e}"))),
        None => println!("{json}"),
    }
}

fn die(m: &str) -> ! {
    eprintln!("galion-hw: {m}");
    std::process::exit(2)
}
