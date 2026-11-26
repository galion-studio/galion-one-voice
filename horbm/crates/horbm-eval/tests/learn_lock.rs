// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Learn pack, taught seal, learned recall, sentence cap, and persona lines,
//! locked through `horbm`'s public API.
//!
//! `horbm-eval` depends on `horbm` and not on `horbm-persona`. Fixture files
//! are written under the process temp dir and removed when the test ends.
//! Nothing here is committed. These tests do not change the process environment.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use horbm::{
    CellState, ClauseSource, HitPool, LearnConfig, LearnPass, LearnedRow, MemoryHit, Persona,
    PersonaError, RetrievalConfig, RetrievalThread, RetrievedCells, SessionSeal, StyleHook,
    UNKNOWN_LINE,
};

const PERSON: &str = "northwind";
const OAR: &str = "The spare oar is in the locker.";
const HOB: &str = "The kettle is on the hob.";
const STOVE: &str = "the kettle is on the stove";
const MISS: &str = "where is the kettle";
const LEFT_ON_HOB: &str = "Someone left the kettle on the hob.";

/// One generated card. Written into a temp directory. Not a committed card.
const CARD_LINE: &str = r#"{"id":"kettle","title":"The kettle is on the hob.","section":"horbm","category":"fixture","summary":"The kettle is on the hob.","body":"The kettle is on the hob.","claim_type":"fact","sources":[],"evidence_strength":"strong","said_by":"northwind","as_of_date":"2026-11-25","status":"draft","contested_views":[],"related":[]}"#;

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "horbm-eval-learn-lock-{name}-{}-{nanos}",
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

fn source(turn_id: u64) -> ClauseSource {
    ClauseSource {
        session: format!("sess-{turn_id}"),
        turn_id,
        time: "2026-11-26T00:00:00Z".to_string(),
        consent_record: "learn-v1".to_string(),
    }
}

fn names(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let Ok(rd) = fs::read_dir(dir) else {
        return out;
    };
    for ent in rd.flatten() {
        out.push(ent.file_name().to_string_lossy().into_owned());
    }
    out.sort();
    out
}

fn write_card(cards: &Path) {
    fs::create_dir_all(cards).expect("cards");
    fs::write(cards.join("kettle.json"), CARD_LINE).expect("card");
}

fn teach(pack: &Path, text: &str) -> Vec<LearnedRow> {
    let learn = LearnPass::open(LearnConfig::granted(pack));
    learn
        .write_clause(PERSON, text, &source(4))
        .expect("write clause");
    let audit = learn.audit().expect("audit");
    assert_eq!(audit.rows.len(), 1, "{:?}", audit.rows);
    assert_eq!(audit.rows[0].text, text);
    assert_eq!(audit.rows[0].said_by, PERSON);
    audit.rows
}

fn spawn(remember: bool, cards: Option<&Path>) -> RetrievalThread {
    RetrievalThread::spawn(RetrievalConfig {
        remember,
        knowledge_dir: cards.map(Path::to_path_buf),
        fixture_rows: 0,
        keys_path: None,
    })
    .expect("spawn")
}

fn learned(hits: &[MemoryHit]) -> Vec<&MemoryHit> {
    hits.iter()
        .filter(|hit| hit.source.starts_with("learned;"))
        .collect()
}

/// Stand-in file with the fixture id and a two-sentence cap. No `[lines]` table.
fn northwind_text() -> String {
    let standin = Persona::standin();
    let text = standin.to_toml();
    let id_line = format!("id = \"{}\"", standin.id());
    let text = text.replace(&id_line, "id = \"northwind\"");
    let cap_line = format!("max_sentences = {}", standin.style().max_sentences);
    text.replace(&cap_line, "max_sentences = 2")
}

fn with_forgot(line: &str) -> String {
    format!("{}\n[lines]\nforgot = \"{line}\"\n", northwind_text())
}

#[test]
fn pack_keeps_distinct_clauses_and_returns_the_existing_row() {
    let root = Scratch::new("pack");
    let pack = root.path().join("pack");
    let learn = LearnPass::open(LearnConfig::granted(&pack));
    let first = learn
        .write_clause(PERSON, OAR, &source(1))
        .expect("first clause");
    let second = learn
        .write_clause(PERSON, HOB, &source(2))
        .expect("second clause");
    assert_ne!(first, second);
    let audit = learn.audit().expect("audit");
    assert_eq!(audit.rows.len(), 2);
    assert!(audit.rows.iter().any(|row| row == &first));
    assert!(audit.rows.iter().any(|row| row == &second));

    let again = learn
        .write_clause(PERSON, OAR, &source(3))
        .expect("duplicate");
    assert_eq!(again, first);
    assert_eq!(learn.audit().expect("audit after duplicate").rows.len(), 2);
}

