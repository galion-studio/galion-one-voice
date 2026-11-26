// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Merged memory behaviour, locked through `horbm`'s public API.
//!
//! "please forget my sister" after a pin speaks the persona's forgot line.
//! Recall no longer returns that pin, and the request line is not stored.
//! [`horbm::PersonMemory::list_person`] is newest first, omits a forgotten
//! row, and is empty when remember is denied.
//! [`horbm::LearnPass::retract_clause`] returns `Ok(true)` once, then
//! `Ok(false)`, and the audit shrinks.
//! [`horbm::RetrievalThread::reload`] picks up a new card file and drops a
//! removed one. [`horbm::Brain::attach_learned`] speaks `learned:`. A long
//! reply that is not a card clause is cut to the persona's sentence cap. A
//! card clause is not. [`horbm::LEARN_PACK_CAP`] equals
//! [`horbm::LEARNED_ROW_CAP`].
//!
//! Fixture files live under the process temp dir and are removed when the
//! test ends. Nothing here is committed. These tests do not change the
//! process environment.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use horbm::{
    Brain, CardHit, CardLookup, CellState, ClauseSource, DialogueConsent, EpisodicStore, HitPool,
    KnowledgeCards, LearnConfig, LearnPass, LearnedRow, PersonMemory, Persona, PersonaLine,
    RetrievalConfig, RetrievalThread, RetrievedCells, WriteOutcome, LEARNED_ROW_CAP,
    LEARN_PACK_CAP, UNKNOWN_REPLY,
};

const PERSON: &str = "northwind";
const FORGOT: &str = "Okay, I've let that go.";
const PIN: &str = "my sister is Ana";
const OLDER: &str = "the kettle stays on the hob";
const NEWER: &str = "my sister is Ana";
const OAR: &str = "The spare oar is in the locker.";
const HOB: &str = "The kettle is on the hob.";
const TAUGHT: &str = "You remember the spare oar is in the locker.";
const KETTLE: &str = "The kettle is on the hob.";
const LANTERN: &str = "The lantern is in the shed.";
const LONG_CARD: &str = "The kettle is on the hob. The hob is hot. The lid is shut.";
const LONG_PLAIN: &str = "Alpha stays. Bravo stays. Charlie goes.";

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

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "horbm-eval-memory-lock-{name}-{}-{nanos}",
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
    let persona = Persona::parse(NORTHWIND).expect("northwind persona");
    assert_eq!(persona.id(), PERSON);
    assert_eq!(persona.line(PersonaLine::Forgot), Some(FORGOT));
    assert_eq!(persona.style().max_sentences, 2);
    persona
}

fn source(turn_id: u64) -> ClauseSource {
    ClauseSource {
        session: format!("sess-{turn_id}"),
        turn_id,
        time: "2026-11-26T00:00:00Z".to_string(),
        consent_record: "learn-v1".to_string(),
    }
}

fn card_line(id: &str, sentence: &str) -> String {
    format!(
        r#"{{"id":"{id}","title":"{sentence}","section":"horbm","category":"fixture","summary":"{sentence}","body":"{sentence}","claim_type":"fact","sources":[],"evidence_strength":"strong","said_by":"northwind","as_of_date":"2026-11-25","status":"draft","contested_views":[],"related":[]}}"#
    )
}

fn write_card(dir: &Path, id: &str, sentence: &str) {
    fs::create_dir_all(dir).expect("cards");
    fs::write(dir.join(format!("{id}.json")), card_line(id, sentence)).expect("card");
}

fn spawn(cards: &Path) -> RetrievalThread {
    RetrievalThread::spawn(RetrievalConfig {
        remember: true,
        knowledge_dir: Some(cards.to_path_buf()),
        fixture_rows: 0,
        keys_path: None,
    })
    .expect("spawn")
}

fn teach(pack: &Path, text: &str) -> Vec<LearnedRow> {
    let learn = LearnPass::open(LearnConfig::granted(pack));
    learn
        .write_clause(PERSON, text, &source(4))
        .expect("write clause");
    let audit = learn.audit().expect("audit");
    assert_eq!(audit.rows.len(), 1, "{:?}", audit.rows);
    assert_eq!(audit.rows[0].text, text);
    audit.rows
}

struct LongCard;

impl CardLookup for LongCard {
    fn lookup(&self, query: &str) -> Option<CardHit> {
        if query.to_ascii_lowercase().contains("kettle") {
            Some(CardHit {
                id: "kettle".into(),
                gist: LONG_CARD.into(),
            })
        } else {
            None
        }
    }
}

