// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! `PersonMemory::list_person` over a temp diary and the northwind fixture.
//!
//! Recall is a search and stops at `MEMORY_HIT_CAP`. This list is every live
//! episode and diary row for one person, newest `source_date` first, and it
//! stops at `limit.min(MEMORY_HIT_CAP * 4)`. A forgotten pin and its
//! session-ring twin stay out.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use horbm::{EpisodicStore, MemoryKind, PersonMemory, PersonState, WriteOutcome, MEMORY_HIT_CAP};

const PERSON: &str = "northwind";
const OTHER: &str = "person-beta";

fn scratch(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "horbm-f7-list-{name}-{}-{nanos}",
        std::process::id()
    ));
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

fn expect_stored(outcome: &WriteOutcome) {
    assert!(
        matches!(outcome, WriteOutcome::Stored { .. }),
        "{outcome:?}"
    );
}

fn mood_only(feeling: &str) -> PersonState {
    PersonState {
        names: Vec::new(),
        arousal: 0.3,
        valence: 0.25,
        dominance: 0.5,
        feeling: Some(feeling.to_string()),
    }
}

#[test]
fn two_pins_and_a_state_row_are_newest_first() {
    let dir = scratch("three");
    let path = dir.join("person-memory.sqlite");
    let mut store = EpisodicStore::open(&path, true);
    expect_stored(&store.pin(
        PERSON,
        "the kettle stays on the hob",
        "2026-11-25T00:00:01Z",
    ));
    expect_stored(&store.note_state(PERSON, &mood_only("calm"), "2026-11-25T00:00:02Z"));
    expect_stored(&store.pin(PERSON, "my sister is Ana", "2026-11-25T00:00:03Z"));
    let listed = store.list_person(PERSON, 32);
    assert_eq!(listed.len(), 3, "{listed:?}");
    assert_eq!(listed[0].text, "my sister is Ana");
    assert_eq!(listed[0].kind, MemoryKind::Pin);
    assert!(listed[0].pinned);
    assert_eq!(listed[0].at, "2026-11-25T00:00:03Z");
    assert_eq!(listed[1].kind, MemoryKind::Mood);
    assert_eq!(listed[1].source, "state-tracker");
    assert!(
        listed[1].text.starts_with("i am feeling calm"),
        "{listed:?}"
    );
    assert!(!listed[1].pinned);
    assert_eq!(listed[1].at, "2026-11-25T00:00:02Z");
    assert_eq!(listed[2].text, "the kettle stays on the hob");
    assert_eq!(listed[2].kind, MemoryKind::Pin);
    assert_eq!(listed[2].at, "2026-11-25T00:00:01Z");
    assert!(listed.iter().all(|item| item.person_id == PERSON));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_forgotten_pin_and_its_twin_are_absent() {
    let dir = scratch("forgotten");
    let path = dir.join("person-memory.sqlite");
    let text = "my sister is Ana";
    let id = {
        let mut store = EpisodicStore::open(&path, true);
        let pinned = store.pin(PERSON, text, "t000001");
        let WriteOutcome::Stored { id } = pinned else {
            panic!("pin: {pinned:?}");
        };
        expect_stored(&store.note_turn(PERSON, PERSON, text, "t000001"));
        expect_stored(&store.end_session(PERSON, "t000099"));
        id
    };
    let before = EpisodicStore::read_transcript_file(&path, "Ana").expect("before");
    assert!(
        before.extracted >= 1,
        "the noted turn did not store a candidate twin"
    );
    let mut store = EpisodicStore::open(&path, true);
    let live = store.list_person(PERSON, 32);
    assert!(
        live.iter()
            .any(|item| item.id == id && item.source == "pin" && item.text == text),
        "{live:?}"
    );
    let archived = store.forget(PERSON, &id);
    assert!(
        matches!(archived, WriteOutcome::Archived { .. }),
        "{archived:?}"
    );
    let listed = store.list_person(PERSON, 32);
    assert!(
        listed
            .iter()
            .all(|item| item.id != id && item.source != "pin"),
        "{listed:?}"
    );
    assert!(
        listed.iter().all(|item| item.source != "session-ring"),
        "twin resurfaced: {listed:?}"
    );
    drop(store);
    let mut reopened = EpisodicStore::open(&path, true);
    let listed = reopened.list_person(PERSON, 32);
    assert!(
        listed
            .iter()
            .all(|item| item.id != id && item.source != "pin"),
        "{listed:?}"
    );
    assert!(
        listed.iter().all(|item| item.source != "session-ring"),
        "twin resurfaced after reopen: {listed:?}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn another_persons_rows_are_absent() {
    let dir = scratch("other");
    let path = dir.join("person-memory.sqlite");
    let mut store = EpisodicStore::open(&path, true);
    expect_stored(&store.pin(PERSON, "the kettle stays on the hob", "t000001"));
    expect_stored(&store.pin(OTHER, "guest keeps a blue notebook", "t000002"));
    expect_stored(&store.note_state(PERSON, &mood_only("calm"), "t000003"));
    let mine = store.list_person(PERSON, 32);
    assert!(!mine.is_empty(), "{mine:?}");
    assert!(mine.iter().all(|item| item.person_id == PERSON), "{mine:?}");
    assert!(
        mine.iter().all(|item| !item.text.contains("blue notebook")),
        "{mine:?}"
    );
    let theirs = store.list_person(OTHER, 32);
    assert_eq!(theirs.len(), 1, "{theirs:?}");
    assert_eq!(theirs[0].person_id, OTHER);
    assert_eq!(theirs[0].text, "guest keeps a blue notebook");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn denied_list_is_empty() {
    let dir = scratch("denied");
    let path = dir.join("person-memory.sqlite");
    let mut denied = EpisodicStore::open(&path, false);
    assert!(!denied.remember_granted());
    assert!(denied.list_person(PERSON, 8).is_empty());
    assert!(!path.exists());
    assert!(sqlite_names(&dir).is_empty(), "{:?}", sqlite_names(&dir));
    drop(denied);
    {
        let mut granted = EpisodicStore::open(&path, true);
        expect_stored(&granted.pin(PERSON, "the kettle stays on the hob", "t000001"));
        assert_eq!(granted.list_person(PERSON, 8).len(), 1);
    }
    let mut denied = EpisodicStore::open(&path, false);
    assert!(denied.list_person(PERSON, 8).is_empty());
    assert!(path.is_file());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_limit_is_honoured() {
    let dir = scratch("limit");
    let path = dir.join("person-memory.sqlite");
    let mut store = EpisodicStore::open(&path, true);
    let n = MEMORY_HIT_CAP * 4 + 4;
    for i in 0..n {
        let text = format!("fact {i} stays on the shelf");
        let at = format!("2026-11-26T00:{i:02}:00Z");
        expect_stored(&store.pin(PERSON, &text, &at));
    }
    let capped = store.list_person(PERSON, usize::MAX);
    assert_eq!(capped.len(), MEMORY_HIT_CAP * 4, "{}", capped.len());
    assert_eq!(capped[0].text, format!("fact {} stays on the shelf", n - 1));
    let oldest = n - MEMORY_HIT_CAP * 4;
    for i in 0..oldest {
        let text = format!("fact {i} stays on the shelf");
        assert!(
            capped.iter().all(|item| item.text != text),
            "older than the cap: {text} in {capped:?}"
        );
    }
    let two = store.list_person(PERSON, 2);
    assert_eq!(two.len(), 2, "{two:?}");
    assert_eq!(two[0].text, capped[0].text);
    assert_eq!(two[1].text, capped[1].text);
    assert!(store.list_person(PERSON, 0).is_empty());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_missing_db_is_empty_and_creates_nothing() {
    let dir = scratch("missing");
    let path = dir.join("absent").join("person-memory.sqlite");
    assert!(!path.exists());
    assert!(!path.parent().expect("parent").exists());
    let mut store = EpisodicStore::open(&path, true);
    assert!(store.remember_granted());
    assert!(store.list_person(PERSON, 8).is_empty());
    assert!(!path.exists());
    assert!(!path.parent().expect("parent").exists());
    assert!(sqlite_names(&dir).is_empty(), "{:?}", sqlite_names(&dir));
    let _ = fs::remove_dir_all(&dir);
}