#[test]
fn denied_learn_pass_creates_no_file() {
    let root = Scratch::new("deny-pass");
    let dir = root.path().join("pack");
    fs::create_dir_all(&dir).expect("pack dir");
    let path = dir.to_str().expect("utf-8 pack path");
    let pass = LearnPass::open(LearnConfig::from_flags(None, Some(path)));
    assert!(!pass.is_granted());
    let err = pass
        .write_clause(PERSON, OAR, &source(1))
        .expect_err("denied");
    assert!(err.is_denied(), "{err}");
    assert!(
        names(&dir).is_empty(),
        "denied pass created {:?} under {}",
        names(&dir),
        dir.display()
    );
}

#[test]
fn taught_line_is_sealed() {
    let root = Scratch::new("taught");
    let pack = root.path().join("pack");
    let clause = source(1);
    let mut seal = SessionSeal::open(LearnConfig::granted(&pack));
    assert!(seal.note_taught(MISS, PERSON, STOVE, &clause));
    let row = seal.seal().expect("seal").expect("taught line");
    assert_eq!(row.text, STOVE);
    assert_eq!(row.said_by, PERSON);
    assert_eq!(row.source, clause.label().expect("label"));
    let audit = LearnPass::open(LearnConfig::granted(&pack))
        .audit()
        .expect("audit");
    assert_eq!(audit.rows.len(), 1);
    assert_eq!(audit.rows[0], row);
}

#[test]
fn a_question_after_a_miss_is_not_kept() {
    let root = Scratch::new("question");
    let pack = root.path().join("pack");
    let mut seal = SessionSeal::open(LearnConfig::granted(&pack));
    assert!(!seal.note_taught(MISS, PERSON, "where is the kettle", &source(1)));
    assert!(!seal.note_taught(MISS, PERSON, "the kettle is on the stove?", &source(2)));
    assert!(seal.seal().expect("empty").is_none());
    assert!(!pack.exists(), "a question created {}", pack.display());
}

#[test]
fn an_empty_miss_is_not_kept() {
    let root = Scratch::new("empty-miss");
    let pack = root.path().join("pack");
    let mut seal = SessionSeal::open(LearnConfig::granted(&pack));
    assert!(!seal.note_taught("", PERSON, STOVE, &source(1)));
    assert!(!seal.note_taught("   ", PERSON, STOVE, &source(2)));
    assert!(seal.seal().expect("empty").is_none());
    assert!(!pack.exists(), "an empty miss created {}", pack.display());
}

#[test]
fn denied_seal_writes_nothing() {
    let root = Scratch::new("deny-seal");
    let pack = root.path().join("pack");
    let path = pack.to_str().expect("utf-8 pack path");
    let mut seal = SessionSeal::open(LearnConfig::from_flags(None, Some(path)));
    assert!(seal.note_taught(MISS, PERSON, STOVE, &source(1)));
    let err = seal.seal().expect_err("denied");
    assert!(err.is_denied(), "{err}");
    assert!(!pack.exists(), "denied seal created {}", pack.display());
    assert!(
        names(root.path()).is_empty(),
        "denied seal left {:?}",
        names(root.path())
    );
}

#[test]
fn learned_row_is_recalled_from_the_thread() {
    let root = Scratch::new("recall");
    let cards = root.path().join("cards");
    let pack = root.path().join("pack");
    write_card(&cards);
    let rows = teach(&pack, OAR);
    let label = rows[0].source.clone();
    let thread = spawn(true, Some(&cards));
    thread.set_learned(rows).expect("set learned");
    thread.ask_wait("spare oar", 1, 1).expect("ask");
    let cell = thread.cell();
    assert_eq!(cell.state, CellState::Hits, "{cell:?}");
    let hits = learned(&cell.hits);
    assert_eq!(hits.len(), 1, "{:?}", cell.hits);
    assert_eq!(hits[0].pool, HitPool::Memory);
    assert_eq!(hits[0].source, format!("learned;{label}"));
    assert!(hits[0].source.starts_with("learned;"));
    assert_eq!(hits[0].text, OAR);
    assert_eq!(hits[0].said_by, PERSON);
    assert!(!hits[0].item_id.is_empty());
    drop(thread);
}

#[test]
fn covering_card_stays_the_first_hit() {
    let root = Scratch::new("card");
    let cards = root.path().join("cards");
    let pack = root.path().join("pack");
    write_card(&cards);
    let rows = teach(&pack, LEFT_ON_HOB);
    let thread = spawn(true, Some(&cards));
    thread.set_learned(rows).expect("set learned");
    thread.ask_wait("kettle", 1, 1).expect("ask");
    let cell = thread.cell();
    assert_eq!(cell.state, CellState::Hits, "{cell:?}");
    assert!(!cell.hits.is_empty());
    assert_eq!(cell.hits[0].item_id, "kettle", "{:?}", cell.hits);
    assert_eq!(cell.hits[0].pool, HitPool::Knowledge);
    assert!(!cell.hits[0].source.starts_with("learned;"));
    assert!(
        learned(&cell.hits)
            .iter()
            .any(|hit| hit.pool == HitPool::Memory && hit.source.starts_with("learned;")),
        "{:?}",
        cell.hits
    );
    drop(thread);
}

