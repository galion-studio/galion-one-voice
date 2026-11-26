// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! `eval-golden` — golden scripts for the configured persona, persona lint, and the Piper
//! round-trip. A missing scorer is UNMEASURED. A measured miss is FAIL.
//! Neither one is turned into a pass.

use std::env;
use std::fs;
use std::process::ExitCode;

use galion_eval_golden::brain::{open_horbm, FakeReplyEngine};
use galion_eval_golden::golden::run_golden;
use galion_eval_golden::lint;
use galion_eval_golden::report::{self, build};
use galion_eval_golden::roundtrip;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() || args.iter().any(|a| a == "-h" || a == "--help") && args.len() == 1 {
        print_help();
        return ExitCode::SUCCESS;
    }
    let cmd = args.first().map(String::as_str).unwrap_or("report");
    let strict = args.iter().any(|a| a == "--strict");
    let out = flag(&args, "--out");
    match cmd {
        "help" | "--help" | "-h" => {
            print_help();
            ExitCode::SUCCESS
        }
        "lint" => {
            let text = flag(&args, "--text").unwrap_or_default();
            let user = flag(&args, "--user");
            let hits = lint::lint_reply(&text, user.as_deref());
            if hits.is_empty() {
                println!("PASS");
                ExitCode::SUCCESS
            } else {
                println!("FAIL");
                for hit in hits {
                    println!("{}: {}", hit.rule, hit.detail);
                }
                if strict {
                    ExitCode::from(1)
                } else {
                    ExitCode::SUCCESS
                }
            }
        }
        "golden" => {
            let mut engine = FakeReplyEngine::new();
            let _ = open_horbm("", "", "");
            let reports = run_golden(&mut engine);
            for turn in &reports {
                println!("{} {}: {}", turn.script, turn.kind, turn.verdict);
                println!("  user: {}", turn.user);
                println!("  reply: {}", turn.reply);
            }
            if strict && reports.iter().any(|t| t.verdict != "PASS") {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            }
        }
        "roundtrip" => finish(Vec::new(), roundtrip::run(), out.as_deref(), strict),
        "report" => {
            let mut engine = FakeReplyEngine::new();
            let golden = run_golden(&mut engine);
            finish(golden, roundtrip::run(), out.as_deref(), strict)
        }
        other => {
            eprintln!("unknown command {other}");
            print_help();
            ExitCode::from(2)
        }
    }
}

fn finish(
    golden: Vec<galion_eval_golden::golden::TurnReport>,
    trip: galion_eval_golden::roundtrip::RoundTrip,
    out: Option<&str>,
    strict: bool,
) -> ExitCode {
    let golden_empty = golden.is_empty();
    let report = if golden_empty {
        let mut engine = FakeReplyEngine::new();
        build(run_golden(&mut engine), trip)
    } else {
        build(golden, trip)
    };
    println!("{}", report::summary(&report));
    let json = report::to_json(&report);
    if let Some(path) = out {
        if let Err(err) = fs::write(path, &json) {
            eprintln!("could not write {path}: {err}");
            return ExitCode::from(1);
        }
        eprintln!("wrote {path}");
    }
    if strict {
        let measured_fail = report.roundtrip.wer.status.label() == "FAIL"
            || report.roundtrip.ecapa.status.label() == "FAIL"
            || report.roundtrip.emotion.status.label() == "FAIL"
            || report.roundtrip.tts_first_audio.status.label() == "FAIL"
            || report.roundtrip.asr_first_partial.status.label() == "FAIL"
            || report.golden.iter().any(|t| t.verdict != "PASS");
        if measured_fail {
            return ExitCode::from(1);
        }
    }
    ExitCode::SUCCESS
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn print_help() {
    println!(
        "eval-golden — quality gates for the configured persona\n\
         \n\
         Commands:\n\
           report [--out FILE] [--strict]    golden scripts plus the Piper round-trip\n\
           golden                            text scripts only (fake reply engine)\n\
           roundtrip [--out FILE]            TTS -> ASR -> WER, plus ECAPA and emotion slots\n\
           lint --text LINE [--user LINE]    persona-tone lint\n\
         \n\
         PASS, FAIL, and UNMEASURED are separate. An absent scorer is not a pass.\n\
         ECAPA is a strict greater-than 0.7057. WER must be <= 0.018. emotion_match >= 0.83.\n\
         Reference audio is read from GALION_VOICE_REF. GALION_SAMANTHA_REF is a\n\
         deprecated fallback. The file is never written into the repo.\n\
         --strict exits 1 when a measured check fails. Default exit is 0 after an honest report."
    );
}
