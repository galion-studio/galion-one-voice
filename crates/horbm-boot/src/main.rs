// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Print the voice boot decision for one `hardware.json`.
//!
//! The process does not call `nvidia-smi`, does not open a socket, and does
//! not read `GALION_LLM`. Exit 0 prints one line and has selected the CPU
//! brain. Exit 2 is a usage or parse error.

#![forbid(unsafe_code)]

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: horbm-boot <hardware.json>");
        std::process::exit(2);
    };
    if args.next().is_some() {
        eprintln!("usage: horbm-boot <hardware.json>");
        std::process::exit(2);
    }
    match horbm_boot::VoiceBoot::open(&path) {
        Ok(boot) => {
            println!("{boot}");
            if !boot.selects_cpu_brain() {
                std::process::exit(1);
            }
        }
        Err(err) => {
            eprintln!("horbm-boot: {err}");
            std::process::exit(2);
        }
    }
}
