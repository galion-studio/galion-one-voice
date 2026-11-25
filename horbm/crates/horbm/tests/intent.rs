// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Typed intent, through the public brain. Recall speaks a person hit or the
//! honest miss. A correction does not repeat the prior clause and does not
//! count a learn row. The five older labels keep the dialogue moves.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use horbm::{Brain, CardHit, CardLookup, DialogueConsent, EpisodicStore, Persona, UNKNOWN_REPLY};

const NORTHWIND: &str = r#"
schema = "galion-horbm-persona/1"
version = 1
id = "northwind"
voice = "northwind"

[traits]
warmth = 0.61

[honesty]
never_fabricate = true
unknown = "I don't know."

[boundaries]
forbid = ["fake-sympathy", "unsourced-fact"]

[style]
brevity = 0.5
warmth = 0.62
max_sentences = 2

[baseline]
valence = 0.1
arousal = 0.31
dominance = 0.4
approach = 0.0
pitch_centre_hz = 216.0

[inner]
care = "heard"
want = "answer"
expect = "pause"
"#;

const PIN: &str = "you remember the blue bowl is mine";

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "horbm-intent-{name}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).expect("scratch");
        Self(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Kettle;

impl CardLookup for Kettle {
    fn lookup(&self, query: &str) -> Option<CardHit> {
        if query.to_ascii_lowercase().contains("kettle") {
            Some(CardHit {
                id: "kettle".into(),
                gist: "The kettle is on the hob.".into(),
            })
        } else {
            None
        }
    }
}

fn northwind() -> Persona {
    Persona::parse(NORTHWIND).expect("northwind persona")
}

fn names(dir: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let Ok(rd) = fs::read_dir(dir) else {
        return found;
    };
    for entry in rd.flatten() {
        found.push(entry.file_name().to_string_lossy().into_owned());
    }
    found.sort();
    found
}

#[test]
fn recall_with_memory_speaks_the_person_hit() {
    let dir = Scratch::new("recall-grant");
    let mut brain = Brain::with_persona(Kettle, None, northwind()).with_memory(
        EpisodicStore::open(dir.path().join("person-memory.sqlite"), true),
    );
    assert_eq!(brain.persona().id(), "northwind");
    brain.set_person("p1");
    let pinned = brain.turn("remember that you remember the blue bowl is mine");
    assert_eq!(pinned.reply, UNKNOWN_REPLY);
    assert!(pinned
        .sources
        .iter()
        .all(|source| !source.starts_with("memory:")));
    let recalled = brain.turn("do you remember the blue bowl");
    assert_eq!(recalled.reply, PIN);
    assert_eq!(recalled.arm, "ask");
    assert!(recalled
        .sources
        .iter()
        .any(|source| source.starts_with("memory:") && source != "memory:callback"));
    assert!(recalled
        .sources
        .iter()
        .all(|source| !source.starts_with("card:")));
    assert!(!recalled.reply.to_ascii_lowercase().contains("hob"));
    let card = brain.turn("do you remember where the kettle is");
    assert_eq!(card.reply, UNKNOWN_REPLY);
    assert_eq!(card.arm, "ask");
    assert_eq!(card.sources, vec!["honest:unknown".to_string()]);
    assert!(card.evidence.is_empty());
    assert!(card
        .sources
        .iter()
        .all(|source| !source.starts_with("card:") && !source.starts_with("memory:")));
    assert!(!card.reply.to_ascii_lowercase().contains("hob"));
    assert!(!card.reply.to_ascii_lowercase().contains("kettle"));
}

#[test]
fn recall_without_memory_is_exactly_unknown() {
    let dir = Scratch::new("recall-deny");
    let path = dir.path().join("person-memory.sqlite");
    let mut brain = Brain::with_persona(Kettle, None, northwind())
        .with_memory(EpisodicStore::open(&path, false));
    assert_eq!(brain.persona().id(), "northwind");
    brain.set_person("p1");
    let out = brain.turn("do you remember where the kettle is");
    assert_eq!(out.reply, UNKNOWN_REPLY);
    assert_eq!(out.reply, "I don't know.");
    assert_eq!(out.arm, "ask");
    assert_eq!(out.sources, vec!["honest:unknown".to_string()]);
    assert!(out.evidence.is_empty());
    assert!(!out.reply.to_ascii_lowercase().contains("hob"));
    assert!(!out.reply.to_ascii_lowercase().contains("kettle"));
    assert!(out
        .sources
        .iter()
        .all(|source| !source.starts_with("card:")));
    assert!(!path.exists(), "remember denied creates no diary file");
    assert!(names(dir.path()).is_empty(), "{:?}", names(dir.path()));
}

#[test]
fn correction_never_repeats_the_card_clause_and_counts_no_learn_row() {
    let dir = Scratch::new("correct");
    let mut brain = Brain::with_persona(Kettle, None, northwind());
    assert_eq!(brain.persona().id(), "northwind");
    brain.set_bandit_enabled(true);
    brain.set_dialogue_consent(DialogueConsent {
        remember: false,
        learn: true,
        check_ins: false,
    });
    assert!(brain.dialogue_consent().learn);
    let answer = brain.turn("where is the kettle");
    assert_eq!(answer.arm, "answer");
    assert_eq!(answer.reply, "The kettle is on the hob.");
    assert_eq!(answer.sources, vec!["card:kettle".to_string()]);
    assert_eq!(brain.bandit_updates(), 1);
    assert_eq!(brain.bandit_counts()[horbm::Move::Answer.index()], 1);
    let wrong = brain.turn("no, that's wrong");
    assert_eq!(wrong.arm, "ask");
    assert_eq!(wrong.reply, UNKNOWN_REPLY);
    assert_eq!(wrong.sources, vec!["honest:unknown".to_string()]);
    assert!(wrong.evidence.is_empty());
    assert_ne!(wrong.reply, "The kettle is on the hob.");
    assert!(!wrong.reply.to_ascii_lowercase().contains("hob"));
    assert!(!wrong.reply.to_ascii_lowercase().contains("kettle"));
    assert!(wrong
        .sources
        .iter()
        .all(|source| !source.starts_with("card:")));
    assert_eq!(brain.bandit_updates(), 1);
    assert_eq!(brain.bandit_counts()[horbm::Move::Answer.index()], 1);
    assert_eq!(brain.bandit_counts()[horbm::Move::Ask.index()], 0);
    assert!(
        names(dir.path()).is_empty(),
        "a correction writes no learn row: {:?}",
        names(dir.path())
    );
}

#[test]
fn five_old_labels_keep_their_moves() {
    let dir = Scratch::new("labels");
    let mut brain = Brain::with_persona(Kettle, None, northwind());
    assert_eq!(brain.persona().id(), "northwind");
    let steps = [
        (
            "where is the kettle",
            "answer",
            "The kettle is on the hob.",
            "card:kettle",
            0.8,
        ),
        (
            "I am so tired",
            "comfort",
            UNKNOWN_REPLY,
            "honest:unknown",
            0.5,
        ),
        (
            "something else",
            "offer_topic",
            "Back to kettle?",
            "topic:kettle",
            0.6,
        ),
        (
            "cite that",
            "cite",
            "The kettle is on the hob.",
            "card:kettle",
            0.7,
        ),
        ("goodbye", "stop", "Talk soon.", "persona:talk-soon", 0.9),
        (
            "who won the match",
            "ask",
            UNKNOWN_REPLY,
            "honest:unknown",
            0.4,
        ),
    ];
    for (heard, arm, reply, source, propensity) in steps {
        let out = brain.turn(heard);
        assert_eq!(out.arm, arm, "{heard}");
        assert_eq!(out.reply, reply, "{heard}");
        assert_eq!(out.sources, vec![source.to_string()], "{heard}");
        assert_eq!(out.propensity, propensity, "{heard}");
    }
    assert!(names(dir.path()).is_empty(), "{:?}", names(dir.path()));
}
