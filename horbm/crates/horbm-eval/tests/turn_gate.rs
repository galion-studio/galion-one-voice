// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! The virtual-clock gate. Numbers come from `Duplex::tick`, not from a table
//! that was typed in ahead of a run.

use horbm_duplex::Persona;
use horbm_eval::{run_stand_in, StageState, FRAME_SAMPLES};

fn persona_file_stems() -> Vec<String> {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../horbm-persona/personas");
    let mut stems = Vec::new();
    for entry in std::fs::read_dir(&dir).unwrap_or_else(|err| panic!("{}: {err}", dir.display())) {
        let path = entry.expect("persona entry").path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
            continue;
        }
        if let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) {
            stems.push(stem.to_string());
        }
    }
    assert!(!stems.is_empty());
    stems
}

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
    let configured = horbm_duplex::ConfiguredPersona::development()
        .expect("configured persona")
        .id()
        .to_string();
    assert!(text.contains(&format!("persona={configured}")));
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
    for word in ["kettle", "hob", "window", "garden", "yeah", "mm-hm"] {
        assert!(!text.contains(word), "{word} leaked into\n{text}");
    }
    let lower = text.to_ascii_lowercase();
    for stem in persona_file_stems() {
        assert!(!lower.contains(&stem), "{stem} leaked into\n{text}");
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
fn the_report_names_the_configured_persona() {
    let report = run_stand_in();
    let id = horbm_duplex::ConfiguredPersona::development()
        .expect("configured persona")
        .id()
        .to_string();
    assert!(report.text().contains(&format!("persona={id}")));
}
