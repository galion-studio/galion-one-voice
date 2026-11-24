// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

use horbm_eval::{research_dir_is_unset, run, Thresholds};

#[test]
fn embedded_thresholds_are_the_r4_gate() {
    let thresholds = Thresholds::embedded();
    assert_eq!(thresholds.schema, "horbm-eval/1");
    assert_eq!(thresholds.persona.id, "samantha");
    assert_eq!(thresholds.grounding.unknown_reply, "I don't know.");
    assert_eq!(thresholds.grounding.card_prefix, "card:");
    assert_eq!(thresholds.grounding.research_prefix, "research:");
    assert_eq!(thresholds.emotion.frame_ms, 80);
    assert_eq!(thresholds.emotion.frame_budget_us, 80_000);
    assert_eq!(thresholds.emotion.sample_rate_hz, 16_000);
    assert_eq!(thresholds.latency.clock, "virtual");
    assert_eq!(thresholds.latency.frame_ms, 80);
    assert_eq!(thresholds.latency.eager_to_voice_p50_max_ms, 200);
    assert_eq!(thresholds.latency.eager_to_voice_p95_max_ms, 240);
    assert_eq!(thresholds.latency.barge_in_max_ms, 200);
    assert!(!thresholds.latency.backchannel_cancels);
}

#[test]
fn a_set_research_dir_is_not_an_answer_source() {
    assert!(research_dir_is_unset(None));
    assert!(research_dir_is_unset(Some("  ")));
    assert!(!research_dir_is_unset(Some("/tmp/research")));
    let report = horbm_eval::run_with(horbm_eval::Config {
        research_dir: Some("notes".into()),
        thresholds: Thresholds::embedded(),
    });
    assert!(!report.pass);
    assert_eq!(report.research_dir, "set");
    assert!(report.truth_cards.is_empty());
}

#[test]
fn the_gate_passes() {
    let report = run();
    eprintln!("{}", report.to_json());
    assert!(report.pass, "{}", report.to_json());
    assert_eq!(report.research_dir, "unset");
    let latency = report.scores.latency.detail.expect("latency detail");
    assert!(latency["eager_to_voice_p50_ms"].as_u64().unwrap() <= 200);
    assert!(latency["eager_to_voice_p95_ms"].as_u64().unwrap() <= 240);
    assert!(latency["barge_in_ms"].as_u64().unwrap() <= 200);
    assert_eq!(latency["backchannel_cancel"], false);
    assert_eq!(latency["clock"], "virtual");
}

#[test]
fn the_binary_exits_zero() {
    let exe = env!("CARGO_BIN_EXE_horbm-eval");
    let output = std::process::Command::new(exe)
        .output()
        .expect("run horbm-eval");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{stdout}");
    assert!(stdout.contains("\"pass\": true"), "{stdout}");
    assert!(stdout.contains("\"research_dir\": \"unset\""), "{stdout}");
}
