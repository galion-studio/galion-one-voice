// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Honesty and consent, locked through `horbm`'s public API.
//!
//! A miss is [`horbm::UNKNOWN_REPLY`] and cites no card. A card hit speaks
//! that card's clause and its `card:` source. Person memory is spoken only
//! for a question about the person. Denied `remember` creates no file.
//! [`horbm::PersonMemory::forget`] hides the row. A turn never writes the
//! learn pack. [`horbm::Brain::boot`] with an explicit file is that persona.
//!
//! Fixture cards and the persona file are written under the process temp dir
//! and removed when the test ends. Nothing here is committed. These tests do
//! not change the process environment.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use horbm::{
    Brain, ClauseSource, EpisodicStore, KnowledgeCards, LearnConfig, PersonMemory, PersonState,
    Persona, SessionSeal, TrackedName, WriteOutcome, LEARN_PACK_DB, LEARN_PACK_LIVE,
    LEARN_PACK_NEXT, LEARN_PACK_RETIRING, UNKNOWN_REPLY,
};

const PERSON: &str = "person-alpha";
const CARD_ID: &str = "fixture-kettle";
const CARD_CLAUSE: &str = "The kettle is on the hob.";
const COLOUR_LINE: &str = "my favourite colour is blue";
const WORLD_LINE: &str = "the capital of france is lyon";

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
            "horbm-eval-brain-lock-{name}-{}-{nanos}",
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

fn northwind() -> Persona {
    Persona::parse(NORTHWIND).expect("northwind persona")
}

fn brain_with(cards: KnowledgeCards) -> Brain {
    let brain = Brain::with_persona(cards, None, northwind());
    assert_eq!(brain.persona().id(), "northwind");
    brain
}

fn kettle_card(id: &str, summary: &str, body: &str) -> String {
    format!(
        r#"{{
  "id": "{id}",
  "title": "{summary}",
  "section": "horbm",
  "category": "fixture",
  "summary": "{summary}",
  "body": "{body}",
  "claim_type": "fact",
  "sources": [{{
    "title": "Galion knowledge fixture",
    "url": "https://github.com/galion-studio/galion-knowledge",
    "author": "Galion Labs",
    "date": "2026-11-18",
    "accessed": "2026-11-22"
  }}],
  "evidence_strength": "strong",
  "said_by": "Galion Labs",
  "as_of_date": "2026-11-18",
  "status": "draft",
  "contested_views": [],
  "related": []
}}
"#
    )
}

/// One kettle card. It does not mention the swallow question.
fn write_kettle(dir: &Path) {
    fs::create_dir_all(dir.join("horbm")).expect("card dir");
    fs::write(
        dir.join("horbm").join(format!("{CARD_ID}.json")),
        kettle_card(CARD_ID, CARD_CLAUSE, CARD_CLAUSE),
    )
    .expect("card");
}

fn load_kettle(dir: &Path) -> KnowledgeCards {
    write_kettle(dir);
    let loaded = KnowledgeCards::load(dir).expect("load cards");
    assert_eq!(loaded.cards.len(), 1);
    assert_eq!(loaded.cards.ids().collect::<Vec<_>>(), vec![CARD_ID]);
    loaded.cards
}

fn no_card_source(sources: &[String]) -> bool {
    sources.iter().all(|source| !source.starts_with("card:"))
}

fn sqlite_names(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let Ok(rd) = fs::read_dir(dir) else {
        return out;
    };
    for entry in rd.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.contains("sqlite") || name.ends_with(".db") {
            out.push(name);
        }
    }
    out.sort();
    out
}

fn stored_id(outcome: WriteOutcome) -> String {
    match outcome {
        WriteOutcome::Stored { id } => id,
        other => panic!("pin was not stored: {other:?}"),
    }
}

fn is_pack_entry(name: &std::ffi::OsStr) -> bool {
    name == LEARN_PACK_DB
        || name == LEARN_PACK_LIVE
        || name == LEARN_PACK_NEXT
        || name == LEARN_PACK_RETIRING
}

/// `LEARN_PACK_*` files and directories anywhere under `dir`.
fn pack_entries(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(rd) = fs::read_dir(dir) else {
            return;
        };
        for ent in rd.flatten() {
            let path = ent.path();
            if is_pack_entry(&ent.file_name()) {
                out.push(path.clone());
            }
            if path.is_dir() {
                walk(&path, out);
            }
        }
    }
    walk(dir, &mut out);
    out.sort();
    out
}

