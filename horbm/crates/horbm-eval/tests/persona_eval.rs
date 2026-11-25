// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

use std::path::{Path, PathBuf};

use horbm_eval::{run_with, Config, Mode, Report, Thresholds};

fn gate(persona_file: &Path) -> Report {
    run_with(Config {
        research_dir: None,
        thresholds: Thresholds::embedded(),
        mode: Mode::Fake,
        prosody: true,
        persona_file: Some(persona_file.to_path_buf()),
    })
}

fn quoted(text: &str, key: &str) -> String {
    for line in text.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix(key) else {
            continue;
        };
        let rest = rest.trim();
        if !rest.starts_with('=') {
            continue;
        }
        return rest
            .trim_start_matches('=')
            .trim()
            .trim_matches('"')
            .to_string();
    }
    panic!("missing {key}");
}

/// The development boot file's persona, as an explicit path. Does not set
/// `GALION_HORBM_DEV_PERSONA`.
fn configured_persona_file() -> PathBuf {
    let boot = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../horbm-persona/development.toml");
    let text = std::fs::read_to_string(&boot).expect("development boot file");
    let id = quoted(&text, "development_persona");
    let dir = quoted(&text, "persona_dir");
    boot.parent()
        .expect("boot parent")
        .join(dir)
        .join(format!("{id}.toml"))
}

#[test]
fn gate_runs_for_the_configured_persona() {
    let path = configured_persona_file();
    let id = quoted(&std::fs::read_to_string(&path).unwrap(), "id");
    let report = gate(&path);
    let json = report.to_json();
    assert!(report.pass, "{json}");
    assert_eq!(report.persona, id);
    assert!(report.scores.persona.pass, "{json}");
    let detail = report.scores.persona.detail.expect("persona detail");
    assert_eq!(detail["id"].as_str(), Some(id.as_str()));
    assert!(detail.get("voice").is_none(), "{detail}");
    assert!(
        !report
            .scores
            .persona
            .failures
            .iter()
            .any(|line| line == "no voice profile"),
        "{json}"
    );
}

#[test]
fn gate_runs_for_northwind() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/northwind.toml");
    let report = gate(&path);
    let json = report.to_json();
    assert_eq!(report.persona, "northwind");
    assert!(json.contains("\"id\": \"northwind\""), "{json}");
    assert!(!report.scores.persona.pass, "{json}");
    assert_eq!(
        report.scores.persona.failures,
        vec!["no voice profile".to_string()]
    );
    let detail = report.scores.persona.detail.expect("persona detail");
    assert_eq!(detail["id"], "northwind");
    assert_eq!(detail["voice"], "no voice profile");
    assert!(detail.get("window").is_none(), "{detail}");
    assert!(report.scores.grounded.pass, "{json}");
    assert!(report.scores.unknown.pass, "{json}");
    assert!(report.scores.latency.pass, "{json}");
    let grounded = report.scores.grounded.detail.expect("grounded detail");
    let unknown = report.scores.unknown.detail.expect("unknown detail");
    assert!(
        grounded["lines"]
            .as_array()
            .expect("grounded lines")
            .iter()
            .any(|line| line == "The kettle is on the hob."),
        "{grounded}"
    );
    assert!(
        unknown["lines"]
            .as_array()
            .expect("unknown lines")
            .iter()
            .any(|line| line == "I don't know."),
        "{unknown}"
    );
    let lower = json.to_ascii_lowercase();
    assert!(!lower.contains("samantha"), "{json}");
    assert!(!lower.contains("aria"), "{json}");
    assert!(!lower.contains("jarvis"), "{json}");
}

#[test]
fn thresholds_numbers_unchanged() {
    let thresholds = Thresholds::embedded();
    assert_eq!(thresholds.schema, "horbm-eval/1");
    assert_eq!(thresholds.persona.id, None);
    assert_eq!(thresholds.grounding.unknown_reply, "I don't know.");
    assert_eq!(thresholds.grounding.card_prefix, "card:");
    assert_eq!(thresholds.grounding.research_prefix, "research:");
    assert_eq!(thresholds.grounding.honest_source, "honest:unknown");
    assert_eq!(thresholds.emotion.frame_ms, 80);
    assert_eq!(thresholds.emotion.frame_budget_us, 80_000);
    assert_eq!(thresholds.emotion.sample_rate_hz, 16_000);
    assert_eq!(thresholds.emotion.fatigue_arousal_drop_min, 0.2);
    assert_eq!(thresholds.emotion.valence_rise_min, 0.2);
    assert_eq!(thresholds.emotion.frames, 40);
    assert_eq!(thresholds.latency.clock, "virtual");
    assert_eq!(thresholds.latency.frame_ms, 80);
    assert_eq!(thresholds.latency.eager_to_voice_p50_max_ms, 200);
    assert_eq!(thresholds.latency.eager_to_voice_p95_max_ms, 240);
    assert_eq!(thresholds.latency.barge_in_max_ms, 200);
    assert!(!thresholds.latency.backchannel_cancels);
    assert_eq!(thresholds.asr.wer_max, 0.02);
    assert_eq!(thresholds.real.latency.clock, "wall");
    assert_eq!(thresholds.real.latency.frame_ms, 80);
    assert_eq!(thresholds.real.latency.eager_to_voice_p50_max_ms, 200);
    assert_eq!(thresholds.real.latency.eager_to_voice_p95_max_ms, 240);
    assert_eq!(thresholds.real.latency.barge_in_max_ms, 500);
    assert!(!thresholds.real.latency.backchannel_cancels);

    let text = include_str!("../thresholds.toml");
    let assigns_id = text.lines().any(|line| {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.is_empty() {
            return false;
        }
        let key = trimmed.split(['=', ' ']).next().unwrap_or("");
        key == "id"
    });
    assert!(
        !assigns_id,
        "persona id must not be hard-coded in thresholds.toml"
    );
}
