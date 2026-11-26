// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! A forget turn archives the diary row and does not store the request.
//!
//! The fixture id is `northwind`. The forgot line comes from that file.
//! Remember denied opens no diary.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use horbm::{
    Brain, CardHit, CardLookup, DialogueConsent, EpisodicStore, Persona, PersonaLine, UNKNOWN_REPLY,
};

const FORGOT: &str = "Okay, I've let that go.";
const PIN: &str = "my sister is Ana";

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

[lines]
forgot = "Okay, I've let that go."
"#;

const NORTHWIND_BARE: &str = r#"
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

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "horbm-forget-turn-{name}-{}-{nanos}",
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

struct Empty;

impl CardLookup for Empty {
    fn lookup(&self, _query: &str) -> Option<CardHit> {
        None
    }
}

fn northwind() -> Persona {
    let persona = Persona::parse(NORTHWIND).expect("northwind persona");
    assert_eq!(persona.id(), "northwind");
    assert_eq!(persona.line(PersonaLine::Forgot), Some(FORGOT));
    persona
}

fn bare() -> Persona {
    let persona = Persona::parse(NORTHWIND_BARE).expect("northwind persona");
    assert_eq!(persona.id(), "northwind");
    assert_eq!(persona.line(PersonaLine::Forgot), None);
    persona
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

fn granted(dir: &Scratch, persona: Persona) -> Brain {
    let mut brain = Brain::with_persona(Empty, None, persona).with_memory(EpisodicStore::open(
        dir.path().join("person-memory.sqlite"),
        true,
    ));
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

fn pin_sister(brain: &mut Brain) {
    let pinned = brain.turn("remember my sister is Ana");
    assert_eq!(pinned.reply, UNKNOWN_REPLY);
    assert!(pinned
        .sources
        .iter()
        .all(|source| !source.starts_with("memory:")));
}

#[test]
fn please_forget_my_sister_speaks_the_forgot_line_and_is_not_stored() {
    let dir = Scratch::new("sister");
    let path = dir.path().join("person-memory.sqlite");
    let mut brain = granted(&dir, northwind());
    pin_sister(&mut brain);
    let updates = brain.bandit_updates();
    let asks = brain.bandit_counts()[horbm::Move::Ask.index()];
    let forgotten = brain.turn("please forget my sister");
    assert_eq!(forgotten.reply, FORGOT);
    assert_eq!(forgotten.arm, "ask");
    assert_eq!(forgotten.propensity, 0.4);
    assert_eq!(forgotten.sources, vec!["forget:1".to_string()]);
    assert!(forgotten.evidence.is_empty());
    assert!(!forgotten.reply.contains("Ana"));
    assert!(!forgotten.reply.to_ascii_lowercase().contains("sister"));
    assert!(forgotten
        .sources
        .iter()
        .all(|source| !source.contains("Ana") && !source.starts_with("memory:")));
    assert_eq!(brain.bandit_updates(), updates);
    assert_eq!(brain.bandit_counts()[horbm::Move::Ask.index()], asks);
    let recalled = brain.turn("do you remember my sister");
    assert_eq!(recalled.reply, UNKNOWN_REPLY);
    assert!(recalled
        .sources
        .iter()
        .all(|source| !source.starts_with("memory:")));
    let person = brain.person_id().to_string();
    brain
        .person_memory()
        .expect("diary")
        .end_session(&person, "t000099");
    drop(brain);
    let saved = EpisodicStore::read_transcript_file(&path, "sister").expect("reopen");
    assert!(
        saved
            .turns
            .iter()
            .any(|turn| turn.text.contains("my sister is Ana")),
        "{:?}",
        saved.turns
    );
    assert!(
        saved
            .turns
            .iter()
            .all(|turn| !turn.text.to_ascii_lowercase().contains("please forget")),
        "{:?}",
        saved.turns
    );
    let asked = EpisodicStore::read_transcript_file(&path, "please forget").expect("reopen");
    assert!(
        asked.turns.is_empty(),
        "the forget request was stored: {:?}",
        asked.turns
    );
}

#[test]
fn forget_that_archives_the_memory_snap() {
    let dir = Scratch::new("that");
    let mut brain = granted(&dir, northwind());
    pin_sister(&mut brain);
    let recalled = brain.turn("do you remember my sister");
    assert_eq!(recalled.reply, PIN);
    assert_eq!(recalled.arm, "ask");
    let snap = brain.ring_get(brain.ring_len() - 1).expect("recall snap");
    assert!(snap
        .source
        .as_deref()
        .is_some_and(|source| { source.starts_with("memory:") && source != "memory:callback" }));
    let updates = brain.bandit_updates();
    let asks = brain.bandit_counts()[horbm::Move::Ask.index()];
    let forgotten = brain.turn("forget that");
    assert_eq!(forgotten.reply, FORGOT);
    assert_eq!(forgotten.arm, "ask");
    assert_eq!(forgotten.propensity, 0.4);
    assert_eq!(forgotten.sources, vec!["forget:1".to_string()]);
    assert!(forgotten.evidence.is_empty());
    assert!(!forgotten.reply.contains("Ana"));
    assert_eq!(brain.bandit_updates(), updates);
    assert_eq!(brain.bandit_counts()[horbm::Move::Ask.index()], asks);
    let gone = brain.turn("do you remember my sister");
    assert_eq!(gone.reply, UNKNOWN_REPLY);
    assert!(gone
        .sources
        .iter()
        .all(|source| !source.starts_with("memory:")));
}

#[test]
fn unknown_topic_stays_the_honest_miss() {
    let dir = Scratch::new("miss");
    let mut brain = granted(&dir, northwind());
    pin_sister(&mut brain);
    let updates = brain.bandit_updates();
    let missed = brain.turn("please forget the violet harbour");
    assert_eq!(missed.reply, UNKNOWN_REPLY);
    assert_eq!(missed.reply, "I don't know.");
    assert_ne!(missed.reply, FORGOT);
    assert_eq!(missed.arm, "ask");
    assert_eq!(missed.sources, vec!["honest:unknown".to_string()]);
    assert!(missed.evidence.is_empty());
    assert_eq!(brain.bandit_updates(), updates);
    let still = brain.turn("do you remember my sister");
    assert_eq!(still.reply, PIN);
    assert!(still
        .sources
        .iter()
        .any(|source| source.starts_with("memory:") && source != "memory:callback"));
}

#[test]
fn denied_remember_is_the_honest_miss_and_opens_nothing() {
    let dir = Scratch::new("denied");
    let path = dir.path().join("person-memory.sqlite");
    let mut brain = Brain::with_persona(Empty, None, northwind())
        .with_memory(EpisodicStore::open(&path, false));
    assert_eq!(brain.persona().id(), "northwind");
    brain.set_person("p1");
    brain.set_bandit_enabled(true);
    brain.set_dialogue_consent(DialogueConsent {
        remember: false,
        learn: true,
        check_ins: false,
    });
    let out = brain.turn("please forget my sister");
    assert_eq!(out.reply, UNKNOWN_REPLY);
    assert_eq!(out.reply, "I don't know.");
    assert_ne!(out.reply, FORGOT);
    assert_eq!(out.arm, "ask");
    assert_eq!(out.sources, vec!["honest:unknown".to_string()]);
    assert!(out.evidence.is_empty());
    assert_eq!(brain.bandit_updates(), 0);
    assert_eq!(brain.bandit_counts()[horbm::Move::Ask.index()], 0);
    assert!(!path.exists(), "remember denied creates no diary file");
    assert!(names(dir.path()).is_empty(), "{:?}", names(dir.path()));
}

#[test]
fn please_dont_forget_that_does_not_forget() {
    let dir = Scratch::new("keep");
    let mut brain = granted(&dir, northwind());
    pin_sister(&mut brain);
    let updates = brain.bandit_updates();
    let kept = brain.turn("please don't forget that");
    assert_ne!(kept.reply, FORGOT);
    assert!(kept
        .sources
        .iter()
        .all(|source| !source.starts_with("forget:")));
    assert!(brain.bandit_updates() > updates);
    let recalled = brain.turn("do you remember my sister");
    assert_eq!(recalled.reply, PIN);
    assert!(recalled
        .sources
        .iter()
        .any(|source| source.starts_with("memory:") && source != "memory:callback"));
}

#[test]
fn unset_forgot_line_falls_back_to_unknown() {
    let dir = Scratch::new("unset");
    let mut brain = granted(&dir, bare());
    pin_sister(&mut brain);
    let forgotten = brain.turn("please forget my sister");
    assert_eq!(forgotten.reply, UNKNOWN_REPLY);
    assert_eq!(forgotten.arm, "ask");
    assert_eq!(forgotten.sources, vec!["forget:1".to_string()]);
    assert!(forgotten.evidence.is_empty());
    let gone = brain.turn("do you remember my sister");
    assert_eq!(gone.reply, UNKNOWN_REPLY);
}
