// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! The virtual-clock gate. Numbers come from `Duplex::tick`, not from a table
//! that was typed in ahead of a run.

use horbm_eval::{run_stand_in, StageState, FRAME_SAMPLES};

#[test]
fn turn_eval_gates_hold() {
    let report = run_stand_in();
    let text = report.text();
    println!("{text}");
    assert!(report.passed(), "{text}");
    assert_eq!(report.speech, "stand-in");
    assert!(report.scenarios >= 20);
    assert!(report.samples > 0);
    assert_eq!(report.samples % FRAME_SAMPLES, 0);
    assert!(text.contains("persona=samantha"));
    assert!(text.contains("clock=virtual"));
    assert!(text.contains("cite=arxiv:2503.04721,arxiv:2507.23159"));
    assert!(text.contains("result=pass"));
}

#[test]
fn stub_stages_are_named_and_the_report_has_no_transcript() {
    let report = run_stand_in();
    let text = report.text();
    assert_eq!(report.end_of_turn, StageState::Stub);
    assert_eq!(report.backchannel, StageState::Stub);
    assert_eq!(report.echo, StageState::Live);
    assert!(text.contains("stage end_of_turn=stub"));
    assert!(text.contains("stage backchannel=stub"));
    assert!(text.contains("stage echo_cancel=live"));
    assert!(text.contains("metric backchannel_rate_per_s n=2 p50=stub p95=stub gate=off"));
    assert!(text.contains("metric backchannel_offset_ms n=0 p50=stub p95=stub gate=off"));
    assert!(text.contains("metric false_barge_in n=2 p50=0.000 p95=0.000 gate=on limit=p95<=0.000"));
    assert!(text.contains("metric pause_handling_tor"));
    assert!(text.contains("gate=off"));
    for word in ["kettle", "hob", "window", "garden", "aria", "yeah", "mm-hm"] {
        assert!(!text.contains(word), "{word} leaked into\n{text}");
    }
}

#[test]
fn echo_probe_stays_quiet_when_the_canceller_is_on() {
    let report = run_stand_in();
    assert_eq!(report.echo, StageState::Live);
    assert!(report.echo_wired);
    assert_eq!(report.echo_probe_ran, 2);
    assert_eq!(report.echo_probe_opened, report.echo_probe_ran);
    assert_eq!(report.echo_probe_barged, 0);
    assert!(report.eot_probe_took_floor);
}

#[test]
fn the_report_names_samantha() {
    let report = run_stand_in();
    assert!(report.text().contains("persona=samantha"));
}
