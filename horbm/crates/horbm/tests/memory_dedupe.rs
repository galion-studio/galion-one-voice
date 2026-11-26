// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Extractor candidates must not repeat a diary row, and must not outlive it.
//!
//! Counts below were traced on horbm-memory `974e5944` before the dedupe
//! (`extract_line`, candidate importance, and `MAX_HITS` = 4). This file
//! asserts the counts after dedupe. It does not claim a laptop run.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use horbm::{
    pin_command, EpisodicStore, MemoryKind, PersonMemory, PersonState, TrackedName, WriteOutcome,
    MEMORY_HIT_CAP,
};

const PERSON: &str = "northwind";

fn scratch(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "horbm-f7-dedupe-{name}-{}-{nanos}",
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

fn named(name: &str, class_label: &str) -> PersonState {
    PersonState {
        names: vec![TrackedName {
            name: name.to_string(),
            class_label: class_label.to_string(),
        }],
        arousal: 0.3,
        valence: 0.25,
        dominance: 0.5,
        feeling: None,
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

/// One granted session that holds both duplicate shapes.
///
/// Before dedupe, traced from the extractor and the search cap:
/// - diary items: 4 (name, mood, pin, summary)
/// - candidates (`extracted`): 3 (name, `rel:sister`, `live_in`)
/// - recall "what is my name": 4 hits. The state-tracker name row and the
///   session-ring name candidate are both in that window, with the transcript
///   row and the RAM ring copy of "my name is Ana".
/// - recall "sister": 4 hits out of 5 matches (pin, candidate, transcript,
///   ring, summary). The pin and the session-ring candidate are both in the
///   window. The summary also contains "sister", so it is a second diary hit
///   when the search window includes it.
///
/// After dedupe the candidate rows stay in the file. Recall keeps the diary
/// row and drops the session-ring twin.
#[test]
fn measured_session_counts() {
    let dir = scratch("measured");
    let path = dir.join("person-memory.sqlite");
    let mut store = EpisodicStore::open(&path, true);
    store
        .note_state(PERSON, &named("Ana", "she"), "t000001")
        .ok_stored();
    store
        .note_turn(PERSON, PERSON, "my name is Ana", "t000002")
        .ok_stored();
    let line = "remember this my sister is Mira";
    let pinned = pin_command(line).expect("pin text");
    store.pin(PERSON, pinned, "t000003").ok_stored();
    store.note_turn(PERSON, PERSON, line, "t000003").ok_stored();
    store
        .note_turn(PERSON, PERSON, "I live in Split", "t000004")
        .ok_stored();
    store.end_session(PERSON, "t000099").ok_stored();
    assert_eq!(store.writes(), 4, "diary items");
    let name = store.recall(PERSON, "what is my name");
    assert_eq!(name.len(), 1, "{name:?}");
    assert_eq!(name[0].source, "state-tracker");
    assert_eq!(name[0].kind, MemoryKind::Name);
    let sister = store.recall(PERSON, "sister");
    assert!(
        sister.iter().all(|hit| hit.source != "session-ring"),
        "{sister:?}"
    );
    assert_eq!(
        sister.iter().filter(|hit| hit.source == "pin").count(),
        1,
        "{sister:?}"
    );
    assert!(
        sister
            .iter()
            .all(|hit| hit.source == "pin" || hit.source == "session-summary"),
        "{sister:?}"
    );
    drop(store);
    let saved = EpisodicStore::read_transcript_file(&path, "Ana").expect("reopen");
    assert_eq!(saved.extracted, 3, "candidates stay stored");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn name_is_one_hit() {
    let dir = scratch("name");
    let path = dir.join("person-memory.sqlite");
    let mut store = EpisodicStore::open(&path, true);
    store
        .note_state(PERSON, &named("Ana", "she"), "t000001")
        .ok_stored();
    store
        .note_turn(PERSON, PERSON, "my name is Ana", "t000002")
        .ok_stored();
    store.end_session(PERSON, "t000099").ok_stored();
    let hits = store.recall(PERSON, "what is my name");
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert_eq!(hits[0].kind, MemoryKind::Name);
    assert_eq!(hits[0].source, "state-tracker");
    assert!(hits[0].text.contains("Ana"), "{}", hits[0].text);
    drop(store);
    let saved = EpisodicStore::read_transcript_file(&path, "Ana").expect("reopen");
    assert!(
        saved.extracted >= 1,
        "the name candidate still exists: {}",
        saved.extracted
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn remember_this_is_one_hit() {
    let dir = scratch("sister");
    let path = dir.join("person-memory.sqlite");
    let mut store = EpisodicStore::open(&path, true);
    let line = "remember this my sister is Mira";
    let pinned = pin_command(line).expect("pin text");
    store.pin(PERSON, pinned, "t000001").ok_stored();
    store.note_turn(PERSON, PERSON, line, "t000001").ok_stored();
    store.end_session(PERSON, "t000099").ok_stored();
    let hits = store.recall(PERSON, "sister");
    assert!(
        hits.iter().all(|hit| hit.source != "session-ring"),
        "{hits:?}"
    );
    let pins: Vec<_> = hits.iter().filter(|hit| hit.source == "pin").collect();
    assert_eq!(pins.len(), 1, "{hits:?}");
    assert!(pins[0].pinned);
    assert_eq!(pins[0].source, "pin");
    assert_eq!(pins[0].text, "my sister is Mira");
    assert!(
        hits.iter()
            .all(|hit| hit.source == "pin" || hit.source == "session-summary"),
        "{hits:?}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn extractor_only_facts_stay() {
    let dir = scratch("split");
    let path = dir.join("person-memory.sqlite");
    let mut store = EpisodicStore::open(&path, true);
    store
        .note_turn(PERSON, PERSON, "I live in Split", "t000001")
        .ok_stored();
    store.end_session(PERSON, "t000099").ok_stored();
    let hits = store.recall(PERSON, "Split");
    let kept: Vec<_> = hits
        .iter()
        .filter(|hit| hit.source == "session-ring" && hit.text.contains("Split"))
        .collect();
    assert!(!kept.is_empty(), "{hits:?}");
    assert!(
        kept.iter()
            .all(|hit| hit.kind != MemoryKind::Name && !hit.pinned && hit.kind == MemoryKind::Pin),
        "{kept:?}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn forget_hides_the_twin() {
    let dir = scratch("forget-name");
    let path = dir.join("person-memory.sqlite");
    {
        let mut store = EpisodicStore::open(&path, true);
        store
            .note_state(PERSON, &named("Ana", "she"), "t000001")
            .ok_stored();
        store
            .note_turn(PERSON, PERSON, "my name is Ana", "t000002")
            .ok_stored();
        store.end_session(PERSON, "t000099").ok_stored();
    }
    let before = EpisodicStore::read_transcript_file(&path, "Ana").expect("before");
    assert!(before.extracted >= 1, "{}", before.extracted);
    let mut store = EpisodicStore::open(&path, true);
    let hits = store.recall(PERSON, "what is my name");
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert_eq!(hits[0].source, "state-tracker");
    let id = hits[0].id.clone();
    let forgotten = store.forget(PERSON, &id);
    assert_eq!(forgotten, WriteOutcome::Archived { id: id.clone() });
    let stone = store.tombstone(&id).expect("tombstone");
    assert_eq!(stone.reason, "forget");
    assert_eq!(stone.person_id, PERSON);
    assert!(
        store.recall(PERSON, "name").is_empty(),
        "forgotten name is still recalled"
    );
    drop(store);
    let after = EpisodicStore::read_transcript_file(&path, "Ana").expect("after");
    assert!(
        after.extracted < before.extracted,
        "candidate was not archived: before {} after {}",
        before.extracted,
        after.extracted
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn distinct_names_both_survive() {
    let dir = scratch("two-names");
    let path = dir.join("person-memory.sqlite");
    let mut store = EpisodicStore::open(&path, true);
    store
        .note_state(PERSON, &named("Ana", "she"), "t000001")
        .ok_stored();
    store
        .note_turn(PERSON, PERSON, "my name is Bea", "t000002")
        .ok_stored();
    store.end_session(PERSON, "t000099").ok_stored();
    let hits = store.recall(PERSON, "name");
    assert!(
        hits.iter().any(|hit| {
            hit.source == "state-tracker"
                && hit.text.contains("Ana")
                && hit.kind == MemoryKind::Name
        }),
        "{hits:?}"
    );
    assert!(
        hits.iter().any(|hit| {
            hit.source == "session-ring"
                && hit.text.contains("Bea")
                && hit.kind == MemoryKind::Name
                && !hit.pinned
        }),
        "{hits:?}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn denied_writes_nothing() {
    let dir = scratch("denied");
    let path = dir.join("person-memory.sqlite");
    let mut store = EpisodicStore::open(&path, false);
    assert_eq!(
        store.note_turn(PERSON, PERSON, "my name is Ana", "t000001"),
        WriteOutcome::Refused
    );
    assert_eq!(
        store.pin(PERSON, "my sister is Mira", "t000001"),
        WriteOutcome::Refused
    );
    assert_eq!(
        store.note_state(PERSON, &named("Ana", "she"), "t000001"),
        WriteOutcome::Refused
    );
    assert_eq!(store.end_session(PERSON, "t000099"), WriteOutcome::Refused);
    assert_eq!(store.forget(PERSON, "cand-1"), WriteOutcome::Refused);
    assert!(store.recall(PERSON, "name").is_empty());
    assert_eq!(store.writes(), 0);
    assert_eq!(store.buffered_turns(), 0);
    assert!(!path.exists());
    assert!(sqlite_names(&dir).is_empty(), "{:?}", sqlite_names(&dir));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn dedupe_happens_before_the_cap() {
    let dir = scratch("cap");
    let path = dir.join("person-memory.sqlite");
    let mut store = EpisodicStore::open(&path, true);
    let extra = MEMORY_HIT_CAP + 2;
    for index in 0..extra {
        let fact = format!("ledgerkey token{index}");
        store
            .pin(PERSON, &fact, &format!("p{index:06}"))
            .ok_stored();
        store
            .note_turn(
                PERSON,
                PERSON,
                &format!("remember this {fact}"),
                &format!("n{index:06}"),
            )
            .ok_stored();
    }
    store.end_session(PERSON, "t000099").ok_stored();
    // "token0" matches one pin, its candidate, the transcript, the ring, and
    // the summary. The candidate outranks the summary, so it sits in the
    // search window. Dedupe removes it before this cap, and the pin stays.
    let hits = store.recall(PERSON, "token0");
    assert!(hits.len() <= MEMORY_HIT_CAP, "{}", hits.len());
    assert!(
        hits.iter().all(|hit| hit.source != "session-ring"),
        "a twin still occupies a capped slot: {hits:?}"
    );
    assert!(
        hits.iter()
            .any(|hit| hit.source == "pin" && hit.text == "ledgerkey token0"),
        "{hits:?}"
    );
    drop(store);
    let saved = EpisodicStore::read_transcript_file(&path, "ledgerkey").expect("reopen");
    assert!(
        saved.extracted >= extra,
        "mixed hits in the file: extracted {} cap {MEMORY_HIT_CAP}",
        saved.extracted
    );
    let _ = fs::remove_dir_all(&dir);
}