fn assert_no_pack(root: &Path, pack: &Path, when: &str) {
    assert!(!pack.exists(), "{when} created {}", pack.display());
    let found = pack_entries(root);
    assert!(found.is_empty(), "{when} left {found:?}");
}

fn clause_source(turn_id: u64) -> ClauseSource {
    ClauseSource {
        session: "sess-7".to_string(),
        turn_id,
        time: "2026-11-18T21:00:00Z".to_string(),
        consent_record: "learn-v1".to_string(),
    }
}

#[test]
fn a_miss_is_exactly_the_unknown_line() {
    let root = Scratch::new("miss");
    let cards = load_kettle(&root.path().join("knowledge"));
    let mut brain = brain_with(cards);
    assert_eq!(brain.cards(), 1);
    let out = brain.turn("what is the airspeed of a fixture swallow");
    assert_eq!(out.reply, UNKNOWN_REPLY);
    assert!(no_card_source(&out.sources), "{:?}", out.sources);
    assert_eq!(out.sources, vec!["honest:unknown".to_string()]);
}

#[test]
fn a_card_hit_speaks_the_card_clause_with_its_source() {
    let root = Scratch::new("hit");
    let cards = load_kettle(&root.path().join("knowledge"));
    let mut brain = brain_with(cards);
    let out = brain.turn("where is the kettle");
    assert_eq!(out.reply, CARD_CLAUSE);
    assert_eq!(out.sources, vec![format!("card:{CARD_ID}")]);
}

#[test]
fn person_memory_speaks_only_for_a_question_about_the_person() {
    let root = Scratch::new("person");
    let path = root.path().join("person-memory.sqlite");
    let mut brain =
        brain_with(KnowledgeCards::empty()).with_memory(EpisodicStore::open(&path, true));
    brain.set_person(PERSON);

    let pinned = brain.turn(&format!("remember that {COLOUR_LINE}"));
    assert_eq!(pinned.reply, UNKNOWN_REPLY);
    brain.turn(&format!("remember that {WORLD_LINE}"));
    {
        let stored = brain
            .person_memory()
            .expect("diary")
            .recall(PERSON, "what is the capital of france");
        assert!(
            stored.iter().any(|hit| hit.text == WORLD_LINE),
            "the pin is in the diary so the guard is what blocks the answer: {stored:?}"
        );
    }

    let world = brain.turn("what is the capital of france");
    assert_ne!(world.reply, WORLD_LINE);
    assert!(!world.reply.to_lowercase().contains("lyon"));
    assert!(
        world
            .sources
            .iter()
            .all(|source| !source.starts_with("memory:")),
        "{:?}",
        world.sources
    );
    assert_eq!(world.reply, UNKNOWN_REPLY);

    let about = brain.turn("what is my favourite colour");
    assert_eq!(about.reply, COLOUR_LINE);
    assert!(
        about
            .sources
            .iter()
            .any(|source| source.starts_with("memory:")),
        "{:?}",
        about.sources
    );
}

#[test]
fn denied_memory_creates_no_file() {
    let root = Scratch::new("deny");
    let path = root.path().join("person-memory.sqlite");
    let mut store = EpisodicStore::open(&path, false);
    assert!(!store.remember_granted());
    let state = PersonState {
        names: vec![TrackedName {
            name: "Nia".into(),
            class_label: "they".into(),
        }],
        arousal: 0.3,
        valence: 0.25,
        dominance: 0.5,
        feeling: None,
    };
    assert_eq!(
        store.pin(PERSON, COLOUR_LINE, "t000001"),
        WriteOutcome::Refused
    );
    assert_eq!(
        store.note_state(PERSON, &state, "t000001"),
        WriteOutcome::Refused
    );
    assert_eq!(
        store.note_turn(PERSON, PERSON, COLOUR_LINE, "t000001"),
        WriteOutcome::Refused
    );
    assert_eq!(store.end_session(PERSON, "t000001"), WriteOutcome::Refused);
    assert_eq!(store.forget(PERSON, "ep-1"), WriteOutcome::Refused);
    assert!(store
        .recall(PERSON, "what is my favourite colour")
        .is_empty());
    assert!(!path.exists());

    let flagged = root.path().join("flagged.sqlite");
    let mut store = EpisodicStore::open_from_flag(&flagged, None);
    assert!(!store.remember_granted());
    assert_eq!(
        store.pin(PERSON, COLOUR_LINE, "t000001"),
        WriteOutcome::Refused
    );
    assert!(!flagged.exists());

    let brain_path = root.path().join("brain.sqlite");
    let mut brain =
        brain_with(KnowledgeCards::empty()).with_memory(EpisodicStore::open(&brain_path, false));
    brain.set_person(PERSON);
    let out = brain.turn(&format!("remember that {COLOUR_LINE}"));
    assert_eq!(out.reply, UNKNOWN_REPLY);
    assert!(
        out.sources
            .iter()
            .all(|source| !source.starts_with("memory:")),
        "{:?}",
        out.sources
    );
    assert!(!brain_path.exists());
    let names = sqlite_names(root.path());
    assert!(names.is_empty(), "{names:?}");
}

