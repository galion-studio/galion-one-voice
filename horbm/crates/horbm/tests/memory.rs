// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Person memory. Consent deny writes nothing. A pin is recalled on the next
//! turn. Forget leaves a tombstone and is not recalled. A world question is
//! not answered from the diary. The same script produces the same rows.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use horbm::{
    about_the_person, CellState, EpisodicStore, MemoryItem, MemoryKind, MemorySink, PersonMemory,
    PersonState, RetrievalConfig, RetrievalThread, RetrievedCells, TrackedName, WriteOutcome,
    UNKNOWN_REPLY,
};

fn scratch(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("horbm-f7-{name}-{}-{nanos}", std::process::id()));
    fs::create_dir_all(&dir).expect("scratch");
    dir
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

struct Count {
    stored: Arc<AtomicUsize>,
    extracted: Arc<AtomicUsize>,
}

impl MemorySink for Count {
    fn on_stored(&mut self, _item: &MemoryItem) {
        self.stored.fetch_add(1, Ordering::SeqCst);
    }

    fn on_extracted(&mut self, _person_id: &str, _text: &str) {
        self.extracted.fetch_add(1, Ordering::SeqCst);
    }
}

fn counter() -> (Count, Arc<AtomicUsize>, Arc<AtomicUsize>) {
    let stored = Arc::new(AtomicUsize::new(0));
    let extracted = Arc::new(AtomicUsize::new(0));
    (
        Count {
            stored: Arc::clone(&stored),
            extracted: Arc::clone(&extracted),
        },
        stored,
        extracted,
    )
}

fn state() -> PersonState {
    PersonState {
        names: vec![TrackedName {
            name: "Nia".into(),
            class_label: "they".into(),
        }],
        arousal: 0.3,
        valence: 0.25,
        dominance: 0.5,
        feeling: None,
    }
}

struct Calls {
    n: Arc<AtomicUsize>,
}

impl PersonMemory for Calls {
    fn remember_granted(&self) -> bool {
        false
    }
    fn set_remember(&mut self, _grant: bool) {
        self.n.fetch_add(1, Ordering::SeqCst);
    }
    fn set_sink(&mut self, _sink: Box<dyn MemorySink + Send>) {
        self.n.fetch_add(1, Ordering::SeqCst);
    }
    fn pin(&mut self, _: &str, _: &str, _: &str) -> WriteOutcome {
        self.n.fetch_add(1, Ordering::SeqCst);
        WriteOutcome::Refused
    }
    fn note_state(&mut self, _: &str, _: &PersonState, _: &str) -> WriteOutcome {
        self.n.fetch_add(1, Ordering::SeqCst);
        WriteOutcome::Refused
    }
    fn note_turn(&mut self, _: &str, _: &str, _: &str, _: &str) -> WriteOutcome {
        self.n.fetch_add(1, Ordering::SeqCst);
        WriteOutcome::Refused
    }
    fn end_session(&mut self, _: &str, _: &str) -> WriteOutcome {
        self.n.fetch_add(1, Ordering::SeqCst);
        WriteOutcome::Refused
    }
    fn forget(&mut self, _: &str, _: &str) -> WriteOutcome {
        self.n.fetch_add(1, Ordering::SeqCst);
        WriteOutcome::Refused
    }
    fn recall(&mut self, _: &str, _: &str) -> Vec<MemoryItem> {
        self.n.fetch_add(1, Ordering::SeqCst);
        Vec::new()
    }
    fn tombstone(&self, _: &str) -> Option<horbm::Tombstone> {
        None
    }
}

