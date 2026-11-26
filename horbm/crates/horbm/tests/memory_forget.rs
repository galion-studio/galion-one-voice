// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Phrase forget. A voice turn has words, not an item id.
//!
//! `forget_matching` is the `PersonMemory` default: recall, then archive each
//! hit. A candidate twin already archived with its pin comes back `Skipped`
//! and is not counted. Nothing here is deleted.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use horbm::{EpisodicStore, PersonMemory, WriteOutcome};

const PERSON: &str = "northwind";
const OTHER: &str = "person-beta";

fn scratch(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "horbm-f7-forget-{name}-{}-{nanos}",
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

/// Pin the phrase and note the same turn. Ending the session flushes that
/// turn, which is what stores the session-ring twin of the pin.
#[test]
fn forget_matching_hides_pin_and_its_candidate_twin() {
    let dir = scratch("sister");
    let path = dir.join("person-memory.sqlite");
    {
        let mut store = EpisodicStore::open(&path, true);
        let text = "my sister is Ana";
        expect_stored(&store.pin(PERSON, text, "t000001"));
        expect_stored(&store.note_turn(PERSON, PERSON, text, "t000001"));
        expect_stored(&store.end_session(PERSON, "t000099"));
    }
    let before = EpisodicStore::read_transcript_file(&path, "Ana").expect("before");
    assert!(
        before.extracted >= 1,
        "the noted turn did not store a candidate twin"
    );
    let mut store = EpisodicStore::open(&path, true);
    let live = store.recall(PERSON, "sister");
    assert!(
        live.iter()
            .any(|hit| hit.source == "pin" && hit.text == "my sister is Ana"),
        "{live:?}"
    );
    let archived = store.forget_matching(PERSON, "sister");
    assert!(!archived.is_empty(), "{archived:?}");
    assert!(
        archived.iter().any(|id| {
            store
                .tombstone(id)
                .is_some_and(|stone| stone.reason == "forget" && stone.person_id == PERSON)
        }),
        "{archived:?}"
    );
    let gone = store.recall(PERSON, "sister");
    assert!(gone.is_empty(), "{gone:?}");
    drop(store);
    let after = EpisodicStore::read_transcript_file(&path, "Ana").expect("after");
    assert!(
        after.extracted < before.extracted,
        "candidate twin was not archived: before {} after {}",
        before.extracted,
        after.extracted
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn no_hits_returns_empty_and_writes_nothing() {
    let dir = scratch("miss");
    let path = dir.join("person-memory.sqlite");
    let mut store = EpisodicStore::open(&path, true);
    expect_stored(&store.pin(PERSON, "my favourite colour is blue", "t000001"));
    let hits = store.recall(PERSON, "colour");
    assert_eq!(hits.len(), 1, "{hits:?}");
    let id = hits[0].id.clone();
    let writes = store.writes();
    let archived = store.forget_matching(PERSON, "sister");
    assert!(archived.is_empty(), "{archived:?}");
    assert_eq!(store.writes(), writes);
    assert!(store.tombstone(&id).is_none());
    let still = store.recall(PERSON, "colour");
    assert_eq!(still.len(), 1, "{still:?}");
    assert_eq!(still[0].id, id);
    assert_eq!(still[0].text, "my favourite colour is blue");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn denied_returns_empty() {
    let dir = scratch("denied");
    let path = dir.join("person-memory.sqlite");
    let mut store = EpisodicStore::open(&path, false);
    assert!(!store.remember_granted());
    assert_eq!(
        store.pin(PERSON, "my sister is Ana", "t000001"),
        WriteOutcome::Refused
    );
    assert!(store.forget_matching(PERSON, "sister").is_empty());
    assert!(store.recall(PERSON, "sister").is_empty());
    assert!(!path.exists());
    assert!(sqlite_names(&dir).is_empty(), "{:?}", sqlite_names(&dir));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn other_person_rows_untouched() {
    let dir = scratch("other");
    let path = dir.join("person-memory.sqlite");
    let mut store = EpisodicStore::open(&path, true);
    expect_stored(&store.pin(PERSON, "my sister is Ana", "t000001"));
    expect_stored(&store.pin(OTHER, "my sister is Bea", "t000002"));
    let beta = store.recall(OTHER, "sister");
    assert_eq!(beta.len(), 1, "{beta:?}");
    let beta_id = beta[0].id.clone();
    assert_eq!(beta[0].text, "my sister is Bea");
    let archived = store.forget_matching(PERSON, "sister");
    assert!(!archived.is_empty(), "{archived:?}");
    assert!(
        !archived.contains(&beta_id),
        "another person's row was archived: {archived:?}"
    );
    assert!(store.recall(PERSON, "sister").is_empty());
    let still = store.recall(OTHER, "sister");
    assert_eq!(still.len(), 1, "{still:?}");
    assert_eq!(still[0].id, beta_id);
    assert_eq!(still[0].text, "my sister is Bea");
    assert!(store.tombstone(&beta_id).is_none());
    let _ = fs::remove_dir_all(&dir);
}
