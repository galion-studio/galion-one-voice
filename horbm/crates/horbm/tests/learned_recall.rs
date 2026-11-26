// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! A granted retrieval thread recalls a row the learn pack wrote.
//!
//! The card is one line written into a temp directory. Nothing here is a
//! committed card. `LearnPass` writes the taught clause. Recall reads it
//! back through `RetrievalThread::set_learned`.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use horbm::{
    CellState, ClauseSource, HitPool, LearnConfig, LearnPass, LearnedRow, RetrievalConfig,
    RetrievalThread, RetrievedCells,
};

/// One generated card. The learned clauses below do not all share its words.
const CARD_LINE: &str = r#"{"id":"kettle","title":"The kettle is on the hob.","section":"horbm","category":"fixture","summary":"The kettle is on the hob.","body":"The kettle is on the hob.","claim_type":"fact","sources":[],"evidence_strength":"strong","said_by":"northwind","as_of_date":"2026-11-25","status":"draft","contested_views":[],"related":[]}"#;

const TAUGHT: &str = "The spare oar is in the locker.";

/// Same value as `LEARNED_ROW_CAP`. Round 8 re-exports the const.
const LEARNED_CAP: usize = 256;

fn scratch(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "horbm-learned-{name}-{}-{nanos}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).expect("scratch");
    dir
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
        .write_clause("northwind", text, &source)
        .expect("write clause");
    let audit = learn.audit().expect("audit");
    assert_eq!(audit.rows.len(), 1, "{:?}", audit.rows);
    assert_eq!(audit.rows[0].text, text);
    audit.rows
}

fn spawn(remember: bool, cards: &Path) -> RetrievalThread {
    RetrievalThread::spawn(RetrievalConfig {
        remember,
        knowledge_dir: Some(cards.to_path_buf()),
        fixture_rows: 0,
        keys_path: None,
    })
    .expect("spawn")
}

fn learned(hits: &[horbm::MemoryHit]) -> Vec<&horbm::MemoryHit> {
    hits.iter()
        .filter(|hit| hit.source.starts_with("learned;"))
        .collect()
}

