// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Old unpinned chatter leaves live recall and stays in the archive.
//!
//! Owner decision 27 Nov 2026: 180 civil days. A row dated 179 days before
//! the session clock stays. So does a row dated exactly 180 days. Pins,
//! session summaries, milestones, and rows with no civil date stay. Denied
//! remember writes no archive row.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use horbm::{
    EpisodicStore, PersonMemory, PersonState, TrackedName, WriteOutcome, SOURCE_MILESTONE,
};
use rusqlite::{params, Connection};

const PERSON: &str = "northwind";
const OTHER: &str = "person-beta";
/// 181 civil days before [`TODAY`].
const OLD: &str = "2026-05-30T08:00:00Z";
/// Exactly 180 civil days before [`TODAY`].
const BOUND: &str = "2026-05-31T08:00:00Z";
/// 179 civil days before [`TODAY`].
const YOUNG: &str = "2026-06-01T08:00:00Z";
const TODAY: &str = "2026-11-27T08:00:00Z";
const OLD_CHATTER: &str = "I prefer dayline181";
const BOUND_CHATTER: &str = "I prefer dayline180";
const UNDATED_CHATTER: &str = "I prefer nodate";
const YOUNG_FEELING: &str = "brisk";
const PIN_TEXT: &str = "my sister is Ana";
const MILESTONE_TEXT: &str = "the ridgecrest milestone held";
const BLANK_TEXT: &str = "blankdate token stays";
const OTHER_FEELING: &str = "guestmood";

fn scratch(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "horbm-f7-fade-{name}-{}-{nanos}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).expect("scratch");
    dir
}

fn expect_stored(outcome: &WriteOutcome) {
    assert!(
        matches!(outcome, WriteOutcome::Stored { .. }),
        "{outcome:?}"
    );
}

