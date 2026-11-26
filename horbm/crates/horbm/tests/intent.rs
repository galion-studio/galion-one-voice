// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Typed intent, through the public brain. Recall speaks a person hit or the
//! honest miss. A correction does not repeat the prior clause and does not
//! count a learn row. A forget phrase does the same, and a negated forget
//! phrase does not. The five older labels keep the dialogue moves.

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

fn learning(dir: &Scratch) -> Brain {
    let mut brain = Brain::with_persona(Kettle, None, northwind()).with_memory(
        EpisodicStore::open(dir.path().join("person-memory.sqlite"), true),
    );
    assert_eq!(brain.persona().id(), "northwind");
    brain.set_person("p1");
    brain.set_bandit_enabled(true);
    brain.set_dialogue_consent(DialogueConsent {
        remember: false,
        learn: true,
        check_ins: false,
    });
    brain
}

fn pin_bowl(brain: &mut Brain) {
    let pinned = brain.turn("remember that you remember the blue bowl is mine");
    assert_eq!(pinned.reply, UNKNOWN_REPLY);
}

#[test]
fn forget_phrases_read_as_forget() {
    let dir = Scratch::new("forget-phrases");
    let mut brain = learning(&dir);
    let kettle = brain.turn("where is the kettle");
    assert_eq!(kettle.arm, "answer");
    assert_eq!(kettle.reply, "The kettle is on the hob.");
    let updates = brain.bandit_updates();
    assert_eq!(updates, 1);
    for heard in [
        "forget that",
        "Forget that.",
        "forget what i said",
        "Forget what I told you",
        "please forget",
        "don't remember that",
        "stop remembering",
    ] {
        let out = brain.turn(heard);
        assert_eq!(out.arm, "ask", "{heard}");
        assert_eq!(out.reply, UNKNOWN_REPLY, "{heard}");
        assert_eq!(out.sources, vec!["honest:unknown".to_string()], "{heard}");
        assert!(out.evidence.is_empty(), "{heard}");
        assert!(!out.reply.to_ascii_lowercase().contains("hob"), "{heard}");
        assert_ne!(out.reply, "Talk soon.", "{heard}");
        assert_eq!(brain.bandit_updates(), updates, "{heard}");
        assert_eq!(
            brain.bandit_counts()[horbm::Move::Ask.index()],
            0,
            "{heard}"
        );
    }
    let chat = brain.turn("thanks");
    assert_eq!(chat.arm, "ask");
    assert_eq!(brain.bandit_updates(), updates + 1);
    assert_eq!(brain.bandit_counts()[horbm::Move::Ask.index()], 1);
}

#[test]
fn dont_forget_is_not_forget() {
    let dir = Scratch::new("dont-forget");
    let mut brain = learning(&dir);
    pin_bowl(&mut brain);
    let updates = brain.bandit_updates();
    for heard in [
        "please don't forget that",
        "never forget that",
        "do not forget that",
        "dont forget that",
    ] {
        let out = brain.turn(heard);
        assert_ne!(out.reply, "Talk soon.", "{heard}");
        assert!(brain.bandit_updates() > updates, "{heard}");
        let recalled = brain.turn(&format!("{heard}, do you remember the blue bowl"));
        assert_eq!(recalled.reply, PIN, "{heard}");
        assert_eq!(recalled.arm, "ask", "{heard}");
        assert!(recalled
            .sources
            .iter()
            .any(|source| source.starts_with("memory:") && source != "memory:callback"));
    }
}

#[test]
fn forgot_and_forget_me_not_are_not_forget() {
    let dir = Scratch::new("forgot");
    let mut brain = learning(&dir);
    pin_bowl(&mut brain);
    let updates = brain.bandit_updates();
    for heard in ["I forgot my keys", "forget-me-not"] {
        let out = brain.turn(heard);
        assert_eq!(out.reply, UNKNOWN_REPLY, "{heard}");
        assert!(brain.bandit_updates() > updates, "{heard}");
        assert!(!out.reply.to_ascii_lowercase().contains("hob"), "{heard}");
        let recalled = brain.turn(&format!("{heard}, do you remember the blue bowl"));
        assert_eq!(recalled.reply, PIN, "{heard}");
        assert!(recalled
            .sources
            .iter()
            .any(|source| source.starts_with("memory:") && source != "memory:callback"));
    }
}

#[test]
fn correct_beats_forget() {
    let dir = Scratch::new("correct-beats-forget");
    let mut brain = learning(&dir);
    pin_bowl(&mut brain);
    let kettle = brain.turn("where is the kettle");
    assert_eq!(kettle.reply, "The kettle is on the hob.");
    let updates = brain.bandit_updates();
    let asks = brain.bandit_counts()[horbm::Move::Ask.index()];
    let out = brain.turn("no, that's wrong, forget that, do you remember the blue bowl");
    assert_eq!(out.arm, "ask");
    assert_eq!(out.reply, UNKNOWN_REPLY);
    assert_eq!(out.sources, vec!["honest:unknown".to_string()]);
    assert!(out.evidence.is_empty());
    assert_ne!(out.reply, PIN);
    assert!(!out.reply.to_ascii_lowercase().contains("bowl"));
    assert!(!out.reply.to_ascii_lowercase().contains("hob"));
    assert_eq!(brain.bandit_updates(), updates);
    assert_eq!(brain.bandit_counts()[horbm::Move::Ask.index()], asks);
}