#[test]
fn granted_learned_row_is_a_memory_hit_with_the_learned_prefix() {
    let root = scratch("grant");
    let cards = root.join("cards");
    let pack = root.join("pack");
    write_card(&cards);
    let rows = teach(&pack, TAUGHT);
    let label = rows[0].source.clone();

    let thread = spawn(true, &cards);
    thread.set_learned(rows).expect("set learned");
    thread.ask_wait("spare oar", 1, 1).expect("ask");
    let cell = thread.cell();
    assert_eq!(cell.state, CellState::Hits, "{:?}", cell);
    let hits = learned(&cell.hits);
    assert_eq!(hits.len(), 1, "{:?}", cell.hits);
    assert_eq!(hits[0].pool, HitPool::Memory);
    assert_eq!(hits[0].source, format!("learned;{label}"));
    assert_eq!(hits[0].text, TAUGHT);
    assert_eq!(hits[0].said_by, "northwind");
    assert!(cell.answer.is_empty());
    drop(thread);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn denied_thread_ignores_learned_rows_and_opens_nothing() {
    let root = scratch("deny");
    let cards = root.join("cards");
    let pack = root.join("pack");
    write_card(&cards);
    let rows = teach(&pack, TAUGHT);
    let mut before = fs::read_dir(&cards)
        .expect("cards")
        .flatten()
        .map(|ent| ent.file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    before.sort();

    let thread = spawn(false, &cards);
    thread.set_learned(rows).expect("set learned");
    assert!(thread.last_error().is_none(), "{:?}", thread.last_error());
    thread.ask_wait("spare oar", 1, 1).expect("ask");
    let cell = thread.cell();
    assert!(cell.is_empty(), "{:?}", cell);
    assert_eq!(cell.state, CellState::Idle);
    assert!(cell.hits.is_empty());
    let mut after = fs::read_dir(&cards)
        .expect("cards")
        .flatten()
        .map(|ent| ent.file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    after.sort();
    assert_eq!(after, before, "denied thread wrote the card dir");
    assert!(after.iter().all(|name| !name.contains("sqlite")));
    assert!(!cards.join("revisions.sqlite").exists());
    drop(thread);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn rows_survive_withdraw_and_regrant() {
    let root = scratch("regrant");
    let cards = root.join("cards");
    let pack = root.join("pack");
    write_card(&cards);
    let rows = teach(&pack, TAUGHT);
    let thread = spawn(true, &cards);
    thread.set_learned(rows).expect("set learned");
    thread.ask_wait("spare oar", 1, 1).expect("ask");
    assert_eq!(learned(&thread.cell().hits).len(), 1);

    thread.set_remember(false).expect("withdraw");
    assert!(thread.cell().is_empty());
    thread.ask_wait("spare oar", 2, 2).expect("ask denied");
    assert!(thread.cell().is_empty());

    thread.set_remember(true).expect("regrant");
    thread.ask_wait("spare oar", 3, 3).expect("ask again");
    let cell = thread.cell();
    assert_eq!(cell.state, CellState::Hits, "{:?}", cell);
    let hits = learned(&cell.hits);
    assert_eq!(hits.len(), 1, "{:?}", cell.hits);
    assert_eq!(hits[0].pool, HitPool::Memory);
    assert!(hits[0].source.starts_with("learned;"));
    assert_eq!(hits[0].text, TAUGHT);
    drop(thread);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn empty_set_clears_them() {
    let root = scratch("clear");
    let cards = root.join("cards");
    let pack = root.join("pack");
    write_card(&cards);
    let rows = teach(&pack, TAUGHT);
    let thread = spawn(true, &cards);
    thread.set_learned(rows).expect("set learned");
    thread.ask_wait("spare oar", 1, 1).expect("ask");
    assert_eq!(thread.cell().state, CellState::Hits);

    thread.set_learned(Vec::new()).expect("clear");
    thread.ask_wait("spare oar", 2, 2).expect("ask after clear");
    let cell = thread.cell();
    assert_eq!(cell.state, CellState::Miss, "{:?}", cell);
    assert!(learned(&cell.hits).is_empty());
    assert!(cell.hits.is_empty());
    drop(thread);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn cap_keeps_the_newest() {
    let root = scratch("cap");
    let extra = 10;
    let total = LEARNED_CAP + extra;
    let rows: Vec<LearnedRow> = (0..total)
        .map(|i| LearnedRow {
            item_id: format!("row-{i}"),
            text: format!("capmarker{i:04} is the stored line"),
            said_by: "user".to_string(),
            date: format!("2024-06-01T00:00:{i:04}Z"),
            source: format!("session=cap;turn={i};time=2024-06-01T00:00:{i:04}Z;consent=learn-v1"),
        })
        .collect();
    let thread = RetrievalThread::spawn(RetrievalConfig {
        remember: true,
        knowledge_dir: None,
        fixture_rows: 0,
        keys_path: None,
    })
    .expect("spawn");
    thread.set_learned(rows).expect("set learned");
    let dropped = format!("dropped {extra} learned rows");
    assert_eq!(thread.last_error().as_deref(), Some(dropped.as_str()));

    thread.ask_wait("capmarker0000", 1, 1).expect("oldest");
    assert_eq!(thread.cell().state, CellState::Miss, "{:?}", thread.cell());

    let oldest_kept = format!("capmarker{extra:04}");
    thread
        .ask_wait(oldest_kept.as_str(), 2, 2)
        .expect("oldest kept");
    let kept = thread.cell();
    assert_eq!(kept.state, CellState::Hits, "{:?}", kept);
    assert!(
        learned(&kept.hits)
            .iter()
            .any(|hit| hit.text.contains(&oldest_kept)),
        "{:?}",
        kept.hits
    );

    let newest = format!("capmarker{:04}", total - 1);
    thread.ask_wait(newest.as_str(), 3, 3).expect("newest");
    let cell = thread.cell();
    assert_eq!(cell.state, CellState::Hits, "{:?}", cell);
    let hits = learned(&cell.hits);
    assert!(
        hits.iter()
            .any(|hit| hit.pool == HitPool::Memory && hit.text.contains(&newest)),
        "{:?}",
        cell.hits
    );
    drop(thread);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_learned_row_does_not_shadow_a_card_that_covers_the_query() {
    let root = scratch("shadow");
    let cards = root.join("cards");
    let pack = root.join("pack");
    write_card(&cards);
    let rows = teach(&pack, "Someone left the kettle on the hob.");
    let thread = spawn(true, &cards);
    thread.set_learned(rows).expect("set learned");
    thread.ask_wait("kettle", 1, 1).expect("ask");
    let cell = thread.cell();
    assert_eq!(cell.state, CellState::Hits, "{:?}", cell);
    assert!(!cell.hits.is_empty());
    assert_eq!(cell.hits[0].item_id, "kettle", "{:?}", cell.hits);
    assert_eq!(cell.hits[0].pool, HitPool::Knowledge);
    assert!(!cell.hits[0].source.starts_with("learned;"));
    assert!(
        learned(&cell.hits)
            .iter()
            .any(|hit| hit.pool == HitPool::Memory),
        "learned row missing from {:?}",
        cell.hits
    );
    drop(thread);
    let _ = fs::remove_dir_all(&root);
}
