// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

use std::time::{Duration, Instant};

use horbm_memory::{Stack, Store, RING_CAP};

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("horbm-a-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn the_ring_drops_a_turn_the_transcript_keeps() {
    let mut store = Store::open_in_memory().unwrap();
    for i in 0..RING_CAP + 1 {
        store.push_turn(format!("turn {i} token ringword{i}"), "Ada", "2026-11-18");
    }
    assert_eq!(store.session_len(), RING_CAP);
    let early = store.search("ringword0").unwrap();
    assert!(
        early.hits.iter().all(|hit| !hit.text.contains("ringword0")),
        "dropped turn still in the ring: {:?}",
        early.hits
    );
    store.drain().unwrap();
    assert_eq!(store.transcript_len().unwrap(), RING_CAP + 1);
    let kept = store.search_transcript("ringword0").unwrap();
    assert!(!kept.hits.is_empty());
    assert_eq!(kept.hits[0].said_by, "Ada");
    assert_eq!(kept.hits[0].source, "session-ring");
    assert_eq!(kept.hits[0].layer, "transcript");
}

#[test]
fn a_pushed_turn_is_in_the_transcript_after_reopen() {
    let dir = scratch("reopen");
    let path = dir.join("memory.sqlite");
    {
        let mut store = Store::open(&path).unwrap();
        store.push_turn(
            "keep the word zephyrlane in the transcript",
            "Ada",
            "2026-11-18",
        );
        store.drain().unwrap();
    }
    let store = Store::open(&path).unwrap();
    assert_eq!(store.session_len(), 0);
    let hits = store.search_transcript("zephyrlane").unwrap();
    assert!(!hits.hits.is_empty());
    assert_eq!(hits.hits[0].said_by, "Ada");
    assert_eq!(hits.hits[0].date, "2026-11-18");
    assert_eq!(hits.hits[0].source, "session-ring");
    drop(store);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_pin_ranks_above_the_same_candidate() {
    let mut store = Store::open_in_memory().unwrap();
    store.push_turn("remember this: I prefer oolong quartz", "Ada", "2026-11-18");
    store.drain().unwrap();
    let pin = store
        .remember(
            "I prefer oolong quartz",
            "Ada",
            "2026-11-18",
            "conversation",
        )
        .unwrap();
    store.drain().unwrap();
    let hits = store.search("oolong").unwrap();
    let pin_hit = hits
        .hits
        .iter()
        .find(|hit| hit.item_id == pin)
        .expect("pin");
    assert!(pin_hit.pinned);
    assert!((pin_hit.importance - 1.0).abs() < 0.001);
    assert_eq!(pin_hit.said_by, "Ada");
    assert_eq!(pin_hit.date, "2026-11-18");
    assert_eq!(pin_hit.source, "conversation");
    assert_eq!(pin_hit.extractor, "user");
    let candidate = hits
        .hits
        .iter()
        .find(|hit| hit.extractor == "system")
        .expect("candidate");
    assert!(candidate.importance > 0.0);
    assert!(!candidate.pinned);
    assert!(pin_hit.score > candidate.score, "{hits:?}");
}

#[test]
fn extraction_runs_on_the_queue_and_skips_a_joke_and_a_secret() {
    let mut store = Store::open_in_memory().unwrap();
    store.pause_worker();
    store.push_turn("remember this: I prefer quartz tea", "Ada", "2026-11-18");
    let mid = store.search("quartz").unwrap();
    assert!(mid.hits.iter().all(|hit| hit.extractor != "system"));
    assert!(mid.hits.iter().any(|hit| hit.layer == "session"));
    assert!(store.queue_len() >= 2);
    store.resume_worker();
    store.drain().unwrap();
    let done = store.search("quartz").unwrap();
    assert!(done.hits.iter().any(|hit| {
        hit.extractor == "system" && hit.importance > 0.0 && hit.kind == "candidate"
    }));

    store.push_turn("haha quokkajoke was just a joke", "Ada", "2026-11-18");
    store.push_turn("my password is hunter2 please", "Ada", "2026-11-18");
    store.drain().unwrap();
    assert!(store
        .search("quokkajoke")
        .unwrap()
        .hits
        .iter()
        .all(|hit| hit.extractor != "system"));
    assert_eq!(store.count_extracted().unwrap(), 1);
    assert!(!store
        .search_transcript("quokkajoke")
        .unwrap()
        .hits
        .is_empty());
    assert!(store.retains_text("hunter2").unwrap());
    assert!(store
        .search("hunter2")
        .unwrap()
        .hits
        .iter()
        .all(|hit| hit.extractor != "system"));
}

#[test]
fn keep_whole_stores_a_summary_and_off_does_not_promote_the_session() {
    let mut quiet = Store::open_in_memory().unwrap();
    for i in 0..=RING_CAP {
        quiet.push_turn(
            format!("chatter {i} token oldword{i} is nothing"),
            "Ada",
            "2026-11-18",
        );
    }
    quiet.push_turn("I prefer saffron ink", "Ada", "2026-11-18");
    quiet.keep_whole_session("default", false).unwrap();
    quiet.drain().unwrap();
    assert!(quiet.session_summary("default").unwrap().is_empty());
    assert!(quiet
        .search("oldword0")
        .unwrap()
        .hits
        .iter()
        .all(|hit| !hit.text.contains("oldword0")));
    assert!(!quiet.search_transcript("oldword0").unwrap().hits.is_empty());
    assert!(quiet
        .search("saffron")
        .unwrap()
        .hits
        .iter()
        .any(|hit| { hit.extractor == "system" && hit.kind == "candidate" }));

    let mut kept = Store::open_in_memory().unwrap();
    kept.push_turn("I prefer celadon tea", "Ada", "2026-11-18");
    kept.push_turn("haha quokkajoke was just a joke", "Ada", "2026-11-18");
    kept.keep_whole_session("default", true).unwrap();
    kept.drain().unwrap();
    let summary = kept.session_summary("default").unwrap();
    assert!(summary.to_lowercase().contains("celadon"), "{summary}");
    assert!(
        summary.to_lowercase().contains("prefer") || summary.contains("likes"),
        "{summary}"
    );
    assert!(!summary.to_lowercase().contains("quokkajoke"), "{summary}");
    assert!(!summary.contains("haha"), "{summary}");
    assert!(!kept
        .search_transcript("quokkajoke")
        .unwrap()
        .hits
        .is_empty());
    assert!(kept
        .search("quokkajoke")
        .unwrap()
        .hits
        .iter()
        .any(|hit| { hit.layer == "transcript" || hit.kind == "transcript" }));
}

#[test]
fn a_revision_archives_the_old_fact_and_keeps_the_text() {
    let mut store = Store::open_in_memory().unwrap();
    let old = store
        .remember(
            "the codename is silverpine",
            "Ada",
            "2026-11-18",
            "conversation",
        )
        .unwrap();
    let new_id = store
        .revise_fact(
            &old,
            "the codename is bluecedar",
            "Grace",
            "2026-11-19",
            "correction",
        )
        .unwrap();
    store.drain().unwrap();
    assert!(store
        .search("silverpine")
        .unwrap()
        .hits
        .iter()
        .all(|hit| hit.item_id != old));
    assert!(store
        .search("bluecedar")
        .unwrap()
        .hits
        .iter()
        .any(|hit| hit.item_id == new_id));
    let archived = store.archive_list().unwrap();
    let row = archived
        .iter()
        .find(|row| row.item_id == old)
        .expect("archive");
    assert_eq!(row.reason, "revised");
    assert!(row.text.contains("silverpine"));
    assert_eq!(row.said_by, "Ada");
    assert_eq!(row.date, "2026-11-18");
    assert_eq!(row.source, "conversation");
    let revisions = store.memory_revisions(&old).unwrap();
    assert!(revisions.iter().any(|row| {
        row.reason == "revised"
            && row.text.contains("silverpine")
            && row.said_by == "Ada"
            && row.date == "2026-11-18"
            && row.source == "conversation"
    }));
    assert!(store.retains_text("silverpine").unwrap());
}

#[test]
fn a_later_name_archives_the_earlier_one() {
    let mut store = Store::open_in_memory().unwrap();
    store.push_turn("my name is Ada Lovelace", "Ada", "2026-11-18");
    store.drain().unwrap();
    store.push_turn("my name is Grace Hopper", "Ada", "2026-11-18");
    store.drain().unwrap();
    let live = store.search("Grace").unwrap();
    assert!(live
        .hits
        .iter()
        .any(|hit| { hit.extractor == "system" && hit.text.to_lowercase().contains("grace") }));
    assert!(store
        .search("Lovelace")
        .unwrap()
        .hits
        .iter()
        .filter(|hit| hit.layer == "episode")
        .all(|hit| !hit.text.to_lowercase().contains("lovelace")));
    assert!(store.archive_list().unwrap().iter().any(|row| {
        row.reason == "revised" && row.text.to_lowercase().contains("ada lovelace")
    }));
    assert!(store.retains_text("Ada Lovelace").unwrap());
}

#[test]
fn query_enqueues_the_gap_and_does_not_wait_on_a_paused_worker() {
    let dir = scratch("pause");
    let mut stack = Stack::open(dir.join("memory.sqlite"), dir.join("knowledge")).unwrap();
    stack.pause_worker();
    let started = Instant::now();
    stack.push_turn("a live turn about marigold", "Ada", "2026-11-18");
    let missed = stack.query("zzznothingmatches").unwrap();
    assert!(started.elapsed() < Duration::from_millis(1_000));
    assert_eq!(missed.answer, "I don't know");
    assert!(missed.gap_id.is_none());
    assert!(stack.queue_len() >= 2);
    assert!(stack.gap_queue().unwrap().is_empty());
    stack.resume_worker();
    stack.drain().unwrap();
    let queue = stack.gap_queue().unwrap();
    assert_eq!(queue[0].query, "zzznothingmatches");
    assert_eq!(queue[0].advice, "ask_galion_research");
    drop(stack);
    let _ = std::fs::remove_dir_all(&dir);
}