#[test]
fn please_forget_my_sister_speaks_the_forgot_line_and_recall_drops_it() {
    let root = Scratch::new("forget");
    let path = root.path().join("person-memory.sqlite");
    let mut brain = Brain::with_persona(KnowledgeCards::empty(), None, northwind())
        .with_memory(EpisodicStore::open(&path, true));
    assert_eq!(brain.persona().id(), PERSON);
    brain.set_person(PERSON);

    let pinned = brain.turn(&format!("remember {PIN}"));
    assert_eq!(pinned.reply, UNKNOWN_REPLY);
    let before = brain
        .person_memory()
        .expect("diary")
        .recall(PERSON, "my sister");
    assert!(
        before.iter().any(|hit| hit.text == PIN),
        "the pin was not stored: {before:?}"
    );

    let forgotten = brain.turn("please forget my sister");
    assert_eq!(
        brain.persona().line(PersonaLine::Forgot),
        Some(forgotten.reply.as_str())
    );
    assert_eq!(forgotten.reply, FORGOT);
    assert_eq!(forgotten.sources, vec!["forget:1".to_string()]);
    assert!(forgotten.evidence.is_empty());
    assert!(!forgotten.reply.contains("Ana"));

    let after = brain
        .person_memory()
        .expect("diary")
        .recall(PERSON, "my sister");
    assert!(
        after.iter().all(|hit| hit.text != PIN),
        "recall still returned the pin: {after:?}"
    );
    assert!(
        after
            .iter()
            .all(|hit| !hit.text.to_ascii_lowercase().contains("please forget")),
        "recall stored the request: {after:?}"
    );

    let person = brain.person_id().to_string();
    brain
        .person_memory()
        .expect("diary")
        .end_session(&person, "t000099");
    drop(brain);

    let saved = EpisodicStore::read_transcript_file(&path, "sister").expect("reopen");
    assert!(
        saved.turns.iter().any(|turn| turn.text.contains(PIN)),
        "the pin turn was not in the transcript: {:?}",
        saved.turns
    );
    assert!(
        saved
            .turns
            .iter()
            .all(|turn| !turn.text.to_ascii_lowercase().contains("please forget")),
        "the forget request was stored: {:?}",
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
fn list_person_is_newest_first_and_a_forgotten_row_is_absent() {
    let root = Scratch::new("list");
    let path = root.path().join("person-memory.sqlite");
    let mut store = EpisodicStore::open(&path, true);
    assert!(matches!(
        store.pin(PERSON, OLDER, "2026-11-25T00:00:01Z"),
        WriteOutcome::Stored { .. }
    ));
    let newer = store.pin(PERSON, NEWER, "2026-11-25T00:00:03Z");
    let WriteOutcome::Stored { id } = newer else {
        panic!("pin: {newer:?}");
    };
    let listed = store.list_person(PERSON, 32);
    assert_eq!(listed.len(), 2, "{listed:?}");
    assert_eq!(listed[0].text, NEWER);
    assert_eq!(listed[0].at, "2026-11-25T00:00:03Z");
    assert_eq!(listed[0].person_id, PERSON);
    assert_eq!(listed[1].text, OLDER);
    assert_eq!(listed[1].at, "2026-11-25T00:00:01Z");

    let archived = store.forget(PERSON, &id);
    assert!(
        matches!(archived, WriteOutcome::Archived { .. }),
        "{archived:?}"
    );
    let listed = store.list_person(PERSON, 32);
    assert!(
        listed
            .iter()
            .all(|item| item.id != id && item.text != NEWER),
        "forgotten row still listed: {listed:?}"
    );
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert_eq!(listed[0].text, OLDER);
}

#[test]
fn denied_list_person_is_empty() {
    let root = Scratch::new("denied-list");
    let path = root.path().join("person-memory.sqlite");
    let mut denied = EpisodicStore::open(&path, false);
    assert!(!denied.remember_granted());
    assert!(denied.list_person(PERSON, 8).is_empty());
    assert!(!path.exists());
    drop(denied);

    {
        let mut granted = EpisodicStore::open(&path, true);
        assert!(matches!(
            granted.pin(PERSON, OLDER, "t000001"),
            WriteOutcome::Stored { .. }
        ));
        assert_eq!(granted.list_person(PERSON, 8).len(), 1);
    }
    let mut denied = EpisodicStore::open(&path, false);
    assert!(!denied.remember_granted());
    assert!(denied.list_person(PERSON, 8).is_empty());
    assert!(path.is_file());
}

#[test]
fn retract_clause_is_true_once_then_false_and_the_audit_shrinks() {
    let root = Scratch::new("retract");
    let pack = root.path().join("pack");
    let learn = LearnPass::open(LearnConfig::granted(&pack));
    learn.write_clause(PERSON, OAR, &source(1)).expect("oar");
    learn.write_clause(PERSON, HOB, &source(2)).expect("hob");
    let before = learn.audit().expect("audit");
    assert_eq!(before.rows.len(), 2, "{:?}", before.rows);

    assert!(learn.retract_clause(PERSON, HOB).expect("once"));
    let after = learn.audit().expect("audit after retract");
    assert!(after.rows.len() < before.rows.len(), "{:?}", after.rows);
    assert_eq!(after.rows.len(), 1, "{:?}", after.rows);
    assert!(after.rows.iter().all(|row| row.text != HOB));
    assert!(after
        .rows
        .iter()
        .any(|row| row.text == OAR && row.said_by == PERSON));

    assert!(!learn.retract_clause(PERSON, HOB).expect("again"));
    let still = learn.audit().expect("audit stayed");
    assert_eq!(still.rows.len(), after.rows.len());
    assert!(still.rows.iter().all(|row| row.text != HOB));
    assert!(still.rows.iter().any(|row| row.text == OAR));
}

#[test]
fn reload_picks_up_a_new_card_and_drops_a_removed_one() {
    let root = Scratch::new("reload");
    let cards = root.path().join("cards");
    write_card(&cards, "kettle", KETTLE);
    let thread = spawn(&cards);

    thread.ask_wait("kettle", 1, 1).expect("ask kettle");
    let loaded = thread.cell();
    assert_eq!(loaded.state, CellState::Hits, "{loaded:?}");
    assert_eq!(loaded.hits[0].item_id, "kettle");
    assert_eq!(loaded.hits[0].pool, HitPool::Knowledge);
    assert_eq!(loaded.hits[0].clause(), KETTLE);
    thread
        .ask_wait("lantern", 2, 2)
        .expect("ask missing lantern");
    let missing = thread.cell();
    assert_eq!(missing.state, CellState::Miss, "{missing:?}");
    assert!(missing.hits.iter().all(|hit| hit.item_id != "lantern"));

    write_card(&cards, "lantern", LANTERN);
    thread.reload().expect("reload add");
    assert!(thread.last_error().is_none(), "{:?}", thread.last_error());
    thread.ask_wait("lantern", 3, 3).expect("ask lantern");
    let added = thread.cell();
    assert_eq!(added.state, CellState::Hits, "{added:?}");
    assert_eq!(added.hits[0].item_id, "lantern");
    assert_eq!(added.hits[0].pool, HitPool::Knowledge);
    assert_eq!(added.hits[0].clause(), LANTERN);
    thread
        .ask_wait("kettle", 4, 4)
        .expect("ask kettle still there");
    assert_eq!(thread.cell().hits[0].item_id, "kettle");

    fs::remove_file(cards.join("kettle.json")).expect("remove kettle");
    thread.reload().expect("reload remove");
    assert!(thread.last_error().is_none(), "{:?}", thread.last_error());
    thread.ask_wait("kettle", 5, 5).expect("ask removed kettle");
    let gone = thread.cell();
    assert_eq!(gone.state, CellState::Miss, "{gone:?}");
    assert!(gone.hits.iter().all(|hit| hit.item_id != "kettle"));
    thread.ask_wait("lantern", 6, 6).expect("ask lantern kept");
    let kept = thread.cell();
    assert_eq!(kept.state, CellState::Hits, "{kept:?}");
    assert_eq!(kept.hits[0].item_id, "lantern");
    assert_eq!(kept.hits[0].clause(), LANTERN);
    drop(thread);
}

#[test]
fn attach_learned_speaks_a_learned_source() {
    let root = Scratch::new("learned");
    let cards = root.path().join("cards");
    let pack = root.path().join("pack");
    write_card(&cards, "kettle", KETTLE);
    let rows = teach(&pack, TAUGHT);

    let thread = spawn(&cards);
    let mut brain =
        Brain::with_persona(KnowledgeCards::empty(), None, northwind()).with_retrieval(thread);
    brain.attach_learned(rows).expect("attach");
    let out = brain.turn("spare oar");
    let cell = brain.retrieval_cell().expect("cell");
    let hit = cell.hits.first().expect("a learned hit");
    assert!(hit.source.starts_with("learned;"), "{hit:?}");
    assert_eq!(out.reply, TAUGHT);
    assert_eq!(out.sources, vec![format!("learned:{}", hit.item_id)]);
    assert!(out.sources[0].starts_with("learned:"), "{:?}", out.sources);
    assert!(out
        .sources
        .iter()
        .all(|source| !source.starts_with("card:")));
}

#[test]
fn a_long_non_card_reply_is_capped_and_a_card_clause_is_not() {
    let mut brain = Brain::with_persona(LongCard, None, northwind());
    assert_eq!(brain.persona().style().max_sentences, 2);
    brain.set_dialogue_consent(DialogueConsent {
        remember: true,
        learn: false,
        check_ins: true,
    });
    assert!(brain.note_callback(LONG_PLAIN));

    let plain = brain.turn("hello");
    assert_eq!(plain.arm, "offer_topic");
    assert_eq!(plain.reply, "Alpha stays. Bravo stays.");
    assert!(!plain.reply.contains("Charlie"));
    assert!(plain.reply.len() < LONG_PLAIN.len());
    assert_eq!(plain.sources, vec!["memory:callback".to_string()]);
    assert!(plain
        .sources
        .iter()
        .all(|source| !source.starts_with("card:")));

    let card = brain.turn("where is the kettle");
    assert_eq!(card.reply, LONG_CARD);
    assert!(card.reply.contains("The lid is shut."));
    assert_eq!(card.sources, vec!["card:kettle".to_string()]);
}

#[test]
fn learn_pack_cap_equals_the_learned_row_cap() {
    assert_eq!(LEARN_PACK_CAP, LEARNED_ROW_CAP);
}
