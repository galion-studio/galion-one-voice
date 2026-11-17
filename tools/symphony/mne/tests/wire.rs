// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! The eight S1 fixtures, synthetic audio only.

use galion_mne::{layer_status, repo_root, run_fixtures};

#[test]
fn landed_layers_are_the_seven_and_the_guard_is_waiting() {
    let layers = layer_status(&repo_root());
    let waiting: Vec<_> = layers
        .iter()
        .filter(|s| s.state == "waiting")
        .map(|s| s.owner)
        .collect();
    assert_eq!(waiting, vec!["S9"]);
}

#[test]
fn eight_fixtures_score_through_listen_back() {
    let scores = run_fixtures(&repo_root()).unwrap();
    assert_eq!(scores.len(), 8);
    for row in &scores {
        println!(
            "{} feeling={} pitch_err={:?} breath={} quality={} backend={} guard={}",
            row.id,
            row.feeling,
            row.pitch_error_st,
            row.breath,
            row.pitch_quality,
            row.backend,
            row.guard
        );
        assert!(row.feeling.is_finite(), "{}", row.id);
        assert!(row.pitch_quality.is_finite(), "{}", row.id);
        assert!(row.breath.is_finite(), "{}", row.id);
        assert!(row.samples > 0, "{}", row.id);
        assert!(row.readback_ok, "{}", row.id);
        assert_eq!(row.guard, "waiting");
        assert!(
            row.backend == "piper" || row.backend == "piper-stand-in",
            "{} backend {}",
            row.id,
            row.backend
        );
        if row.planned_inhales > 0 {
            assert!(
                row.breath > 0.0,
                "{} planned {} inhales but breath={}",
                row.id,
                row.planned_inhales,
                row.breath
            );
        }
    }
}
