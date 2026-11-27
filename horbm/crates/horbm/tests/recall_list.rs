// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! "what do you remember about me" lists the person, newest first.
//!
//! The topic left after the recall cue is "about me". The list is capped by
//! the persona sentence budget. No rows speak the nothing-remembered line.
//! Remember denied stays the honest miss.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use horbm::{
    Brain, CardLookup, EpisodicStore, PersonMemory, Persona, PersonaLine, WriteOutcome,
    UNKNOWN_REPLY,
};

const NOTHING: &str = "I don't have anything saved about you yet.";
const SISTER: &str = "my sister is Ana";
const OAR: &str = "the spare oar is in the locker";
const BOWL: &str = "the blue bowl is mine";

fn northwind(max_sentences: u8, lines: &str) -> String {
    format!(
        r#"
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
max_sentences = {max_sentences}

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
{lines}"#
    )
}

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "horbm-recall-list-{name}-{}-{nanos}",
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
    fn lookup(&self, _query: &str) -> Option<horbm::CardHit> {
        None
    }
}

fn persona(max_sentences: u8, lines: &str) -> Persona {
    let persona = Persona::parse(&northwind(max_sentences, lines)).expect("northwind persona");
    assert_eq!(persona.id(), "northwind");
    assert_eq!(persona.style().max_sentences, max_sentences);
    persona
}

fn lined(max_sentences: u8) -> Persona {
    let persona = persona(
        max_sentences,
        "[lines]\nnothing_remembered = \"I don't have anything saved about you yet.\"\n",
    );
    assert_eq!(persona.line(PersonaLine::NothingRemembered), Some(NOTHING));
    persona
}

fn bare() -> Persona {
    let persona = persona(2, "");
    assert_eq!(persona.line(PersonaLine::NothingRemembered), None);
    persona
}

fn granted(dir: &Scratch, persona: Persona) -> Brain {
    let mut brain = Brain::with_persona(Empty, None, persona).with_memory(EpisodicStore::open(
        dir.path().join("person-memory.sqlite"),
        true,
    ));
    assert_eq!(brain.persona().id(), "northwind");
    brain.set_person("p1");
    brain
}

fn remember(brain: &mut Brain, text: &str) {
    let pinned = brain.turn(&format!("remember {text}"));
    assert_eq!(pinned.reply, UNKNOWN_REPLY, "{text}");
    assert!(pinned
        .sources
        .iter()
        .all(|source| !source.starts_with("memory:")));
}

#[test]
fn two_pins_are_spoken_newest_first() {
    let dir = Scratch::new("two");
    let mut brain = granted(&dir, lined(2));
    remember(&mut brain, OAR);
    remember(&mut brain, SISTER);
    let out = brain.turn("what do you remember about me");
    assert_eq!(out.reply, format!("{SISTER}. {OAR}."));
    assert!(out.reply.find(SISTER).unwrap() < out.reply.find(OAR).unwrap());
    assert!(!out.reply.to_ascii_lowercase().contains("feeling"));
    assert_eq!(out.arm, "ask");
    assert_eq!(out.sources.len(), 2, "{:?}", out.sources);
    assert!(out.sources.iter().all(|source| {
        source.starts_with("memory:") && source != "memory:callback" && source != "memory:"
    }));
    assert_eq!(out.evidence, out.sources);
    assert!(out.reply.matches('.').count() <= 2);
}

#[test]
fn the_sentence_cap_keeps_the_newest_pins() {
    let dir = Scratch::new("cap");
    let mut brain = granted(&dir, lined(2));
    remember(&mut brain, OAR);
    remember(&mut brain, SISTER);
    remember(&mut brain, BOWL);
    let out = brain.turn("what do you remember about me");
    assert_eq!(out.reply, format!("{BOWL}. {SISTER}."));
    assert!(!out.reply.contains("spare oar"));
    assert!(!out.reply.to_ascii_lowercase().contains("feeling"));
    assert_eq!(out.sources.len(), 2, "{:?}", out.sources);
    assert!(out
        .sources
        .iter()
        .all(|source| source.starts_with("memory:")));
    assert!(out.reply.matches('.').count() <= 2);
}

#[test]
fn no_rows_speak_the_nothing_remembered_line() {
    let dir = Scratch::new("none");
    let mut brain = granted(&dir, lined(2));
    let out = brain.turn("what do you remember about me");
    assert_eq!(out.reply, NOTHING);
    assert_ne!(out.reply, UNKNOWN_REPLY);
    assert_eq!(out.arm, "ask");
    assert_eq!(out.sources, vec!["honest:unknown".to_string()]);
    assert!(out.evidence.is_empty());
    assert!(!out.reply.to_ascii_lowercase().contains("feeling"));
}

#[test]
fn an_unset_nothing_line_is_the_unknown_reply() {
    let dir = Scratch::new("unset");
    let mut brain = granted(&dir, bare());
    let out = brain.turn("what do you remember about me");
    assert_eq!(out.reply, UNKNOWN_REPLY);
    assert_eq!(out.reply, "I don't know.");
    assert_eq!(out.arm, "ask");
    assert_eq!(out.sources, vec!["honest:unknown".to_string()]);
    assert!(out.evidence.is_empty());
}

#[test]
fn denied_remember_is_the_honest_miss() {
    let dir = Scratch::new("denied");
    let path = dir.path().join("person-memory.sqlite");
    {
        let mut store = EpisodicStore::open(&path, true);
        let pinned = store.pin("p1", SISTER, "t000001");
        assert!(matches!(pinned, WriteOutcome::Stored { .. }), "{pinned:?}");
    }
    let mut brain =
        Brain::with_persona(Empty, None, lined(2)).with_memory(EpisodicStore::open(&path, false));
    assert_eq!(brain.persona().id(), "northwind");
    brain.set_person("p1");
    let out = brain.turn("what do you remember about me");
    assert_eq!(out.reply, UNKNOWN_REPLY);
    assert_eq!(out.reply, "I don't know.");
    assert_ne!(out.reply, NOTHING);
    assert!(!out.reply.contains("Ana"));
    assert_eq!(out.arm, "ask");
    assert_eq!(out.sources, vec!["honest:unknown".to_string()]);
    assert!(out.evidence.is_empty());
    assert!(path.is_file());
}