fn named(name: &str) -> PersonState {
    PersonState {
        names: vec![TrackedName {
            name: name.to_string(),
            class_label: "they".to_string(),
        }],
        arousal: 0.3,
        valence: 0.25,
        dominance: 0.5,
        feeling: None,
    }
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

fn retouch(path: &Path, body: &str, source: &str, date: &str, pinned: i64) {
    let conn = Connection::open(path).expect("open diary");
    let changed = conn
        .execute(
            "UPDATE items SET source = ?1, pinned = ?2, source_date = ?3 WHERE body = ?4",
            params![source, pinned, date, body],
        )
        .expect("retouch");
    assert_eq!(changed, 1, "expected one row for {body}");
}

fn live_texts(store: &mut EpisodicStore, person: &str) -> Vec<String> {
    store
        .list_person(person, 64)
        .into_iter()
        .map(|item| item.text)
        .collect()
}

#[test]
fn an_old_unpinned_chatter_row_leaves_live_recall_and_stays_in_the_archive() {
    let dir = scratch("fade");
    let path = dir.join("person-memory.sqlite");
    {
        let mut store = EpisodicStore::open(&path, true);
        expect_stored(&store.pin(PERSON, PIN_TEXT, OLD));
        expect_stored(&store.milestone(PERSON, MILESTONE_TEXT, OLD));
        expect_stored(&store.pin(PERSON, BLANK_TEXT, OLD));
        expect_stored(&store.note_state(PERSON, &named("Oldfern"), OLD));
        expect_stored(&store.note_state(PERSON, &mood_only(YOUNG_FEELING), YOUNG));
        expect_stored(&store.note_state(OTHER, &mood_only(OTHER_FEELING), OLD));
        expect_stored(&store.note_turn(PERSON, PERSON, OLD_CHATTER, OLD));
        expect_stored(&store.note_turn(PERSON, PERSON, BOUND_CHATTER, BOUND));
        expect_stored(&store.note_turn(PERSON, PERSON, UNDATED_CHATTER, "undated"));
        expect_stored(&store.end_session(PERSON, OLD));
    }
    retouch(&path, MILESTONE_TEXT, SOURCE_MILESTONE, OLD, 0);
    retouch(&path, BLANK_TEXT, "state-tracker", "", 0);

    let mut store = EpisodicStore::open(&path, true);
    expect_stored(&store.note_turn(PERSON, PERSON, "I prefer todaytoken", TODAY));
    expect_stored(&store.end_session(PERSON, TODAY));

    let live = live_texts(&mut store, PERSON);
    assert!(
        live.iter().all(|text| text != OLD_CHATTER),
        "old session-ring still live: {live:?}"
    );
    assert!(
        live.iter().all(|text| !text.contains("Oldfern")),
        "old state-tracker still live: {live:?}"
    );
    assert!(
        live.iter().any(|text| text.contains(YOUNG_FEELING)),
        "179-day row left live recall: {live:?}"
    );
    assert!(
        live.iter().any(|text| text == BOUND_CHATTER),
        "180-day row left live recall: {live:?}"
    );
    assert!(
        live.iter().any(|text| text == UNDATED_CHATTER),
        "undated chatter left live recall: {live:?}"
    );
    assert!(
        live.iter().any(|text| text == PIN_TEXT),
        "pin left live recall: {live:?}"
    );
    assert!(
        live.iter().any(|text| text == MILESTONE_TEXT),
        "milestone left live recall: {live:?}"
    );
    assert!(
        live.iter().any(|text| text == BLANK_TEXT),
        "row without a date left live recall: {live:?}"
    );
    assert!(
        live.iter()
            .any(|text| text.contains("Session ") && text.contains(OLD)),
        "old session summary left live recall: {live:?}"
    );

    let recalled = store.recall(PERSON, "dayline181");
    assert!(
        recalled.iter().all(|hit| hit.text != OLD_CHATTER),
        "old chatter still recalled: {recalled:?}"
    );
    assert!(
        store
            .recall(PERSON, "what is my name")
            .iter()
            .all(|hit| !hit.text.contains("Oldfern")),
        "faded name still recalled"
    );
    assert!(
        store
            .recall(PERSON, "feeling brisk")
            .iter()
            .any(|hit| hit.source == "state-tracker" && hit.text.contains(YOUNG_FEELING)),
        "179-day mood was not recalled"
    );
    assert_eq!(store.recall(PERSON, "sister Ana").len(), 1);
    assert!(
        store
            .recall(PERSON, "ridgecrest")
            .iter()
            .any(|hit| hit.source == SOURCE_MILESTONE && hit.text == MILESTONE_TEXT),
        "milestone was not recalled"
    );

    let other = live_texts(&mut store, OTHER);
    assert!(
        other.iter().any(|text| text.contains(OTHER_FEELING)),
        "another person's old row was faded: {other:?}"
    );

    drop(store);
    let conn = EpisodicStore::open_readonly(&path).expect("reopen");
    let archive = EpisodicStore::read_archive(&conn, Some(PERSON)).expect("archive");
    let faded: Vec<_> = archive.iter().filter(|row| row.reason == "fade").collect();
    assert!(
        faded
            .iter()
            .any(|row| row.text == OLD_CHATTER && row.source == "session-ring"),
        "old session-ring missing from the fade archive: {archive:?}"
    );
    assert!(
        faded
            .iter()
            .any(|row| row.text.contains("Oldfern") && row.source == "state-tracker"),
        "old state-tracker missing from the fade archive: {archive:?}"
    );
    assert!(
        archive.iter().all(|row| {
            row.text != BOUND_CHATTER
                && row.text != UNDATED_CHATTER
                && row.text != PIN_TEXT
                && row.text != MILESTONE_TEXT
                && row.text != BLANK_TEXT
                && !row.text.contains(YOUNG_FEELING)
                && row.source != "session-summary"
        }),
        "a kept row was archived: {archive:?}"
    );
    let mut kept = EpisodicStore::open(&path, true);
    let _ = kept.recall(PERSON, "dayline181");
    assert!(kept.holds_text(OLD_CHATTER));
    assert!(kept.holds_text("Oldfern"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn denied_remember_writes_no_fade_archive() {
    let dir = scratch("denied");
    let path = dir.join("person-memory.sqlite");
    let mut denied = EpisodicStore::open(&path, false);
    assert_eq!(
        denied.note_turn(PERSON, PERSON, OLD_CHATTER, OLD),
        WriteOutcome::Refused
    );
    assert_eq!(denied.end_session(PERSON, TODAY), WriteOutcome::Refused);
    assert!(!path.exists());
    drop(denied);

    {
        let mut store = EpisodicStore::open(&path, true);
        expect_stored(&store.note_state(PERSON, &mood_only(YOUNG_FEELING), OLD));
        expect_stored(&store.note_turn(PERSON, PERSON, OLD_CHATTER, OLD));
        expect_stored(&store.end_session(PERSON, OLD));
    }
    let mut closed = EpisodicStore::open(&path, true);
    closed.set_remember(false);
    assert_eq!(
        closed.note_turn(PERSON, PERSON, "I prefer todaytoken", TODAY),
        WriteOutcome::Refused
    );
    assert_eq!(closed.end_session(PERSON, TODAY), WriteOutcome::Refused);
    drop(closed);

    let conn = EpisodicStore::open_readonly(&path).expect("reopen");
    let archive = EpisodicStore::read_archive(&conn, Some(PERSON)).expect("archive");
    assert!(
        archive.iter().all(|row| row.reason != "fade"),
        "denied session wrote a fade tombstone: {archive:?}"
    );
    let mut store = EpisodicStore::open(&path, true);
    let live = live_texts(&mut store, PERSON);
    assert!(
        live.iter().any(|text| text.contains(YOUNG_FEELING)),
        "denied session dropped the old row: {live:?}"
    );
    let _ = fs::remove_dir_all(&dir);
}
