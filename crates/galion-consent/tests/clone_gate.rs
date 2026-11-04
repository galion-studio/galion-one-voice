// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! The clone-time gate: valid consent passes; missing, tampered, expired or
//! celebrity consent refuses with a clear error and records the refusal.

mod common;

use common::Tree;
use galion_consent::{clone_gate_into, evaluate, Registry, Verdict};
use std::fs;

#[test]
fn valid_consent_clones_and_is_recorded() {
    let t = Tree::new("valid");
    t.place("samantha");
    t.place("jarvis");
    let reg = Registry::new();
    for p in ["samantha", "jarvis"] {
        let ok = clone_gate_into(&reg, p, &t.env()).expect("valid consent must clone");
        assert_eq!(ok.persona, p);
        assert!((ok.seconds - 10.0).abs() < 1e-9);
        assert!(!ok.public);
        assert_eq!(ok.sha256.len(), 64);
        assert!(reg.approved(p));
    }
}

#[test]
fn public_demo_yes_is_carried() {
    let t = Tree::new("public");
    t.place_with("samantha", &common::wav_bytes(16_000, 1, 2, 10.0), |f| {
        f.iter_mut().find(|(k, _)| k == "public_demo").unwrap().1 = "yes".into();
    });
    assert!(clone_gate_into(&Registry::new(), "samantha", &t.env()).unwrap().public);
}

fn refused(t: &Tree, persona: &str) -> (String, &'static str, Registry) {
    let reg = Registry::new();
    let err = clone_gate_into(&reg, persona, &t.env()).expect_err("clone must be refused");
    (err.to_string(), err.reason, reg)
}

#[test]
fn missing_consent_is_refused() {
    let t = Tree::new("missing");
    t.place("samantha");
    fs::remove_file(t.folder("samantha").join("consent.txt")).unwrap();
    let (msg, reason, reg) = refused(&t, "samantha");
    assert_eq!(reason, "consent");
    assert!(msg.starts_with("clone refused for samantha:"), "{msg}");
    assert!(msg.contains("consent gate: consent"), "{msg}");
    assert_eq!(reg.lookup("samantha"), Some(Verdict::Refused("consent")));
    assert!(!reg.approved("samantha"));
}

#[test]
fn missing_field_or_placeholder_is_refused() {
    for (key, value) in [("counsel", "TBD"), ("performer", "[name]"), ("scope", "public-tts"), ("date", "2026-11-04 10:00 UTC")] {
        let t = Tree::new("field");
        t.place("samantha");
        t.set("samantha", key, value);
        assert_eq!(refused(&t, "samantha").1, "consent", "{key}={value}");
    }
}

#[test]
fn missing_wav_or_store_is_refused() {
    let t = Tree::new("nowav");
    assert_eq!(refused(&t, "samantha").1, "wav");
    let mut env = t.env();
    env.store = None;
    assert_eq!(clone_gate_into(&Registry::new(), "samantha", &env).unwrap_err().reason, "store");
}

#[test]
fn tampered_reference_is_refused() {
    let t = Tree::new("tamper");
    t.place("samantha");
    let wav = t.folder("samantha").join("reference.wav");
    let mut bytes = fs::read(&wav).unwrap();
    let mid = bytes.len() / 2;
    bytes[mid] ^= 0x55;
    fs::write(&wav, bytes).unwrap();
    let (msg, reason, _) = refused(&t, "samantha");
    assert_eq!(reason, "sha");
    assert!(msg.contains("sha256"), "{msg}");
}

#[test]
fn tampered_sidecar_or_declared_hash_is_refused() {
    let t = Tree::new("sidecar");
    t.place("samantha");
    fs::write(t.folder("samantha").join("reference.wav.sha256"), format!("{}  reference.wav\n", "a".repeat(64))).unwrap();
    assert_eq!(refused(&t, "samantha").1, "sha");
    let t = Tree::new("declared");
    t.place("samantha");
    t.set("samantha", "sha256", &"b".repeat(64));
    assert_eq!(refused(&t, "samantha").1, "sha");
}

#[test]
fn consent_for_another_persona_is_refused() {
    let t = Tree::new("swap");
    t.place("samantha");
    t.set("samantha", "persona", "jarvis");
    assert_eq!(refused(&t, "samantha").1, "consent");
}

#[test]
fn expired_consent_is_refused() {
    let t = Tree::new("expired");
    t.place("samantha");
    t.set("samantha", "expiry", "2026-11-03");
    let (msg, reason, reg) = refused(&t, "samantha");
    assert_eq!(reason, "expired");
    assert!(msg.contains("expired"), "{msg}");
    assert_eq!(reg.lookup("samantha"), Some(Verdict::Refused("expired")));
    // The expiry day itself is still valid; free-text expiry is not a date.
    t.set("samantha", "expiry", "2026-11-04");
    assert!(clone_gate_into(&Registry::new(), "samantha", &t.env()).is_ok());
    t.set("samantha", "expiry", "on written withdrawal");
    assert!(clone_gate_into(&Registry::new(), "samantha", &t.env()).is_ok());
}

#[test]
fn armstrong_stays_blocked() {
    let t = Tree::new("armstrong");
    t.place("samantha");
    for p in ["armstrong", "Neil Armstrong", "ARMSTRONG-moon"] {
        let (msg, reason, _) = refused(&t, p);
        assert_eq!(reason, "celebrity", "{p}");
        assert!(msg.contains("celebrity") || msg.contains("blocked"), "{msg}");
    }
    // A consented clip that names the blocked voice is refused too.
    for key in ["performer", "work", "grantor"] {
        let t = Tree::new("armstrong-field");
        t.place("samantha");
        t.set("samantha", key, "Neil Armstrong");
        assert_eq!(refused(&t, "samantha").1, "celebrity", "{key}");
    }
}

#[test]
fn closed_lock_refuses_even_with_consent() {
    let t = Tree::new("lock");
    t.place("samantha");
    fs::write(t.repo.join("LOCKS.toml"), "[formats.exceptions.v2s14_chatterbox]\nreference_clips = false\n").unwrap();
    assert_eq!(refused(&t, "samantha").1, "lock");
}

#[test]
fn evaluation_counter_moves_once_per_gate() {
    let t = Tree::new("count");
    t.place("samantha");
    let before = galion_consent::evaluations();
    let _ = evaluate("samantha", &t.env(), true);
    assert!(galion_consent::evaluations() > before);
}
