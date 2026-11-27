// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! A correction after a learned answer queues the same retraction.
//!
//! The reply stays the honest ask. A card answer queues nothing.
//! `reload_knowledge` reads a card file added after the thread started.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use horbm::{
    Brain, CardLookup, ClauseSource, LearnConfig, LearnPass, LearnedRow, Persona, RetrievalConfig,
    RetrievalThread, UNKNOWN_REPLY,
};

const TAUGHT: &str = "You remember the spare oar is in the locker.";
const CARD_CLAUSE: &str = "The kettle is on the hob.";
const LANTERN: &str = "The lantern is in the shed.";

const CARD_LINE: &str = r#"{"id":"kettle","title":"The kettle is on the hob.","section":"horbm","category":"fixture","summary":"The kettle is on the hob.","body":"The kettle is on the hob.","claim_type":"fact","sources":[],"evidence_strength":"strong","said_by":"northwind","as_of_date":"2026-11-25","status":"draft","contested_views":[],"related":[]}"#;

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

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "horbm-correct-turn-{name}-{}-{nanos}",
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

fn northwind() -> Persona {
    let persona = Persona::parse(NORTHWIND).expect("northwind persona");
    assert_eq!(persona.id(), "northwind");
    persona
}

fn write_card(cards: &Path) {
    fs::create_dir_all(cards).expect("cards");
    fs::write(cards.join("kettle.json"), CARD_LINE).expect("card");
}

fn card_line(id: &str, sentence: &str) -> String {
    format!(
        r#"{{"id":"{id}","title":"{sentence}","section":"horbm","category":"fixture","summary":"{sentence}","body":"{sentence}","claim_type":"fact","sources":[],"evidence_strength":"strong","said_by":"northwind","as_of_date":"2026-11-25","status":"draft","contested_views":[],"related":[]}}"#
    )
}

fn teach(pack: &Path, text: &str) -> Vec<LearnedRow> {
    let learn = LearnPass::open(LearnConfig::granted(pack));
    let source = ClauseSource {
        session: "sess-learn".to_string(),
        turn_id: 4,
        time: "2026-11-26T00:00:00Z".to_string(),
        consent_record: "learn-v1".to_string(),
    };
    learn
        .write_clause("user", text, &source)
        .expect("write clause");
    learn.audit().expect("audit").rows
}

fn granted_brain(cards: &Path) -> Brain {
    let thread = RetrievalThread::spawn(RetrievalConfig {
        remember: true,
        knowledge_dir: Some(cards.to_path_buf()),
        fixture_rows: 0,
        keys_path: None,
    })
    .expect("spawn");
    let brain = Brain::with_persona(Empty, None, northwind()).with_retrieval(thread);
    assert_eq!(brain.persona().id(), "northwind");
    brain
}

#[test]
fn a_correction_after_a_learned_answer_queues_the_clause() {
    let root = Scratch::new("learned");
    let cards = root.path().join("cards");
    let pack = root.path().join("pack");
    write_card(&cards);
    let rows = teach(&pack, TAUGHT);

    let mut brain = granted_brain(&cards);
    brain.allow_learn_retract(true);
    brain.attach_learned(rows).expect("attach");
    let spoken = brain.turn("spare oar");
    assert_eq!(spoken.reply, TAUGHT);
    assert!(
        spoken.sources[0].starts_with("learned:diary-"),
        "{:?}",
        spoken.sources
    );

    let wrong = brain.turn("no, that's wrong");
    assert_eq!(wrong.reply, UNKNOWN_REPLY);
    assert_eq!(wrong.reply, "I don't know.");
    assert_eq!(wrong.arm, "ask");
    assert_eq!(wrong.sources, vec!["honest:unknown".to_string()]);
    assert!(wrong.evidence.is_empty());
    assert!(!wrong.reply.contains("oar"));
    assert_ne!(wrong.reply, TAUGHT);
    let taken = brain.take_retraction().expect("one retraction");
    assert_eq!(taken.said_by, "user");
    assert_eq!(taken.text, TAUGHT);
    assert!(brain.take_retraction().is_none());
    let audit = LearnPass::open(LearnConfig::granted(&pack))
        .audit()
        .expect("audit");
    assert_eq!(audit.rows.len(), 1);
    assert_eq!(audit.rows[0].text, TAUGHT);
}

#[test]
fn a_correction_after_a_card_queues_nothing() {
    let root = Scratch::new("card");
    let cards = root.path().join("cards");
    write_card(&cards);

    let mut brain = granted_brain(&cards);
    brain.allow_learn_retract(true);
    let card = brain.turn("kettle");
    assert_eq!(card.reply, CARD_CLAUSE);
    assert_eq!(card.sources, vec!["retrieve:kettle".to_string()]);

    let wrong = brain.turn("no, that's wrong");
    assert_eq!(wrong.reply, UNKNOWN_REPLY);
    assert_eq!(wrong.sources, vec!["honest:unknown".to_string()]);
    assert!(wrong.evidence.is_empty());
    assert!(!wrong.reply.to_ascii_lowercase().contains("kettle"));
    assert!(!wrong.reply.to_ascii_lowercase().contains("hob"));
    assert!(brain.take_retraction().is_none());
}

#[test]
fn reload_knowledge_picks_up_a_card_added_on_disk() {
    let root = Scratch::new("reload");
    let cards = root.path().join("cards");
    fs::create_dir_all(&cards).expect("cards");
    let mut brain = granted_brain(&cards);
    let missed = brain.turn("lantern");
    assert_eq!(missed.reply, UNKNOWN_REPLY);

    fs::write(cards.join("lantern.json"), card_line("lantern", LANTERN)).expect("card");
    brain.reload_knowledge().expect("reload");
    let hit = brain.turn("lantern");
    assert_eq!(hit.reply, LANTERN);
    assert_eq!(hit.sources, vec!["retrieve:lantern".to_string()]);
}

#[test]
fn reload_without_a_thread_is_ok() {
    let mut brain = Brain::with_persona(Empty, None, northwind());
    assert!(brain.retrieval_cell().is_none());
    brain.reload_knowledge().expect("no thread");
}
