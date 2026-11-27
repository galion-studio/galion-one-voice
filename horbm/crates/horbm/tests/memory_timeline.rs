// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Relationship timeline. Two granted sessions and one milestone, oldest
//! date first. A world question does not speak them. Denied remember writes
//! nothing. The same milestone text is not inserted again.

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use horbm::{EpisodicStore, WriteOutcome, SOURCE_MILESTONE, UNKNOWN_REPLY};

const PERSON: &str = "northwind";
const MILESTONE: &str = "the oak we planted";

fn scratch(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "horbm-f7-timeline-{name}-{}-{nanos}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).expect("scratch");
    dir
}

struct EmptyCards;

impl horbm::CardLookup for EmptyCards {
    fn lookup(&self, _query: &str) -> Option<horbm::CardHit> {
        None
    }
}

fn expect_stored(outcome: WriteOutcome) -> String {
    match outcome {
        WriteOutcome::Stored { id } => id,
        other => panic!("not stored: {other:?}"),
    }
}

fn in_order(reply: &str, parts: &[&str]) {
    let mut from = 0;
    for part in parts {
        let found = reply[from..]
            .find(part)
            .unwrap_or_else(|| panic!("{part} missing after {from} in {reply}"));
        from += found + part.len();
    }
}

#[test]
fn relationship_timeline_lists_summaries_and_milestones_in_date_order() {
    let dir = scratch("order");
    let path = dir.join("person-memory.sqlite");
    let mut brain =
        horbm::Brain::with_cards(EmptyCards, None).with_memory(EpisodicStore::open(&path, true));
    brain.set_person(PERSON);

    {
        let memory = brain.person_memory().expect("diary");
        expect_stored(memory.pin(PERSON, "my favourite colour is blue", "2026-01-01"));
        expect_stored(memory.note_turn(PERSON, PERSON, "I prefer cedar tea", "2026-01-02"));
        expect_stored(memory.end_session(PERSON, "2026-01-02"));
        let first = expect_stored(memory.milestone(PERSON, MILESTONE, "2026-02-14"));
        let second =
            expect_stored(memory.milestone(PERSON, "  The oak we planted. ", "2026-02-20"));
        assert_eq!(first, second, "a repeat milestone must keep the first row");
        expect_stored(memory.note_turn(PERSON, PERSON, "I prefer river walks", "2026-03-04"));
        expect_stored(memory.end_session(PERSON, "2026-03-04"));

        let mut rows = memory.list_person(PERSON, usize::MAX);
        rows.retain(|item| item.source == "session-summary" || item.source == SOURCE_MILESTONE);
        rows.sort_by(|left, right| left.at.cmp(&right.at).then_with(|| left.id.cmp(&right.id)));
        assert_eq!(rows.len(), 3, "{rows:?}");
        assert_eq!(rows[0].source, "session-summary");
        assert_eq!(rows[0].at, "2026-01-02");
        assert!(
            rows[0].text.contains("cedar tea"),
            "first summary: {}",
            rows[0].text
        );
        assert_eq!(rows[1].source, SOURCE_MILESTONE);
        assert_eq!(rows[1].text, MILESTONE);
        assert_eq!(rows[1].at, "2026-02-14");
        assert_eq!(rows[1].id, first);
        assert_eq!(rows[2].source, "session-summary");
        assert_eq!(rows[2].at, "2026-03-04");
        assert!(
            rows[2].text.contains("river walks"),
            "second summary: {}",
            rows[2].text
        );
        assert!(
            rows.iter()
                .all(|item| item.text != "my favourite colour is blue"),
            "a pin is not a timeline row: {rows:?}"
        );
    }

    let parts = ["cedar tea", MILESTONE, "river walks"];
    let through = brain.turn("what have we been through");
    in_order(&through.reply, &parts);
    assert!(
        !through.reply.contains("favourite colour"),
        "ordinary pin spoken on the timeline: {}",
        through.reply
    );
    assert!(
        through
            .sources
            .iter()
            .any(|source| source.starts_with("memory:")),
        "{:?}",
        through.sources
    );
    let timeline = brain.turn("our timeline");
    in_order(&timeline.reply, &parts);
    assert!(
        !timeline.reply.contains("favourite colour"),
        "{}",
        timeline.reply
    );

    let kettle = brain.turn("where is the kettle");
    assert_eq!(kettle.reply, UNKNOWN_REPLY);
    assert!(!kettle.reply.contains(MILESTONE));
    assert!(!kettle.reply.contains("cedar tea"));
    assert!(!kettle.reply.contains("river walks"));
    assert!(kettle
        .sources
        .iter()
        .all(|source| !source.starts_with("memory:")));
    let capital = brain.turn("what is the capital of france");
    assert_eq!(capital.reply, UNKNOWN_REPLY);
    assert!(!capital.reply.contains(MILESTONE));
    assert!(capital
        .sources
        .iter()
        .all(|source| !source.starts_with("memory:")));

    let colour = brain.turn("what is my favourite colour");
    assert_eq!(colour.reply, "my favourite colour is blue");

    brain.turn("milestone the oak we planted");
    brain.turn("please remember this moment the oak we planted");
    let milestones: Vec<_> = brain
        .person_memory()
        .expect("diary")
        .list_person(PERSON, usize::MAX)
        .into_iter()
        .filter(|item| item.source == SOURCE_MILESTONE)
        .collect();
    assert_eq!(milestones.len(), 1, "{milestones:?}");
    assert_eq!(milestones[0].text, MILESTONE);
    let _ = fs::remove_dir_all(&dir);

    let denied_dir = scratch("denied");
    let denied_path = denied_dir.join("person-memory.sqlite");
    let mut denied = horbm::Brain::with_cards(EmptyCards, None)
        .with_memory(EpisodicStore::open(&denied_path, false));
    denied.set_person(PERSON);
    assert_eq!(
        denied
            .person_memory()
            .expect("diary")
            .milestone(PERSON, MILESTONE, "2026-02-14"),
        WriteOutcome::Refused
    );
    denied.turn("milestone the oak we planted");
    denied.turn("please remember this moment the oak we planted");
    assert!(!denied_path.exists(), "denied remember created a diary");
    assert!(denied
        .person_memory()
        .expect("diary")
        .list_person(PERSON, 8)
        .is_empty());
    let spoken = denied.turn("what have we been through");
    assert_eq!(spoken.reply, UNKNOWN_REPLY);
    assert!(!spoken.reply.contains(MILESTONE));
    let _ = fs::remove_dir_all(&denied_dir);
}
