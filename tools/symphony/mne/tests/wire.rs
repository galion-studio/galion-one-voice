// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Synthetic tone only. No download, no dataset, no person audio.

use std::path::PathBuf;

use galion_mne::{layer_status, repo_root, run, RunIn};

#[test]
fn layer_slots_are_waiting_on_their_owners() {
    let layers = layer_status(&repo_root());
    assert_eq!(layers.len(), 7);
    assert!(layers.iter().all(|s| s.state == "waiting"), "{layers:?}");
    assert_eq!(layers[0].crate_name, "galion-mne-contract");
}

#[test]
fn stacked_drafts_run_one_pass_on_a_synthetic_tone() {
    let repo = repo_root();
    let voice = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../voice-v2/voices/synthetic.toml");
    let report = run(&RunIn {
        repo: &repo,
        text: "Hello. This is a short line.",
        pin: "embarrassment",
        voice: &voice,
    })
    .unwrap();
    assert_eq!(report.tick_ms, 80);
    assert!(report.lungs.n_ticks >= 1);
    assert!(report.lungs.not_emotion);
    assert_eq!(report.clone_request_patch["emotion"], "embarrassment");
    assert_eq!(report.clone_request_patch["track"], "prosody");
    assert!(report.listen.iterations >= 1 && report.listen.iterations <= 2);
    assert_eq!(report.listen.profile.schema, "galion-voice-decode/2");
    assert!((report.voice_f0_hz - 180.0).abs() < 1e-6);
}
