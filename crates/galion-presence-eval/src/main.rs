// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! `galion-presence-eval [--headless] [--out REPORT.json] [--wav-dir DIR] [--commit HEX]`
//!
//! Exit 0 green, 1 a scored case failed, 2 usage or IO.
//! Offline. The default binds 127.0.0.1 only. `--headless` does not bind, does
//! not open a phone lane, and does not load speech weights. See docs/eval/PRESENCE-EVAL.md.

use std::path::PathBuf;
use std::process::ExitCode;

use galion_presence_eval::{run, run_headless, RunArgs};

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut out: Option<String> = None;
    let mut wav_dir: Option<String> = None;
    let mut commit: Option<String> = None;
    let mut headless = false;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--out" => out = args.next(),
            "--wav-dir" => wav_dir = args.next(),
            "--commit" => commit = args.next(),
            "--headless" => headless = true,
            "-h" | "--help" => {
                println!(
                    "usage: galion-presence-eval [--headless] [--out REPORT.json] [--wav-dir DIR] [--commit HEX]\n\
                     default report: galion-gates/results/presence-eval/report.json\n\
                     exit 0 green, 1 red, 2 usage or IO.\n\
                     default: loopback on 127.0.0.1. --headless: no socket, no phone lane, no weights.\n\
                     interim is a placeholder, not recognition. ttfa stays unmeasured without a reply wav."
                );
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("galion-presence-eval: unknown argument {other}");
                return ExitCode::from(2);
            }
        }
    }
    let report = PathBuf::from(
        out.unwrap_or_else(|| "galion-gates/results/presence-eval/report.json".into()),
    );
    let wav_owned = match wav_dir {
        Some(dir) => PathBuf::from(dir),
        None => report
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .to_path_buf(),
    };
    let commit = commit.unwrap_or_else(|| {
        std::env::var("GALION_EVAL_COMMIT").unwrap_or_else(|_| "unknown".into())
    });
    let args = RunArgs {
        report: &report,
        wav_dir: &wav_owned,
        commit: &commit,
    };
    let ran = if headless { run_headless(args) } else { run(args) };
    match ran {
        Ok(summary) => {
            println!(
                "galion-presence-eval · verdict={} · cases={} · failed={} · ttfa={} · barge-in={} · report={}",
                summary.verdict,
                summary.cases,
                summary.failed,
                summary.ttfa_ms.map(|n| n.to_string()).unwrap_or_else(|| summary.ttfa_status.clone()),
                summary.barge_in_cancel,
                summary.report.display()
            );
            if summary.verdict == "green" {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        Err(e) => {
            eprintln!("galion-presence-eval: {e}");
            ExitCode::from(2)
        }
    }
}