#[test]
fn forget_beats_recall() {
    let dir = Scratch::new("forget-beats-recall");
    let mut brain = learning(&dir);
    pin_bowl(&mut brain);
    let updates = brain.bandit_updates();
    for heard in [
        "forget that, do you remember the blue bowl",
        "do you remember the blue bowl, please forget",
        "what did I tell you, stop remembering",
    ] {
        let out = brain.turn(heard);
        assert_eq!(out.arm, "ask", "{heard}");
        assert_eq!(out.reply, UNKNOWN_REPLY, "{heard}");
        assert_eq!(out.sources, vec!["honest:unknown".to_string()], "{heard}");
        assert!(out.evidence.is_empty(), "{heard}");
        assert_ne!(out.reply, PIN, "{heard}");
        assert!(!out.reply.to_ascii_lowercase().contains("bowl"), "{heard}");
        assert_eq!(brain.bandit_updates(), updates, "{heard}");
    }
    let recalled = brain.turn("do you remember the blue bowl");
    assert_eq!(recalled.reply, PIN);
    assert!(recalled
        .sources
        .iter()
        .any(|source| source.starts_with("memory:") && source != "memory:callback"));
}

#[test]
fn forget_plan_is_honest_unknown_and_not_counted() {
    let dir = Scratch::new("forget-plan");
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
    let forgotten = brain.turn("please forget what I told you");
    assert_eq!(forgotten.arm, "ask");
    assert_eq!(forgotten.reply, UNKNOWN_REPLY);
    assert_eq!(forgotten.reply, "I don't know.");
    assert_eq!(forgotten.propensity, 0.4);
    assert_eq!(forgotten.sources, vec!["honest:unknown".to_string()]);
    assert!(forgotten.evidence.is_empty());
    assert_ne!(forgotten.reply, "The kettle is on the hob.");
    assert!(!forgotten.reply.to_ascii_lowercase().contains("hob"));
    assert!(!forgotten.reply.to_ascii_lowercase().contains("kettle"));
    assert!(forgotten
        .sources
        .iter()
        .all(|source| !source.starts_with("card:")));
    assert_eq!(brain.bandit_updates(), 1);
    assert_eq!(brain.bandit_counts()[horbm::Move::Answer.index()], 1);
    assert_eq!(brain.bandit_counts()[horbm::Move::Ask.index()], 0);
    assert!(
        names(dir.path()).is_empty(),
        "a forget writes no learn row: {:?}",
        names(dir.path())
    );
}

#[test]
fn learned_source_counts_as_person_memory_for_recall() {
    // `Brain::turn` still tags a person hit `memory:` until S121. The
    // assertion that `learned:<id>` is kept, and that an empty id is the
    // miss, is the policy unit test of this same name. This lock is the
    // source of that check, plus the live recall of a person hit.
    let policy = include_str!("../src/dialogue/policy.rs");
    let body = policy
        .split("fn is_person_memory")
        .nth(1)
        .expect("is_person_memory")
        .split("enum Offer")
        .next()
        .expect("is_person_memory body");
    assert!(body.contains("strip_prefix(\"learned:\")"), "{body}");
    assert!(body.contains("strip_prefix(\"memory:\")"), "{body}");
    assert!(body.contains("!= \"callback\""), "{body}");
    let dir = Scratch::new("learned-recall");
    let mut brain = learning(&dir);
    pin_bowl(&mut brain);
    let recalled = brain.turn("do you remember the blue bowl");
    assert_eq!(recalled.reply, PIN);
    assert_eq!(recalled.arm, "ask");
    assert!(recalled
        .sources
        .iter()
        .any(|source| source.starts_with("memory:") && source != "memory:callback"));
    let card = brain.turn("do you remember where the kettle is");
    assert_eq!(card.reply, UNKNOWN_REPLY);
    assert!(card.evidence.is_empty());
    assert!(card
        .sources
        .iter()
        .all(|source| !source.starts_with("card:")));
}

#[test]
fn forget_scan_does_not_allocate() {
    let src = include_str!("../src/dialogue/intent.rs");
    let head = src.split("#[cfg(test)]").next().expect("module head");
    for needle in [
        "String",
        "Vec",
        "format!",
        "to_string",
        "to_owned",
        "Box::",
        "collect",
    ] {
        assert!(
            !head.contains(needle),
            "the forget scan must not allocate via {needle}"
        );
    }
    assert!(head.contains("const FORGET"));
    assert!(head.contains("fn contains_phrase"));
    assert!(head.contains("fn negated_before"));
    let read = head
        .split("fn read")
        .nth(1)
        .expect("read")
        .split("fn from_label")
        .next()
        .expect("read body");
    let correct = read.find("CORRECT").expect("correct check");
    let forget = read.find("contains_forget").expect("forget check");
    let recall = read.find("RECALL").expect("recall check");
    assert!(correct < forget && forget < recall, "{read}");
}
