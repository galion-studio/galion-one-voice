// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Phase 2 gate: `remember` denied leaves an empty cell and writes nothing.
//! A 1,000-row RAM fixture returns. Untimed.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use horbm::{CellState, RetrievalConfig, RetrievalThread, RetrievedCells, MEMORY_HIT_CAP};

fn disk_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().expect("disk lock")
}

fn scratch(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("horbm-f2-{name}-{}-{nanos}", std::process::id()));
    fs::create_dir_all(&dir).expect("scratch");
    dir
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

fn suspicious(dir: &Path) -> Vec<String> {
    names(dir)
        .into_iter()
        .filter(|name| {
            let lower = name.to_ascii_lowercase();
            lower.contains("horbm-mem")
                || lower.ends_with(".sqlite")
                || lower.ends_with(".sqlite-wal")
                || lower.ends_with(".sqlite-shm")
                || lower.ends_with(".db")
        })
        .collect()
}

#[test]
fn remember_denied_publishes_an_empty_cell_and_writes_nothing() {
    let _guard = disk_lock();
    let scratch = scratch("deny");
    let cwd = std::env::current_dir().expect("cwd");
    let before_cwd = suspicious(&cwd);
    let before_temp = suspicious(&std::env::temp_dir());
    let thread = RetrievalThread::spawn(RetrievalConfig {
        remember: false,
        knowledge_dir: Some(scratch.clone()),
        fixture_rows: 1_000,
        keys_path: None,
    })
    .expect("spawn denied");
    assert!(thread.cell().is_empty());
    assert!(thread.try_cell().is_some_and(|cell| cell.is_empty()));
    thread
        .ask_wait("marker m742", 1, 1)
        .expect("ask while denied");
    let cell = thread.cell();
    assert!(
        cell.is_empty(),
        "denied cell kept query {:? } state {:?}",
        cell.query,
        cell.state
    );
    assert!(cell.hits.is_empty());
    assert!(cell.answer.is_empty());
    assert_eq!(cell.state, CellState::Idle);
    assert!(names(&scratch).is_empty(), "scratch: {:?}", names(&scratch));
    assert!(!scratch.join("revisions.sqlite").exists());
    assert_eq!(suspicious(&cwd), before_cwd);
    assert_eq!(suspicious(&std::env::temp_dir()), before_temp);
    drop(thread);
    assert!(names(&scratch).is_empty());
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn fixture_of_one_thousand_rows_returns_and_withdraw_clears_the_cell() {
    let _guard = disk_lock();
    let scratch = scratch("fixture");
    let cwd = std::env::current_dir().expect("cwd");
    let before_cwd = suspicious(&cwd);
    let before_temp = suspicious(&std::env::temp_dir());
    let thread = RetrievalThread::spawn(RetrievalConfig {
        remember: true,
        knowledge_dir: Some(scratch.clone()),
        fixture_rows: 1_000,
        keys_path: None,
    })
    .expect("spawn fixture");

    thread.ask_wait("m742", 7, 3).expect("ask");
    let cell = thread.cell();
    assert_eq!(cell.state, CellState::Hits);
    assert!(!cell.hits.is_empty());
    assert!(cell.hits.len() <= MEMORY_HIT_CAP);
    assert!(
        cell.hits.iter().any(|hit| hit.item_id == "sample-742"),
        "hits: {:?}",
        cell.hits
            .iter()
            .map(|hit| hit.item_id.as_str())
            .collect::<Vec<_>>()
    );
    assert!(cell.hits.iter().any(|hit| hit.clause().contains("m742")));
    assert!(cell.answer.is_empty());

    thread.ask_wait("zzznomatchtoken", 7, 5).expect("miss");
    let miss = thread.cell();
    assert_eq!(miss.state, CellState::Miss);
    assert!(miss.hits.is_empty());
    assert_eq!(miss.answer, horbm::UNKNOWN_REPLY);
    assert!(!miss.is_empty());

    thread.ask("m500", 8, 6);
    let started = Instant::now();
    let found = loop {
        let snap = thread.cell();
        if snap.epoch == 6 && snap.hits.iter().any(|hit| hit.item_id == "sample-500") {
            break true;
        }
        if started.elapsed() > Duration::from_secs(30) {
            break false;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    assert!(found, "non-blocking ask did not publish sample-500");

    thread.set_remember(false).expect("withdraw");
    assert!(thread.cell().is_empty());
    thread.ask_wait("m742", 9, 7).expect("ask after withdraw");
    assert!(thread.cell().is_empty());

    assert!(
        names(&scratch).is_empty(),
        "fixture wrote {:?}",
        names(&scratch)
    );
    assert!(!scratch.join("revisions.sqlite").exists());
    assert_eq!(suspicious(&cwd), before_cwd, "cwd gained a diary file");
    assert_eq!(
        suspicious(&std::env::temp_dir()),
        before_temp,
        "temp gained a diary file"
    );
    drop(thread);
    let _ = fs::remove_dir_all(&scratch);
}

/// From the superseded in-process FTS crate: stopwords alone are not a query.
/// Here that is a granted miss, `I don't know.`, and the diary still writes nothing.
#[test]
fn stopwords_alone_are_a_miss_and_write_nothing() {
    let _guard = disk_lock();
    let scratch = scratch("stop");
    let thread = RetrievalThread::spawn(RetrievalConfig {
        remember: true,
        knowledge_dir: Some(scratch.clone()),
        fixture_rows: 1_000,
        keys_path: None,
    })
    .expect("spawn");
    thread.ask_wait("the and of", 1, 1).expect("stopword ask");
    let cell = thread.cell();
    assert_eq!(cell.state, CellState::Miss);
    assert!(cell.hits.is_empty());
    assert_eq!(cell.answer, horbm::UNKNOWN_REPLY);
    assert!(suspicious(&scratch).is_empty(), "{:?}", names(&scratch));
    assert!(!scratch.join("revisions.sqlite").exists());
    drop(thread);
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn deny_does_not_open_a_missing_key_file() {
    let missing = scratch("no-keys").join("keys.hkb");
    let thread = RetrievalThread::spawn(RetrievalConfig {
        remember: false,
        knowledge_dir: None,
        fixture_rows: 0,
        keys_path: Some(missing.clone()),
    })
    .expect("deny does not open keys");
    thread.ask_wait("blue bowl", 1, 1).expect("ask");
    assert!(thread.cell().is_empty());
    assert!(!missing.exists());
    drop(thread);
    let _ = fs::remove_dir_all(missing.parent().unwrap());
}

#[test]
fn a_missing_key_file_fails_the_grant() {
    let missing = scratch("bad-keys").join("absent.hkb");
    let spawned = RetrievalThread::spawn(RetrievalConfig {
        remember: true,
        knowledge_dir: None,
        fixture_rows: 0,
        keys_path: Some(missing.clone()),
    });
    let err = match spawned {
        Err(err) => err,
        Ok(_) => panic!("grant opened a missing key file"),
    };
    assert!(!err.message().is_empty());
    assert!(!err.message().contains("blue bowl"));
    assert!(!missing.exists());
    let _ = fs::remove_dir_all(missing.parent().unwrap());
}

#[test]
#[ignore = "set GALION_HORBM_REMEMBER=grant and GALION_HORBM_KNOWLEDGE to the card folder"]
fn real_card_folder_is_read_and_not_written() {
    let cfg = RetrievalConfig::from_env();
    assert!(
        cfg.remember,
        "set GALION_HORBM_REMEMBER=grant (the lock default is deny)"
    );
    let dir = cfg
        .knowledge_dir
        .clone()
        .expect("set GALION_HORBM_KNOWLEDGE to the card folder");
    let before = names(&dir);
    let thread = RetrievalThread::spawn(cfg).expect("ingest cards");
    thread
        .ask_wait("photosynthesis", 1, 1)
        .expect("query the folder");
    let cell = thread.cell();
    assert_ne!(cell.state, CellState::Idle);
    assert!(cell.hits.len() <= MEMORY_HIT_CAP);
    assert!(cell.answer.is_empty() || cell.answer == horbm::UNKNOWN_REPLY);
    assert_eq!(names(&dir), before, "card folder changed");
    assert!(!dir.join("revisions.sqlite").exists());
}
