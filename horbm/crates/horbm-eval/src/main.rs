// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Run the closed HORBM gate and print the JSON report.

use std::path::Path;
use std::process::ExitCode;

use horbm_eval::{run_with, Config, Thresholds};

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut out_path = None;
    let mut thresholds_path = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--out" => match args.next() {
                Some(path) => out_path = Some(path),
                None => return usage(),
            },
            "--thresholds" => match args.next() {
                Some(path) => thresholds_path = Some(path),
                None => return usage(),
            },
            "-h" | "--help" => return usage(),
            other => {
                eprintln!("unknown argument {other}");
                return usage();
            }
        }
    }
    let thresholds = match thresholds_path {
        Some(path) => match Thresholds::load_path(Path::new(&path)) {
            Ok(parsed) => parsed,
            Err(err) => {
                eprintln!("{err}");
                return ExitCode::from(2);
            }
        },
        None => Thresholds::embedded(),
    };
    let report = run_with(Config {
        research_dir: horbm_eval::research_dir_from_env(),
        thresholds,
    });
    let json = report.to_json();
    println!("{json}");
    if let Some(path) = out_path {
        if let Err(err) = std::fs::write(&path, format!("{json}\n")) {
            eprintln!("write {path}: {err}");
            return ExitCode::from(2);
        }
    }
    if report.pass {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn usage() -> ExitCode {
    eprintln!("horbm-eval [--thresholds FILE] [--out FILE]");
    ExitCode::from(2)
}
