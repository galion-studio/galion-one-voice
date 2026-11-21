// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

use std::time::{Duration, Instant};

use samantha_brain::{
    horbm_linked, speakable_chunks, FakeEngine, Session, FRAME_MS, HORBM_LANDING_PATH,
};

#[test]
fn license_matches_the_repo_and_this_crate_is_not_a_workspace_member() {
    let ours = include_str!("../LICENSE");
    let repo = include_str!("../../../LICENSE");
    assert_eq!(ours, repo);
    let root = include_str!("../../../Cargo.toml");
    assert!(
        !root.contains("samantha-brain"),
        "samantha-brain must stay out of the root workspace"
    );
    assert!(
        !root.contains("s04-horbm-bridge"),
        "this sector must stay out of the root workspace"
    );
}

#[test]
fn landing_path_is_the_one_in_the_sector_brief() {
    assert_eq!(HORBM_LANDING_PATH, "../../horbm/crates/horbm");
    let manifest = include_str!("../Cargo.toml");
    assert!(
        manifest.contains("path = \"../../horbm/crates/horbm\""),
        "Cargo.toml must keep the landing path"
    );
    assert_eq!(FRAME_MS, 80);
}

#[test]
fn a_session_speaks_the_echo_in_sentence_chunks() {
    let mut session = Session::new(FakeEngine::new());
    let spoken = session.turn("Mr. Smith left the kettle on. It can wait.");
    assert_eq!(spoken.chunks.len(), 2);
    assert_eq!(spoken.chunks[0].text, "Mr. Smith left the kettle on.");
    assert_eq!(spoken.chunks[1].text, "It can wait.");
    assert_eq!(speakable_chunks("Pi is 3.14 today.").len(), 1);
}

#[cfg(not(feature = "horbm"))]
#[test]
fn the_default_build_does_not_link_horbm() {
    assert!(!horbm_linked());
}

#[cfg(feature = "horbm")]
#[test]
fn the_feature_build_links_horbm() {
    assert!(horbm_linked());
}

#[test]
fn dropping_the_mood_thread_joins() {
    let tick = samantha_brain::MoodTick::spawn(FakeEngine::new()).expect("spawn");
    let started = Instant::now();
    drop(tick);
    let waited = started.elapsed();
    assert!(
        waited < Duration::from_millis(2_000),
        "mood thread did not join, waited {waited:?}"
    );
}
