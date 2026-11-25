// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Piper duplex probe. Compiled only when `horbm-duplex` has `piper_mouth.rs`.
//! A missing binary is `stub` and is not given a time.

fn main() {
    match horbm_duplex::discover_piper() {
        None => println!("piper_duplex=stub"),
        Some(paths) => {
            let started = std::time::Instant::now();
            match horbm_duplex::measure_piper(&paths) {
                Ok(stats) => {
                    std::hint::black_box(stats.text());
                    let ns = started.elapsed().as_nanos();
                    println!("piper_duplex=real");
                    println!("piper_duplex_ns={ns}");
                }
                Err(_) => println!("piper_duplex=stub"),
            }
        }
    }
}
