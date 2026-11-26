// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! A learned row spoken on the turn.
//!
//! The card is one line written into a temp directory. Nothing here is a
//! committed card. `LearnPass` writes the taught clause. `Brain::attach_learned`
//! hands those rows to the retrieval thread. The turn speaks a learned hit
//! as `learned:<diary id>`.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use horbm::{
    Brain, CardLookup, ClauseSource, LearnConfig, LearnPass, LearnedRow, Persona, RetrievalConfig,
    RetrievalThread, UNKNOWN_REPLY,
};

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

/// One generated card. The learned clauses below do not all share its words.
const CARD_LINE: &str = r#"{"id":"kettle","title":"The kettle is on the hob.","section":"horbm","category":"fixture","summary":"The kettle is on the hob.","body":"The kettle is on the hob.","claim_type":"fact","sources":[],"evidence_strength":"strong","said_by":"northwind","as_of_date":"2026-11-25","status":"draft","contested_views":[],"related":[]}"#;

/// Content words of a recall ask ("you", "remember", "spare", "oar") sit in
/// this clause, so the same row covers both "spare oar" and
/// "do you remember the spare oar".
const TAUGHT: &str = "You remember the spare oar is in the locker.";

const CARD_CLAUSE: &str = "The kettle is on the hob.";

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "horbm-learned-turn-{name}-{}-{nanos}",
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
    Persona::parse(NORTHWIND).expect("northwind persona")
}

fn write_card(cards: &Path) {
    fs::create_dir_all(cards).expect("cards");
    fs::write(cards.join("kettle.json"), CARD_LINE).expect("card");
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
    let audit = learn.audit().expect("audit");
    assert_eq!(audit.rows.len(), 1, "{:?}", audit.rows);
    assert_eq!(audit.rows[0].text, text);
    audit.rows
}

fn granted_brain(cards: &Path) -> Brain {
    let thread = RetrievalThread::spawn(RetrievalConfig {
        remember: true,
        knowledge_dir: Some(cards.to_path_buf()),
        fixture_rows: 0,
        keys_path: None,
    })
    .expect("spawn");
    Brain::with_persona(Empty, None, northwind()).with_retrieval(thread)
}

#[test]
fn taught_clause_is_spoken_as_learned() {
    let root = Scratch::new("speak");
    let cards = root.path().join("cards");
    let pack = root.path().join("pack");
    write_card(&cards);
    let rows = teach(&pack, TAUGHT);
    let label = rows[0].source.clone();

    let mut brain = granted_brain(&cards);
    brain.attach_learned(rows).expect("attach");
    let out = brain.turn("spare oar");
    let cell = brain.retrieval_cell().expect("cell");
    let hit = cell.hits.first().expect("a learned hit");
    assert!(hit.source.starts_with("learned;"), "{hit:?}");
    assert!(hit.item_id.starts_with("diary-"), "{}", hit.item_id);
    assert_eq!(hit.source, format!("learned;{label}"));
    assert_eq!(out.reply, TAUGHT);
    assert_eq!(out.sources, vec![format!("learned:{}", hit.item_id)]);
    assert!(out.sources[0].starts_with("learned:diary-"));
    assert_eq!(
        out.evidence,
        vec![format!("learned:{}", hit.item_id), hit.source.clone()]
    );
}

#[test]
fn recall_intent_keeps_a_learned_line() {
    let root = Scratch::new("recall");
    let cards = root.path().join("cards");
    let pack = root.path().join("pack");
    write_card(&cards);
    let rows = teach(&pack, TAUGHT);

    let mut brain = granted_brain(&cards);
    brain.attach_learned(rows).expect("attach");
    let out = brain.turn("do you remember the spare oar");
    assert_ne!(out.reply, UNKNOWN_REPLY);
    assert_eq!(out.reply, TAUGHT);
    assert_eq!(out.sources.len(), 1, "{:?}", out.sources);
    assert!(
        out.sources[0].starts_with("learned:diary-"),
        "{:?}",
        out.sources
    );
    assert_eq!(
        out.evidence.first().map(String::as_str),
        Some(out.sources[0].as_str())
    );
}

#[test]
fn no_rows_attached_is_unknown() {
    let root = Scratch::new("none");
    let cards = root.path().join("cards");
    write_card(&cards);
    let mut brain = granted_brain(&cards);
    let out = brain.turn("spare oar");
    assert_eq!(out.reply, UNKNOWN_REPLY);
    assert!(out
        .sources
        .iter()
        .all(|source| !source.starts_with("learned:")));
}

#[test]
fn covering_card_wins_over_learned_row() {
    let root = Scratch::new("cover");
    let cards = root.path().join("cards");
    let pack = root.path().join("pack");
    write_card(&cards);
    let rows = teach(&pack, "Someone left the kettle on the hob.");

    let mut brain = granted_brain(&cards);
    brain.attach_learned(rows).expect("attach");
    let out = brain.turn("kettle");
    let cell = brain.retrieval_cell().expect("cell");
    assert!(!cell.hits.is_empty(), "{cell:?}");
    assert_eq!(cell.hits[0].item_id, "kettle", "{:?}", cell.hits);
    assert!(!cell.hits[0].source.starts_with("learned;"));
    assert_eq!(out.reply, CARD_CLAUSE);
    assert_eq!(out.sources, vec!["retrieve:kettle".to_string()]);
    assert_eq!(out.evidence, vec!["retrieve:kettle".to_string()]);
    assert!(out
        .sources
        .iter()
        .all(|source| !source.starts_with("learned:")));
}

#[test]
fn empty_attach_clears() {
    let root = Scratch::new("clear");
    let cards = root.path().join("cards");
    let pack = root.path().join("pack");
    write_card(&cards);
    let rows = teach(&pack, TAUGHT);

    let mut brain = granted_brain(&cards);
    brain.attach_learned(rows).expect("attach");
    let spoken = brain.turn("spare oar");
    assert_eq!(spoken.reply, TAUGHT);
    assert!(spoken.sources[0].starts_with("learned:diary-"));

    brain.attach_learned(Vec::new()).expect("clear");
    let cleared = brain.turn("spare oar");
    assert_eq!(cleared.reply, UNKNOWN_REPLY);
    assert!(cleared
        .sources
        .iter()
        .all(|source| !source.starts_with("learned:")));
}

#[test]
fn attach_without_thread_is_ok_and_silent() {
    let root = Scratch::new("silent");
    let pack = root.path().join("pack");
    let rows = teach(&pack, TAUGHT);
    let mut brain = Brain::with_persona(Empty, None, northwind());
    assert!(brain.retrieval_cell().is_none());
    brain.attach_learned(rows).expect("no thread");
    let out = brain.turn("spare oar");
    assert_eq!(out.reply, UNKNOWN_REPLY);
    assert_ne!(out.reply, TAUGHT);
    assert!(out
        .sources
        .iter()
        .all(|source| !source.starts_with("learned:")));
    assert!(out.evidence.iter().all(|item| !item.starts_with("learned")));
}
