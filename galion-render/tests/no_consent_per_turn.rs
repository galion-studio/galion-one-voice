// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! A live turn with an already-cloned voice makes no consent call.
//!
//! A fake `python3` heads PATH and writes a marker if anything runs it. The
//! floor is built with a clip store set (the case that used to spawn
//! `consent.py check` twice), a voice is recorded as cloned, and turns run.
//! Pass: no marker, the consent evaluation counter does not move, and the
//! turn still reports the clone as live. One test, so the process-wide PATH
//! and counter are not shared with a parallel test.

use std::fs;
use std::os::unix::fs::PermissionsExt;

#[test]
fn live_turns_with_a_cloned_voice_spawn_no_consent_process() {
    let dir = std::env::temp_dir().join(format!("galion-render-no-consent-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("bin")).unwrap();
    fs::create_dir_all(dir.join("store")).unwrap();
    let marker = dir.join("python3-ran");
    let shim = dir.join("bin/python3");
    fs::write(&shim, format!("#!/bin/sh\necho \"$@\" >> '{}'\nexit 1\n", marker.display())).unwrap();
    fs::set_permissions(&shim, fs::Permissions::from_mode(0o755)).unwrap();
    let path = std::env::var("PATH").unwrap_or_default();
    std::env::set_var("PATH", format!("{}:{path}", dir.join("bin").display()));
    std::env::set_var("GALION_CLIP_STORE", dir.join("store"));
    std::env::remove_var("GALION_LLM_URL");

    // Clone time (done once by the clone slot): record the verdict.
    galion_consent::global().record(
        "samantha",
        galion_consent::Verdict::Approved(galion_consent::Approval {
            persona: "samantha",
            seconds: 10.0,
            public: false,
            cc0: false,
            wav: dir.join("store/samantha/reference.wav"),
            sha256: "0".repeat(64),
        }),
    );

    let before = galion_consent::evaluations();
    let mut floor = galion_render::v2_persona::Floor::new();
    for i in 0..50 {
        let said = floor.say_turn(&format!("Turn {i}, how are you?"), false, 0.2, true);
        assert!(said.clone_live, "turn {i} lost the cloned voice");
        assert_eq!(said.rights, "cleared");
    }
    let state = floor.state_json();
    assert!(state.contains("\"clone_live\":true"), "{state}");

    assert_eq!(galion_consent::evaluations(), before, "a turn evaluated consent");
    let ran = fs::read_to_string(&marker).unwrap_or_default();
    assert!(ran.is_empty(), "python3 ran during floor build or a turn: {ran}");
    let _ = fs::remove_dir_all(&dir);
}
