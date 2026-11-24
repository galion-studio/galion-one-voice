// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Boot reads the persona once. An invalid file is refused.

use std::path::PathBuf;

use horbm::{
    Brain, CardHit, CardLookup, InnerLife, MoveClaim, MoveVerdict, PersonaError, VoiceBaseline,
    UNKNOWN_LINE, UNKNOWN_REPLY,
};

struct NoCards;

impl CardLookup for NoCards {
    fn lookup(&self, _query: &str) -> Option<CardHit> {
        None
    }
}

fn scratch(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "horbm-boot-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ))
}

fn samantha() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../horbm-persona/personas/samantha.toml")
}

fn boot_err(
    root: &std::path::Path,
    knowledge: &std::path::Path,
    persona: &std::path::Path,
) -> PersonaError {
    match Brain::boot(root, knowledge, &root.join("cases"), Some(persona)) {
        Err(err) => err,
        Ok(_) => panic!("boot accepted an invalid persona"),
    }
}

fn bare() -> MoveClaim<'static> {
    MoveClaim {
        tag: b"fact",
        claims_fact: true,
        has_card: false,
        sympathy_script: false,
    }
}

#[test]
fn invalid_persona_refused_at_boot() {
    let root = scratch("bad");
    std::fs::create_dir(&root).unwrap();
    let bad = root.join("bad.toml");
    std::fs::write(&bad, "schema = \"nope\"\nversion = 1\n").unwrap();
    let knowledge = root.join("knowledge");
    let err = boot_err(&root, &knowledge, &bad);
    assert_eq!(
        err,
        PersonaError::Invalid("persona file does not match the schema")
    );
    assert!(!knowledge.exists());

    let text = std::fs::read_to_string(samantha())
        .unwrap()
        .replace("never_fabricate = true", "never_fabricate = false");
    let lied = root.join("lied.toml");
    std::fs::write(&lied, text).unwrap();
    let err = boot_err(&root, &knowledge, &lied);
    assert_eq!(err, PersonaError::Invalid("never_fabricate must be true"));
    assert!(!knowledge.exists());

    let missing = root.join("missing.toml");
    let err = boot_err(&root, &knowledge, &missing);
    assert_eq!(err, PersonaError::Unreadable);
    assert!(!knowledge.exists());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn boot_without_a_file_loads_samantha() {
    let root = scratch("none");
    std::fs::create_dir(&root).unwrap();
    let knowledge = root.join("knowledge");
    let brain = Brain::boot(&root, &knowledge, &root.join("cases"), None).unwrap();
    assert!(!knowledge.exists());
    assert_eq!(brain.persona().id(), "samantha");
    assert_eq!(brain.emotion().baseline().persona_id(), "samantha");
    assert_ne!(
        brain.emotion().baseline().identity_bits(),
        VoiceBaseline::standin().identity_bits()
    );
    assert_eq!(brain.inner().care(0).unwrap().token_str(), "heard");
    assert_eq!(brain.admit_move(bare()), MoveVerdict::Forbid);
    assert_eq!(brain.persona().unknown_line(), UNKNOWN_REPLY);
    assert_eq!(UNKNOWN_LINE, UNKNOWN_REPLY);
    let open = Brain::open(&root, &knowledge, &root.join("cases"));
    assert_eq!(open.persona().id(), "samantha");
    let cards = Brain::with_cards(NoCards, None);
    assert_eq!(cards.persona().id(), "samantha");
    assert_ne!(cards.persona().id(), "aria");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn boot_loads_samantha_and_keeps_her_while_the_moment_swings() {
    let root = scratch("sam");
    std::fs::create_dir(&root).unwrap();
    let knowledge = root.join("knowledge");
    let mut brain = Brain::boot(&root, &knowledge, &root.join("cases"), Some(&samantha())).unwrap();
    assert!(!knowledge.exists());
    assert_eq!(brain.persona().id(), "samantha");
    assert_eq!(brain.emotion().baseline().persona_id(), "samantha");
    assert_eq!(brain.persona().unknown_line(), UNKNOWN_REPLY);
    assert_eq!(brain.inner().care(0).unwrap().token_str(), "heard");
    let before = *brain.persona();
    let style = brain.persona().style();
    let bits = brain.emotion().baseline().identity_bits();
    let nudged = brain.inner_nudge("what?", None).unwrap();
    assert!(nudged.surprise > 0.0);
    assert_eq!(*brain.persona(), before);
    assert_eq!(brain.persona().style(), style);
    assert_eq!(brain.emotion().baseline().identity_bits(), bits);
    assert_ne!(brain.emotion().moment().pos[1].to_bits(), bits.pos[1]);
    assert_eq!(brain.inner().learned_count(), 0);
    assert_eq!(brain.admit_move(bare()), MoveVerdict::Forbid);
    let card = MoveClaim {
        tag: b"fact",
        claims_fact: true,
        has_card: true,
        sympathy_script: false,
    };
    assert_eq!(brain.admit_move(card), MoveVerdict::Allow);

    let text = std::fs::read_to_string(samantha())
        .unwrap()
        .replace("care = \"heard\"", "care = \"focus\"");
    let file = root.join("focus.toml");
    std::fs::write(&file, text).unwrap();
    let focused = Brain::boot(&root, &knowledge, &root.join("cases"), Some(&file)).unwrap();
    assert_eq!(focused.persona().id(), "samantha");
    assert_eq!(focused.inner().care(0).unwrap().token_str(), "focus");
    assert_eq!(focused.inner().want(0).unwrap().token_str(), "answer");
    assert_ne!(
        focused.inner().care(0).unwrap().token_str(),
        InnerLife::seeded().care(0).unwrap().token_str()
    );
    let _ = std::fs::remove_dir_all(&root);
}