#[test]
fn consent_deny_means_zero_writes() {
    let dir = scratch("deny");
    let path = dir.join("person-memory.sqlite");
    let (sink, stored, extracted) = counter();
    let mut store = EpisodicStore::open(&path, false);
    store.set_sink(Box::new(sink));
    assert!(!store.remember_granted());
    assert_eq!(
        store.pin("person-alpha", "my favourite colour is blue", "t000001"),
        WriteOutcome::Refused
    );
    assert_eq!(
        store.note_state("person-alpha", &state(), "t000001"),
        WriteOutcome::Refused
    );
    assert_eq!(
        store.note_turn(
            "person-alpha",
            "person-alpha",
            "I prefer quiet mornings",
            "t000001"
        ),
        WriteOutcome::Refused
    );
    assert_eq!(
        store.end_session("person-alpha", "t000001"),
        WriteOutcome::Refused
    );
    assert_eq!(store.forget("person-alpha", "ep-1"), WriteOutcome::Refused);
    assert!(store
        .recall("person-alpha", "what is my favourite colour")
        .is_empty());
    assert_eq!(store.writes(), 0);
    assert_eq!(store.extractions(), 0);
    assert_eq!(store.buffered_turns(), 0);
    assert!(!path.exists());
    assert!(sqlite_names(&dir).is_empty(), "{:?}", sqlite_names(&dir));
    assert_eq!(stored.load(Ordering::SeqCst), 0);
    assert_eq!(extracted.load(Ordering::SeqCst), 0);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_denied_brain_does_not_call_the_diary() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut brain = horbm::Brain::with_cards(EmptyCards, None).with_memory(Calls {
        n: Arc::clone(&calls),
    });
    brain.set_person("person-alpha");
    let pin = brain.turn("remember that my favourite colour is blue");
    let ask = brain.turn("what is my favourite colour");
    assert_eq!(pin.reply, UNKNOWN_REPLY);
    assert_eq!(ask.reply, UNKNOWN_REPLY);
    assert!(pin
        .sources
        .iter()
        .all(|source| !source.starts_with("memory:")));
    assert!(ask
        .sources
        .iter()
        .all(|source| !source.starts_with("memory:")));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn pin_is_recalled_on_the_next_turn() {
    let dir = scratch("pin");
    let mut brain = horbm::Brain::with_cards(EmptyCards, None)
        .with_memory(EpisodicStore::open(dir.join("person-memory.sqlite"), true));
    brain.set_person("person-alpha");
    brain.note_name("Nia", horbm::PronounClass::They);
    let pinned = brain.turn("remember that my favourite colour is blue");
    assert_eq!(pinned.reply, UNKNOWN_REPLY);
    assert!(pinned
        .sources
        .iter()
        .all(|source| !source.starts_with("memory:")));
    let recalled = brain.turn("what is my favourite colour");
    assert_eq!(recalled.reply, "my favourite colour is blue");
    assert!(recalled
        .sources
        .iter()
        .any(|source| source.starts_with("memory:")));
    assert!(recalled
        .sources
        .iter()
        .all(|source| !source.starts_with("card:")));
    let name = brain.turn("what is my name");
    assert!(name.reply.contains("Nia"), "{}", name.reply);
    assert!(name
        .sources
        .iter()
        .any(|source| source.starts_with("memory:")));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn forget_is_a_tombstone_and_is_not_recalled() {
    let dir = scratch("forget");
    let mut brain = horbm::Brain::with_cards(EmptyCards, None)
        .with_memory(EpisodicStore::open(dir.join("person-memory.sqlite"), true));
    brain.set_person("person-alpha");
    brain.turn("remember that my favourite colour is blue");
    let id = {
        let memory = brain.person_memory().expect("diary");
        let hits = memory.recall("person-alpha", "what is my favourite colour");
        assert_eq!(hits.len(), 1, "{hits:?}");
        assert_eq!(hits[0].kind, MemoryKind::Pin);
        hits[0].id.clone()
    };
    let forgotten = brain
        .person_memory()
        .expect("diary")
        .forget("person-alpha", &id);
    assert_eq!(forgotten, WriteOutcome::Archived { id: id.clone() });
    let stone = brain
        .person_memory()
        .expect("diary")
        .tombstone(&id)
        .expect("tombstone");
    assert_eq!(stone.text, "my favourite colour is blue");
    assert_eq!(stone.reason, "forget");
    assert_eq!(stone.person_id, "person-alpha");
    assert!(brain
        .person_memory()
        .expect("diary")
        .recall("person-alpha", "what is my favourite colour")
        .is_empty());
    let out = brain.turn("what is my favourite colour");
    assert_eq!(out.reply, UNKNOWN_REPLY);
    assert!(!out.reply.contains("blue"));
    assert!(out
        .sources
        .iter()
        .all(|source| !source.starts_with("memory:")));
    assert!(dir.join("person-memory.sqlite").is_file());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn memory_never_answers_a_world_fact() {
    let dir = scratch("world");
    let mut brain = horbm::Brain::with_cards(Kettle, None)
        .with_memory(EpisodicStore::open(dir.join("person-memory.sqlite"), true));
    brain.set_person("person-alpha");
    brain.turn("remember that the kettle is on the moon");
    brain.turn("remember that the capital of france is lyon");
    let stored = brain
        .person_memory()
        .expect("diary")
        .recall("person-alpha", "what is the capital of france");
    assert!(
        stored.iter().any(|hit| hit.text.contains("lyon")),
        "the pin is in the diary so the guard is what blocks the answer: {stored:?}"
    );
    assert!(!about_the_person("what is the capital of france"));
    assert!(!about_the_person("where is the kettle"));
    let capital = brain.turn("what is the capital of france");
    assert_eq!(capital.reply, UNKNOWN_REPLY);
    assert!(!capital.reply.to_lowercase().contains("lyon"));
    assert!(capital
        .sources
        .iter()
        .all(|source| !source.starts_with("memory:")));
    let kettle = brain.turn("where is the kettle");
    assert_eq!(kettle.reply, "The kettle is on the hob.");
    assert_eq!(kettle.sources, vec!["card:kettle".to_string()]);
    assert!(!kettle.reply.to_lowercase().contains("moon"));
    assert!(kettle
        .sources
        .iter()
        .all(|source| !source.starts_with("memory:")));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_same_script_produces_the_same_rows() {
    let left = scratch("same-a");
    let right = scratch("same-b");
    let once = script(&left);
    let twice = script(&right);
    assert_eq!(once, twice);
    assert!(once.session_text.contains("quiet"), "{}", once.session_text);
    assert!(
        !once.session_text.to_lowercase().contains("haha"),
        "{}",
        once.session_text
    );
    assert_eq!(once.colour_text, "my favourite colour is blue");
    assert!(once.name_text.contains("Nia"), "{}", once.name_text);
    let _ = fs::remove_dir_all(&left);
    let _ = fs::remove_dir_all(&right);
}

#[test]
fn a_secret_is_not_written_and_withdraw_hides_recall() {
    let dir = scratch("secret");
    let path = dir.join("person-memory.sqlite");
    let (sink, stored, extracted) = counter();
    let mut store = EpisodicStore::open(&path, true);
    store.set_sink(Box::new(sink));
    assert_eq!(
        store.pin("person-alpha", "my password is example", "t000001"),
        WriteOutcome::Skipped
    );
    assert_eq!(
        store.note_turn(
            "person-alpha",
            "person-alpha",
            "my password is example",
            "t000001"
        ),
        WriteOutcome::Skipped
    );
    assert_eq!(store.writes(), 0);
    assert_eq!(store.extractions(), 0);
    assert!(!path.exists());
    assert_eq!(stored.load(Ordering::SeqCst), 0);
    assert_eq!(extracted.load(Ordering::SeqCst), 0);

    let pin = store.pin("person-alpha", "my favourite colour is blue", "t000002");
    let WriteOutcome::Stored { id } = pin else {
        panic!("pin was not stored: {pin:?}");
    };
    assert!(path.is_file());
    assert!(store.holds_text("my favourite colour is blue"));
    store.set_remember(false);
    assert!(store
        .recall("person-alpha", "what is my favourite colour")
        .is_empty());
    assert_eq!(
        store.pin("person-alpha", "my favourite colour is green", "t000003"),
        WriteOutcome::Refused
    );
    assert_eq!(stored.load(Ordering::SeqCst), 1);
    store.set_remember(true);
    let back = store.recall("person-alpha", "what is my favourite colour");
    assert_eq!(back.len(), 1, "{back:?}");
    assert_eq!(back[0].id, id);
    assert_eq!(back[0].text, "my favourite colour is blue");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn person_memory_is_a_different_file_from_truth_cards() {
    let dir = scratch("split");
    let cards = dir.join("cards");
    fs::create_dir(&cards).expect("cards");
    let card = cards.join("kettle.txt");
    let body = b"kettle card stays on disk\n";
    fs::write(&card, body).expect("card");
    let memory_path = dir.join("person-memory.sqlite");
    let mut store = EpisodicStore::open(&memory_path, true);
    store
        .pin("person-alpha", "my favourite colour is blue", "t000001")
        .ok_stored();
    let colour = store.recall("person-alpha", "what is my favourite colour");
    assert_eq!(colour.len(), 1, "{colour:?}");
    assert!(colour.iter().all(|hit| !hit.text.contains("stays on disk")));
    assert_eq!(fs::read(&card).expect("reread"), body);
    let thread = RetrievalThread::spawn(RetrievalConfig {
        remember: true,
        knowledge_dir: Some(cards),
        fixture_rows: 0,
        keys_path: None,
    })
    .expect("thread");
    thread.ask_wait("favourite colour blue", 1, 1).expect("ask");
    let cell = thread.cell();
    assert_eq!(cell.state, CellState::Miss);
    assert!(cell.hits.iter().all(|hit| !hit.text.contains("blue")));
    assert!(cell.hits.iter().all(|hit| hit.item_id != colour[0].id));
    let _ = fs::remove_dir_all(&dir);
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Snapshot {
    pin: WriteOutcome,
    state: WriteOutcome,
    summary: WriteOutcome,
    colour_text: String,
    colour_at: String,
    name_text: String,
    session_text: String,
    extractions: u64,
}

fn script(dir: &Path) -> Snapshot {
    let mut store = EpisodicStore::open(dir.join("person-memory.sqlite"), true);
    let pin = store.pin("person-alpha", "my favourite colour is blue", "t000001");
    let state = store.note_state("person-alpha", &state(), "t000002");
    store
        .note_turn("person-alpha", "person-alpha", "my name is Nia", "t000003")
        .ok_stored();
    store
        .note_turn(
            "person-alpha",
            "person-alpha",
            "I prefer quiet mornings",
            "t000004",
        )
        .ok_stored();
    store
        .note_turn(
            "person-alpha",
            "person-alpha",
            "haha that was a joke",
            "t000005",
        )
        .ok_stored();
    let summary = store.end_session("person-alpha", "t000006");
    let colour = store.recall("person-alpha", "what is my favourite colour");
    let name = store.recall("person-alpha", "what is my name");
    let session = store.recall("person-alpha", "quiet mornings");
    assert_eq!(colour.len(), 1, "{colour:?}");
    assert_eq!(name.len(), 1, "{name:?}");
    assert_eq!(session.len(), 1, "{session:?}");
    assert_eq!(store.extractions(), 1);
    Snapshot {
        pin,
        state,
        summary,
        colour_text: colour[0].text.clone(),
        colour_at: colour[0].at.clone(),
        name_text: name[0].text.clone(),
        session_text: session[0].text.clone(),
        extractions: store.extractions(),
    }
}

trait OutcomeExt {
    fn ok_stored(self);
}

impl OutcomeExt for WriteOutcome {
    fn ok_stored(self) {
        assert!(matches!(self, WriteOutcome::Stored { .. }), "{self:?}");
    }
}

struct EmptyCards;

impl horbm::CardLookup for EmptyCards {
    fn lookup(&self, _query: &str) -> Option<horbm::CardHit> {
        None
    }
}

struct Kettle;

impl horbm::CardLookup for Kettle {
    fn lookup(&self, query: &str) -> Option<horbm::CardHit> {
        if query.to_ascii_lowercase().contains("kettle") {
            Some(horbm::CardHit {
                id: "kettle".into(),
                gist: "The kettle is on the hob.".into(),
            })
        } else {
            None
        }
    }
}
