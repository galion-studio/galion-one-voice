// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Print the turn-taking report. The clock is virtual.
//!
//! Exit 0 when every live gate holds. A stub stage is not a failure.
//! `--persona FILE` names the report. The duplex engine is unchanged.

fn main() {
    let mut args = std::env::args().skip(1);
    let mut persona_file = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--persona" => match args.next() {
                Some(path) => persona_file = Some(path),
                None => {
                    eprintln!("--persona needs a file");
                    std::process::exit(2);
                }
            },
            "-h" | "--help" => {
                eprintln!(
                    "horbm-turn-eval [--persona FILE]\n\
                     Virtual-clock turn-taking report. Exit 0 when every live gate holds."
                );
                std::process::exit(2);
            }
            other => {
                eprintln!("unknown argument {other}");
                std::process::exit(2);
            }
        }
    }
    let report = match persona_file.as_deref() {
        Some(path) => horbm_eval::run_stand_in_for(Some(std::path::Path::new(path))),
        None => horbm_eval::run_stand_in(),
    };
    println!("{}", report.text());
    if !report.passed() {
        std::process::exit(1);
    }
}