#[test]
fn empty_learned_rows_clear_the_hit() {
    let root = Scratch::new("clear");
    let cards = root.path().join("cards");
    let pack = root.path().join("pack");
    write_card(&cards);
    let rows = teach(&pack, OAR);
    let thread = spawn(true, Some(&cards));
    thread.set_learned(rows).expect("set learned");
    thread.ask_wait("spare oar", 1, 1).expect("ask");
    assert_eq!(learned(&thread.cell().hits).len(), 1);

    thread.set_learned(vec![]).expect("clear");
    thread.ask_wait("spare oar", 2, 2).expect("ask after clear");
    let cell = thread.cell();
    assert!(learned(&cell.hits).is_empty(), "{:?}", cell.hits);
    assert_eq!(cell.state, CellState::Miss, "{cell:?}");
    drop(thread);
}

#[test]
fn denied_thread_gives_no_learned_hit() {
    let root = Scratch::new("deny-thread");
    let cards = root.path().join("cards");
    let pack = root.path().join("pack");
    write_card(&cards);
    let rows = teach(&pack, OAR);
    let thread = spawn(false, Some(&cards));
    thread.set_learned(rows).expect("set learned");
    thread.ask_wait("spare oar", 1, 1).expect("ask");
    let cell = thread.cell();
    assert!(learned(&cell.hits).is_empty(), "{:?}", cell.hits);
    assert!(cell.hits.is_empty());
    assert_eq!(cell.state, CellState::Idle);
    drop(thread);
}

#[test]
fn offline_taught_line_is_audited_and_recalled() {
    let root = Scratch::new("loop");
    let pack = root.path().join("pack");
    let clause = source(1);
    let mut seal = SessionSeal::open(LearnConfig::granted(&pack));
    assert!(seal.note_taught(MISS, PERSON, STOVE, &clause));
    let sealed = seal.seal().expect("seal").expect("taught line");
    assert_eq!(sealed.text, STOVE);

    let audit = LearnPass::open(LearnConfig::granted(&pack))
        .audit()
        .expect("audit");
    assert_eq!(audit.rows.len(), 1);
    assert_eq!(audit.rows[0], sealed);

    let label = audit.rows[0].source.clone();
    let thread = spawn(true, None);
    thread.set_learned(audit.rows).expect("set learned");
    thread.ask_wait("kettle", 1, 1).expect("ask");
    let cell = thread.cell();
    assert_eq!(cell.state, CellState::Hits, "{cell:?}");
    let hits = learned(&cell.hits);
    assert_eq!(hits.len(), 1, "{:?}", cell.hits);
    assert_eq!(hits[0].pool, HitPool::Memory);
    assert_eq!(hits[0].text, STOVE);
    assert_eq!(hits[0].said_by, PERSON);
    assert_eq!(hits[0].source, format!("learned;{label}"));
    assert!(hits[0].source.starts_with("learned;"));
    drop(thread);
}

#[test]
fn sentence_cap_keeps_two_and_passes_the_unknown_line() {
    let text = northwind_text();
    assert!(!text.contains("[lines]"));
    let persona = Persona::parse(&text).expect("northwind");
    assert_eq!(persona.id(), PERSON);
    let style: StyleHook = persona.style();
    assert_eq!(style.max_sentences, 2);
    let reply = "One. Two. Three.";
    let kept = style.cap_sentences(reply);
    assert_eq!(kept.trim_end(), "One. Two.");
    assert!(reply.starts_with(kept.trim_end()));
    assert_eq!(style.cap_sentences(UNKNOWN_LINE), UNKNOWN_LINE);
}

#[test]
fn persona_lines_round_trip_and_refuse_an_81_byte_line() {
    let bare = Persona::parse(&northwind_text()).expect("bare");
    assert_eq!(bare.id(), PERSON);
    assert!(!bare.to_toml().contains("[lines]"));

    let persona = Persona::parse(&with_forgot("Okay.")).expect("lines");
    let written = persona.to_toml();
    assert!(written.contains("[lines]"), "{written}");
    assert_eq!(Persona::parse(&written).expect("round trip"), persona);

    // `g` is printable and not a hex digit, so the secret-marker scan does not
    // refuse the file before the line-length check.
    let overlong = "g".repeat(81);
    assert_eq!(overlong.len(), 81);
    let err = Persona::parse(&with_forgot(&overlong)).expect_err("81-byte line");
    assert_eq!(
        err,
        PersonaError::Invalid("persona line must be 1 to 80 printable ascii bytes with no quote")
    );
}