#[test]
fn forget_hides_the_row() {
    let root = Scratch::new("forget");
    let path = root.path().join("person-memory.sqlite");
    let mut store = EpisodicStore::open(&path, true);
    let id = stored_id(store.pin(PERSON, COLOUR_LINE, "t000001"));
    let before = store.recall(PERSON, "what is my favourite colour");
    assert!(
        before
            .iter()
            .any(|hit| hit.id == id && hit.text == COLOUR_LINE),
        "{before:?}"
    );

    let forgotten = PersonMemory::forget(&mut store, PERSON, &id);
    assert_eq!(forgotten, WriteOutcome::Archived { id: id.clone() });
    let hidden = store.recall(PERSON, "what is my favourite colour");
    assert!(
        hidden.is_empty(),
        "forget hid the row, recall still returned {hidden:?}"
    );
    let stone = store
        .tombstone(&id)
        .expect("hidden row stays as a tombstone");
    assert_eq!(stone.text, COLOUR_LINE);
    assert_eq!(stone.reason, "forget");
    assert_eq!(stone.person_id, PERSON);
    assert!(
        path.is_file(),
        "forget hides the row and keeps the diary file"
    );

    let mut brain = brain_with(KnowledgeCards::empty()).with_memory(store);
    brain.set_person(PERSON);
    let out = brain.turn("what is my favourite colour");
    assert_eq!(out.reply, UNKNOWN_REPLY);
    assert!(!out.reply.contains("blue"));
    assert!(
        out.sources
            .iter()
            .all(|source| !source.starts_with("memory:")),
        "{:?}",
        out.sources
    );
}

#[test]
fn a_turn_never_writes_the_learn_pack() {
    let root = Scratch::new("learn");
    let pack = root.path().join("pack");
    let pack_flag = pack.to_str().expect("utf-8 pack path").to_string();
    let config = LearnConfig::from_flags(None, Some(&pack_flag));
    assert!(!config.granted, "unset learn is fresh");
    assert_eq!(config.pack_dir, pack);

    let cards = load_kettle(&root.path().join("knowledge"));
    let mut brain = brain_with(cards);
    let mut seal = SessionSeal::open(config);
    seal.note("user", CARD_CLAUSE, &clause_source(1));
    assert_no_pack(root.path(), &pack, "note");

    for i in 0..4 {
        let kettle = i % 2 == 0;
        let out = brain.turn(if kettle {
            "where is the kettle"
        } else {
            "I am so tired"
        });
        if kettle {
            assert_eq!(out.reply, CARD_CLAUSE, "turn {i}");
            assert_eq!(out.sources, vec![format!("card:{CARD_ID}")], "turn {i}");
        } else {
            assert_eq!(out.reply, UNKNOWN_REPLY, "turn {i}");
            assert!(no_card_source(&out.sources), "turn {i} {:?}", out.sources);
        }
        assert_no_pack(root.path(), &pack, &format!("turn {i}"));
    }

    let err = seal.seal().expect_err("unset learn");
    assert!(err.is_denied(), "{err}");
    assert_no_pack(root.path(), &pack, "unset seal");
}

#[test]
fn boot_is_the_configured_persona() {
    let root = Scratch::new("boot");
    let persona_path = root.path().join("northwind.toml");
    fs::write(&persona_path, NORTHWIND).expect("persona file");
    let memory_db = root.path().join("person.sqlite");
    let knowledge = root.path().join("knowledge");
    let cases = root.path().join("cases");
    fs::create_dir_all(&knowledge).expect("knowledge");
    fs::create_dir_all(&cases).expect("cases");

    let brain = Brain::boot(&memory_db, &knowledge, &cases, Some(&persona_path)).expect("boot");
    assert_eq!(brain.persona().id(), "northwind");
}
