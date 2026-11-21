// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! CLI for the S12 latency harness.
//!
//! ```text
//! s12-latency-harness run [--backend fake|piper|all] [--reps N] [--warmup N] [--out-dir DIR]
//! ```
//!
//! Exit 0 when every requested backend measured. Exit 2 when a requested
//! backend is absent or fails. That path writes `measured: false` and no
//! percentile fields. Exit 1 is usage.
//!
//! Publish numbers from `cargo run --release`. The results file records
//! `profile`.

use s12_latency_harness::{probe_machine, repo_root, run_fake, run_piper, HarnessError, Report};
use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() || args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return if args.is_empty() {
            ExitCode::from(1)
        } else {
            ExitCode::SUCCESS
        };
    }
    if args[0] != "run" {
        eprintln!("unknown command '{}'. Try --help.", args[0]);
        return ExitCode::from(1);
    }
    match parse_run(&args[1..]) {
        Ok(opts) => run(opts),
        Err(msg) => {
            eprintln!("{msg}");
            ExitCode::from(1)
        }
    }
}

struct Opts {
    backend: String,
    reps: usize,
    warmup: usize,
    out_dir: PathBuf,
}

fn parse_run(args: &[String]) -> Result<Opts, String> {
    let mut backend = "all".to_string();
    let mut reps = 100usize;
    let mut warmup = 1usize;
    let mut out_dir = PathBuf::from("results");
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        let val = || -> Result<String, String> {
            args.get(i + 1)
                .cloned()
                .ok_or_else(|| format!("{a} needs a value"))
        };
        match a.as_str() {
            "--backend" => {
                backend = val()?;
                i += 2;
            }
            "--reps" => {
                reps = val()?
                    .parse()
                    .map_err(|_| "--reps is not a number".to_string())?;
                i += 2;
            }
            "--warmup" => {
                warmup = val()?
                    .parse()
                    .map_err(|_| "--warmup is not a number".to_string())?;
                i += 2;
            }
            "--out-dir" => {
                out_dir = PathBuf::from(val()?);
                i += 2;
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    if !matches!(backend.as_str(), "fake" | "piper" | "all") {
        return Err("--backend must be fake, piper, or all".into());
    }
    if !(1..=10_000).contains(&reps) {
        return Err("--reps must be from 1 to 10000".into());
    }
    if warmup > 20 {
        return Err("--warmup must be from 0 to 20".into());
    }
    Ok(Opts {
        backend,
        reps,
        warmup,
        out_dir,
    })
}

fn run(opts: Opts) -> ExitCode {
    let mut failed = false;
    let want_fake = opts.backend == "fake" || opts.backend == "all";
    let want_piper = opts.backend == "piper" || opts.backend == "all";
    if want_fake {
        match run_fake(opts.reps, opts.warmup) {
            Ok(report) => {
                if write(&report, &opts.out_dir, "fake").is_err() {
                    failed = true;
                }
            }
            Err(e) => {
                eprintln!("fake backend failed: {e}");
                failed = true;
                let _ = write_unmeasured("fake", &e.to_string(), &opts.out_dir);
            }
        }
    }
    if want_piper {
        match run_piper(opts.reps, opts.warmup) {
            Ok(report) => {
                if write(&report, &opts.out_dir, "piper").is_err() {
                    failed = true;
                }
            }
            Err(HarnessError::Absent { reason }) => {
                eprintln!("piper not measured: {reason}");
                failed = true;
                let _ = write_unmeasured("piper", &reason, &opts.out_dir);
            }
            Err(HarnessError::Failed(e)) => {
                eprintln!("piper run failed: {e}");
                failed = true;
                let _ = write_unmeasured("piper", &e.to_string(), &opts.out_dir);
            }
        }
    }
    if failed {
        ExitCode::from(2)
    } else {
        ExitCode::SUCCESS
    }
}

fn write(report: &Report, dir: &PathBuf, stem: &str) -> Result<(), ()> {
    match report.write_pair(dir, stem) {
        Ok((json, md)) => {
            eprintln!("wrote {} and {}", json.display(), md.display());
            Ok(())
        }
        Err(e) => {
            eprintln!("{e}");
            Err(())
        }
    }
}

fn write_unmeasured(backend: &str, reason: &str, dir: &PathBuf) -> Result<(), ()> {
    let machine = probe_machine(&repo_root());
    let report = Report::unmeasured(backend, reason, machine);
    write(&report, dir, backend)
}

fn print_help() {
    eprintln!(
        "\
s12-latency-harness — per-stage clocks for the local voice path

USAGE
  s12-latency-harness run [--backend fake|piper|all] [--reps N] [--warmup N] [--out-dir DIR]

The default is --backend all --reps 100 --warmup 1 --out-dir results.
Measure with cargo run --release. The file records profile, CPU, date, and commit.

BACKENDS
  fake    In-process VAD, ASR, reply, and one sine frame. Not a product clock.
  piper   Same fake VAD, ASR, and reply. TTS is the local Piper child.
          The voice is en_US-ljspeech-medium. It is not Samantha.

PIPER
  Linux x86_64: from the repo root, scripts/a21/fetch-a21.sh
  Or set GALION_A21_RUNTIME to the piper binary and GALION_TTS_MODEL to the .onnx.
  The .onnx.json must sit beside the model. This program does not download.

EXIT
  0  every requested backend measured
  1  usage
  2  a requested backend was absent or failed (JSON says measured false, no percentiles)"
    );
}
