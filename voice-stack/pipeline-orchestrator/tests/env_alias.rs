// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Alias rule for the reference-path variables.
//!
//! Pure pairs of `Option<&str>`. Nothing here calls `set_var`.

use galion_sam_orch::voice_ref_choice;

#[test]
fn new_name_wins() {
    let only_new = voice_ref_choice(Some("voice-ref"), None);
    assert_eq!(only_new.value, Some("voice-ref"));
    assert!(only_new.warning.is_none());

    let same = voice_ref_choice(Some("voice-ref"), Some("voice-ref"));
    assert_eq!(same.value, Some("voice-ref"));
    assert!(same.warning.is_none());
}

#[test]
fn old_name_is_a_fallback_with_one_warning() {
    let choice = voice_ref_choice(None, Some("old-ref"));
    assert_eq!(choice.value, Some("old-ref"));
    let warning = choice.warning.expect("one warning");
    assert_eq!(warning.lines().count(), 1);
    assert!(warning.contains("GALION_VOICE_REF"), "{warning}");
}

#[test]
fn both_set_and_differ_new_wins() {
    let choice = voice_ref_choice(Some("voice-ref"), Some("old-ref"));
    assert_eq!(choice.value, Some("voice-ref"));
    let warning = choice.warning.expect("one warning");
    assert_eq!(warning.lines().count(), 1);
    assert!(warning.contains("GALION_VOICE_REF"), "{warning}");
    assert_ne!(choice.value, Some("old-ref"));
}

#[test]
fn neither_is_none() {
    let choice = voice_ref_choice(None, None);
    assert_eq!(choice.value, None);
    assert!(choice.warning.is_none());

    let empty = voice_ref_choice(Some(""), Some(""));
    assert_eq!(empty.value, None);
    assert!(empty.warning.is_none());
}

#[test]
fn samantha_traits_alias_still_exports_the_same_items() {
    let gate: std::sync::Arc<galion_sam_orch::samantha_traits::WaitGate> =
        galion_sam_orch::samantha_traits::WaitGate::new();
    assert!(!gate.release.load(std::sync::atomic::Ordering::SeqCst));
    assert_eq!(
        galion_sam_orch::samantha_traits::SAMANTHA_REF_ENV,
        "GALION_SAMANTHA_REF"
    );
    assert_eq!(
        galion_sam_orch::samantha_traits::VOICE_REF_ENV,
        "GALION_VOICE_REF"
    );
    let _forward: fn() -> Option<std::path::PathBuf> =
        galion_sam_orch::samantha_traits::samantha_ref_from_env;
    let _voice: fn() -> Option<std::path::PathBuf> =
        galion_sam_orch::samantha_traits::voice_ref_from_env;
}
