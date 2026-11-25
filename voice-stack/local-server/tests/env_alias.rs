// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Pure alias decisions. No process environment.

use samantha_local_server::env_alias::{resolve, warning_text, AliasWarning};

#[test]
fn new_wins() {
    let got = resolve(Some("127.0.0.1:7881"), None);
    assert_eq!(got.value.as_deref(), Some("127.0.0.1:7881"));
    assert!(got.warning.is_none());
    let same = resolve(Some("ui"), Some("ui"));
    assert_eq!(same.value.as_deref(), Some("ui"));
    assert!(
        same.warning.is_none(),
        "equal values are the new name and do not warn"
    );
}

#[test]
fn old_is_fallback_and_warns_once() {
    let got = resolve(None, Some("127.0.0.1:9"));
    assert_eq!(got.value.as_deref(), Some("127.0.0.1:9"));
    let warning = got.warning.expect("the old name warns");
    assert_eq!(warning, AliasWarning::Fallback);
    let line = warning_text("GALION_VOICE_WS", "GALION_SAMANTHA_WS", warning);
    assert_eq!(line.lines().count(), 1, "one warning line");
    assert!(line.contains("GALION_VOICE_WS"));
    assert!(line.contains("deprecated"));
}

#[test]
fn both_differ_new_wins_and_warns() {
    let got = resolve(Some("127.0.0.1:7881"), Some("127.0.0.1:9"));
    assert_eq!(got.value.as_deref(), Some("127.0.0.1:7881"));
    let warning = got.warning.expect("a difference warns");
    assert_eq!(warning, AliasWarning::Conflict);
    let line = warning_text("GALION_VOICE_REF", "GALION_SAMANTHA_REF", warning);
    assert_eq!(line.lines().count(), 1);
    assert!(line.contains("using GALION_VOICE_REF"));
}

#[test]
fn none_is_none() {
    let got = resolve(None, None);
    assert!(got.value.is_none());
    assert!(got.warning.is_none());
}
