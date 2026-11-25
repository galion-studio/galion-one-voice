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
fn only_the_grant_word_opens_the_person_file() {
    let denied = [
        None,
        Some("yes"),
        Some("true"),
        Some("granted"),
        Some("GRANT"),
        Some("Grant"),
        Some("deny"),
        Some(""),
        Some("  "),
    ];
    for (index, flag) in denied.into_iter().enumerate() {
        let dir = scratch(&format!("flag-deny-{index}"));
        let path = dir.join("person-memory.sqlite");
        let mut store = EpisodicStore::open_from_flag(&path, flag);
        assert!(!store.remember_granted(), "{flag:?}");
        assert_eq!(store.path(), path.as_path(), "{flag:?}");
        assert!(!path.exists(), "denied open creates no file: {flag:?}");
        assert_eq!(
            store.pin("person-alpha", "my favourite colour is blue", "t000001"),
            WriteOutcome::Refused,
            "{flag:?}"
        );
        assert!(!path.exists(), "{flag:?}");
        assert!(
            sqlite_names(&dir).is_empty(),
            "{flag:?} {:?}",
            sqlite_names(&dir)
        );
        let _ = fs::remove_dir_all(&dir);
    }

    for (index, flag) in [Some("grant"), Some(" grant ")].into_iter().enumerate() {
        let dir = scratch(&format!("flag-grant-{index}"));
        let path = dir.join("person-memory.sqlite");
        let mut store = EpisodicStore::open_from_flag(&path, flag);
        assert!(store.remember_granted(), "{flag:?}");
        assert_eq!(store.path(), path.as_path(), "{flag:?}");
        assert!(
            !path.exists(),
            "the grant opens the store; the file waits for a write: {flag:?}"
        );
        store
            .pin("person-alpha", "my favourite colour is blue", "t000001")
            .ok_stored();
        assert!(path.is_file(), "{flag:?}");
        let recalled = store.recall("person-alpha", "what is my favourite colour");
        assert_eq!(recalled.len(), 1, "{flag:?} {recalled:?}");
        assert_eq!(recalled[0].text, "my favourite colour is blue");
        let _ = fs::remove_dir_all(&dir);
    }
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
    // A preference candidate can still sit beside the summary. The name
    // candidate must not sit beside the tracker row for the same name.
    let name_row = name
        .iter()
        .find(|hit| hit.source == "state-tracker" && hit.text.contains("Nia"))
        .unwrap_or_else(|| panic!("name row missing: {name:?}"));
    assert!(
        name.iter()
            .all(|hit| hit.source != "session-ring"
                || !hit.text.to_ascii_lowercase().contains("nia")),
        "name candidate still beside the tracker row: {name:?}"
    );
    let session_row = session
        .iter()
        .find(|hit| hit.source == "session-summary")
        .unwrap_or_else(|| panic!("summary missing: {session:?}"));
    assert_eq!(store.extractions(), 1);
    Snapshot {
        pin,
        state,
        summary,
        colour_text: colour[0].text.clone(),
        colour_at: colour[0].at.clone(),
        name_text: name_row.text.clone(),
        session_text: session_row.text.clone(),
        extractions: store.extractions(),
    }
}

#[test]
fn end_session_writes_every_buffered_turn_to_session_turns() {
    let dir = scratch("turns");
    let path = dir.join("person-memory.sqlite");
    let mut store = EpisodicStore::open(&path, true);
    let lines = [
        ("northwind", "quillmark my name is Northwind", "t000001"),
        ("guest", "quillmark I prefer cedar tea", "t000002"),
        ("northwind", "quillmark stays outside", "t000003"),
    ];
    for (said_by, text, at) in lines {
        store.note_turn("northwind", said_by, text, at).ok_stored();
        assert!(!path.exists(), "note_turn opened the file");
    }
    store.end_session("northwind", "t000099").ok_stored();
    assert_eq!(store.buffered_turns(), 0);
    drop(store);
    let saved =
        EpisodicStore::read_transcript_file(&path, "quillmark").expect("reopen session_turns");
    assert_eq!(
        saved.ring_len, 0,
        "session_len is the RAM ring and is empty on reopen"
    );
    assert_eq!(saved.rows, 3, "transcript_len");
    assert_eq!(
        saved.extracted, 2,
        "push_turn's extractor added a name candidate and a prefer candidate"
    );
    let texts: Vec<&str> = saved.turns.iter().map(|turn| turn.text.as_str()).collect();
    assert_eq!(
        texts,
        [
            "quillmark my name is Northwind",
            "quillmark I prefer cedar tea",
            "quillmark stays outside",
        ]
    );
    let speakers: Vec<&str> = saved
        .turns
        .iter()
        .map(|turn| turn.said_by.as_str())
        .collect();
    assert_eq!(speakers, ["northwind", "guest", "northwind"]);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn denied_end_session_writes_nothing() {
    let dir = scratch("denied-end");
    let path = dir.join("person-memory.sqlite");
    let mut store = EpisodicStore::open(&path, false);
    assert_eq!(
        store.note_turn("northwind", "northwind", "quillmark alpha", "t000001"),
        WriteOutcome::Refused
    );
    assert_eq!(
        store.end_session("northwind", "t000099"),
        WriteOutcome::Refused
    );
    assert_eq!(store.buffered_turns(), 0);
    assert!(!path.exists());
    assert!(sqlite_names(&dir).is_empty(), "{:?}", sqlite_names(&dir));
    drop(store);
    assert!(!path.exists());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn two_sessions_do_not_merge() {
    let dir = scratch("split-sessions");
    let path = dir.join("person-memory.sqlite");
    let mut store = EpisodicStore::open(&path, true);
    store
        .note_turn("northwind", "northwind", "quillmark alphaone", "t000001")
        .ok_stored();
    store.end_session("northwind", "t000099").ok_stored();
    store
        .note_turn("northwind", "northwind", "quillmark bravotwo", "t000002")
        .ok_stored();
    store.end_session("northwind", "t000199").ok_stored();
    drop(store);
    let saved =
        EpisodicStore::read_transcript_file(&path, "quillmark").expect("reopen session_turns");
    assert_eq!(saved.ring_len, 0);
    assert_eq!(saved.rows, 2, "{:?}", saved.turns);
    let texts: Vec<&str> = saved.turns.iter().map(|turn| turn.text.as_str()).collect();
    assert_eq!(texts, ["quillmark alphaone", "quillmark bravotwo"]);
    let bytes = fs::read(&path).expect("diary bytes");
    let first = EpisodicStore::transcript_session_id("northwind", "t000099");
    let second = EpisodicStore::transcript_session_id("northwind", "t000199");
    assert!(
        bytes
            .windows(first.len())
            .any(|window| window == first.as_bytes()),
        "missing {first}"
    );
    assert!(
        bytes
            .windows(second.len())
            .any(|window| window == second.as_bytes()),
        "missing {second}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn note_turn_does_no_io() {
    let dir = scratch("ram-only");
    let path = dir.join("person-memory.sqlite");
    let mut store = EpisodicStore::open(&path, true);
    store
        .note_turn("northwind", "northwind", "quillmark alpha", "t000001")
        .ok_stored();
    store
        .note_turn("northwind", "guest", "quillmark bravo", "t000002")
        .ok_stored();
    assert_eq!(store.buffered_turns(), 2);
    assert!(!path.exists());
    assert!(sqlite_names(&dir).is_empty(), "{:?}", sqlite_names(&dir));
    drop(store);
    assert!(!path.exists());
    assert!(sqlite_names(&dir).is_empty(), "{:?}", sqlite_names(&dir));
    let _ = fs::remove_dir_all(&dir);
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
