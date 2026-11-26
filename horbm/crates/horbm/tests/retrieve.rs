// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Phase 2 gate: `remember` denied leaves an empty cell and writes nothing.
//! A 1,000-row RAM fixture returns. Untimed.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[cfg(target_os = "linux")]
use std::fs::OpenOptions;
#[cfg(target_os = "linux")]
use std::io::Write;
#[cfg(target_os = "linux")]
use std::sync::{mpsc, Arc};
#[cfg(target_os = "linux")]
use std::thread;

use horbm::{CellState, RetrievalConfig, RetrievalThread, RetrievedCells, MEMORY_HIT_CAP};
#[cfg(target_os = "linux")]
use horbm::{HitPool, LearnedRow};

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

/// Unset `keys_path` is the lexical ask: one card is one knowledge hit.
/// The scan would also publish the same id from the diary leg.
#[test]
fn unset_keys_path_stays_lexical() {
    let unset = RetrievalConfig::default();
    assert!(!unset.remember);
    assert!(unset.keys_path.is_none());
    assert_eq!(unset.fixture_rows, 0);

    let dir = scratch("lexical");
    let cards = dir.join("cards");
    fs::create_dir(&cards).expect("cards");
    fs::write(
        cards.join("kettle.json"),
        r#"{
  "id": "kettle",
  "title": "The kettle is on the hob.",
  "section": "horbm",
  "category": "fixture",
  "summary": "The kettle is on the hob.",
  "body": "The kettle is on the hob.",
  "claim_type": "fact",
  "sources": [],
  "evidence_strength": "strong",
  "said_by": "horbm",
  "as_of_date": "2026-11-25",
  "status": "draft",
  "contested_views": [],
  "related": []
}
"#,
    )
    .expect("card");
    let thread = RetrievalThread::spawn(RetrievalConfig {
        remember: true,
        knowledge_dir: Some(cards),
        ..unset
    })
    .expect("lexical grant");
    thread.ask_wait("where is the kettle", 1, 1).expect("ask");
    let cell = thread.cell();
    assert_eq!(cell.state, CellState::Hits);
    assert_eq!(cell.hits.len(), 1, "{:?}", cell.hits);
    assert!(cell.hits.len() <= MEMORY_HIT_CAP);
    let hit = &cell.hits[0];
    assert_eq!(hit.item_id, "kettle");
    assert_eq!(hit.pool, horbm::HitPool::Knowledge);
    assert_eq!(hit.clause(), "The kettle is on the hob.");
    assert_ne!(hit.clause(), hit.item_id);
    assert!(cell.answer.is_empty());
    drop(thread);
    let _ = fs::remove_dir_all(&dir);
}

/// A granted key file publishes at most four hits. Deny does not open the file.
#[test]
fn granted_key_file_publishes_at_most_four_hits() {
    let dir = scratch("keys");
    let cards = dir.join("cards");
    fs::create_dir(&cards).expect("cards");
    fs::write(
        cards.join("bowl.json"),
        r#"{
  "id": "bowl",
  "title": "The blue bowl is in the cupboard.",
  "section": "horbm",
  "category": "fixture",
  "summary": "The blue bowl is in the cupboard.",
  "body": "The blue bowl is in the cupboard.",
  "claim_type": "fact",
  "sources": [],
  "evidence_strength": "strong",
  "said_by": "horbm",
  "as_of_date": "2026-11-25",
  "status": "draft",
  "contested_views": [],
  "related": []
}
"#,
    )
    .expect("card");
    let keys = dir.join("keys.hkb");
    write_keys(&keys, &["bowl", "not-in-store"], "where is the blue bowl");

    let thread = RetrievalThread::spawn(RetrievalConfig {
        remember: true,
        knowledge_dir: Some(cards.clone()),
        fixture_rows: 0,
        keys_path: Some(keys.clone()),
    })
    .expect("grant opens keys");
    thread
        .ask_wait("where is the blue bowl", 1, 1)
        .expect("ask");
    let cell = thread.cell();
    assert_ne!(cell.state, CellState::Idle);
    assert!(!cell.hits.is_empty(), "granted ask published no hit");
    assert!(cell.hits.len() <= MEMORY_HIT_CAP, "{}", cell.hits.len());
    assert!(
        cell.hits.iter().all(|hit| hit.item_id != "not-in-store"),
        "unresolved id was published: {:?}",
        cell.hits
    );
    assert!(
        cell.hits
            .iter()
            .all(|hit| hit.clause() == "The blue bowl is in the cupboard."),
        "clause was not the stored card: {:?}",
        cell.hits
    );
    drop(thread);

    let garbage = b"this is not an HKB1 file";
    fs::write(&keys, garbage).expect("corrupt keys");
    let denied = RetrievalThread::spawn(RetrievalConfig {
        remember: false,
        knowledge_dir: Some(cards),
        fixture_rows: 0,
        keys_path: Some(keys.clone()),
    })
    .expect("deny does not open keys");
    denied
        .ask_wait("where is the blue bowl", 2, 1)
        .expect("ask");
    assert!(denied.cell().is_empty());
    assert!(denied.last_error().is_none());
    assert_eq!(fs::read(&keys).expect("keys"), garbage);
    drop(denied);

    let opened = RetrievalThread::spawn(RetrievalConfig {
        remember: true,
        knowledge_dir: None,
        fixture_rows: 0,
        keys_path: Some(keys.clone()),
    });
    assert!(opened.is_err(), "grant opened a corrupt key file");
    let _ = fs::remove_dir_all(&dir);
}

fn write_keys(path: &Path, ids: &[&str], text: &str) {
    let mut lane = [0i8; 8];
    horbm_brain::encode_query_into(text, &mut lane);
    let n = ids.len();
    let mut keys = Vec::with_capacity(n * lane.len());
    for _ in 0..n {
        keys.extend_from_slice(&lane);
    }
    let pack = horbm_brain::StoredPack {
        k: lane.len(),
        keys,
        clause_id: (0..n as u64).collect(),
        allow: vec![true; n],
        source: vec![1; n],
        item_id: ids.iter().map(|id| (*id).to_string()).collect(),
        scale: vec![1; n],
    };
    pack.write(path).expect("write keys");
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

/// One-line card the fifo stall turns into a regular file before the
/// learned rebuild reads the directory again.
#[cfg(target_os = "linux")]
const PENDING_CARD: &str = r#"{"id":"kettle","title":"The kettle is on the hob.","section":"horbm","category":"fixture","summary":"The kettle is on the hob.","body":"The kettle is on the hob.","claim_type":"fact","sources":[],"evidence_strength":"strong","said_by":"northwind","as_of_date":"2026-11-25","status":"draft","contested_views":[],"related":[]}"#;

#[cfg(target_os = "linux")]
fn pending_marker() -> LearnedRow {
    LearnedRow {
        item_id: "pack-1".to_string(),
        text: "The pending token is marigold".to_string(),
        said_by: "northwind".to_string(),
        date: "2026-11-26T00:00:00Z".to_string(),
        source: "session=sess;turn=1;time=2026-11-26T00:00:00Z;consent=learn-v1".to_string(),
    }
}

#[cfg(target_os = "linux")]
fn task_wchan(tid: &str) -> String {
    fs::read_to_string(format!("/proc/self/task/{tid}/wchan")).unwrap_or_default()
}

#[cfg(target_os = "linux")]
fn retrieval_wchan() -> String {
    let Ok(tasks) = fs::read_dir("/proc/self/task") else {
        return "no task dir".to_string();
    };
    for task in tasks.flatten() {
        let comm = fs::read_to_string(task.path().join("comm")).unwrap_or_default();
        if comm.trim() == "horbm-retrieval" {
            return fs::read_to_string(task.path().join("wchan"))
                .unwrap_or_default()
                .trim()
                .to_string();
        }
    }
    "horbm-retrieval missing".to_string()
}

#[cfg(target_os = "linux")]
fn wait_wchan(read: impl Fn() -> String, want: &str, what: &str) {
    let started = Instant::now();
    let mut seen = String::new();
    while started.elapsed() < Duration::from_secs(10) {
        seen = read();
        if seen.trim() == want {
            return;
        }
        thread::sleep(Duration::from_millis(5));
    }
    panic!("{what}; last wchan {seen:?}");
}

/// Opens the write side if the test unwinds while the worker is blocked.
#[cfg(target_os = "linux")]
struct FifoRelease {
    path: PathBuf,
    writer: Option<std::fs::File>,
    done: bool,
}

#[cfg(target_os = "linux")]
impl FifoRelease {
    fn finish(&mut self, bytes: &[u8]) {
        if self.writer.is_none() {
            self.writer = Some(
                OpenOptions::new()
                    .write(true)
                    .open(&self.path)
                    .expect("fifo writer"),
            );
        }
        fs::remove_file(&self.path).expect("unlink fifo");
        fs::write(&self.path, bytes).expect("regular card");
        if let Some(writer) = self.writer.as_mut() {
            writer.write_all(bytes).expect("fifo body");
        }
        self.writer.take();
        self.done = true;
    }
}

#[cfg(target_os = "linux")]
impl Drop for FifoRelease {
    fn drop(&mut self) {
        if self.done {
            return;
        }
        if let Some(mut writer) = self.writer.take() {
            let _ = writer.write_all(b"{}");
            return;
        }
        // Linux O_NONBLOCK. Retry briefly so a reader that arrives during
        // unwind is released, and a missing reader does not stall exit.
        use std::os::unix::fs::OpenOptionsExt;
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(2) {
            if let Ok(mut writer) = OpenOptions::new()
                .write(true)
                .custom_flags(0o4000)
                .open(&self.path)
            {
                let _ = writer.write_all(b"{}");
                return;
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
}

/// The worker is stuck reading a fifo card, so `set_learned` sits in the
/// one slot. Asks fired then must leave that command in place.
#[cfg(target_os = "linux")]
#[test]
fn set_learned_is_not_replaced_by_a_pending_ask() {
    let _guard = disk_lock();
    let dir = scratch("pending-ask");
    let cards = dir.join("cards");
    fs::create_dir(&cards).expect("cards");
    let fifo = cards.join("kettle.json");
    let made = std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .expect("mkfifo");
    assert!(made.success(), "mkfifo {}", fifo.display());

    let thread = Arc::new(
        RetrievalThread::spawn(RetrievalConfig {
            remember: false,
            knowledge_dir: Some(cards),
            fixture_rows: 0,
            keys_path: None,
        })
        .expect("spawn denied"),
    );
    let mut release = FifoRelease {
        path: fifo,
        writer: None,
        done: false,
    };

    let granting = Arc::clone(&thread);
    let grant = thread::spawn(move || granting.set_remember(true));
    wait_wchan(
        retrieval_wchan,
        "wait_for_partner",
        "retrieval thread did not block opening the card fifo",
    );

    let (tid_tx, tid_rx) = mpsc::channel();
    let learning = Arc::clone(&thread);
    let pending = thread::spawn(move || {
        tid_tx.send(linux_tid()).expect("tid");
        learning
            .set_learned(vec![pending_marker()])
            .expect("set learned");
    });
    let tid = tid_rx.recv().expect("tid");
    wait_wchan(
        || task_wchan(&tid),
        "futex_wait_queue",
        "set_learned did not wait with the command still pending",
    );

    thread.ask("unrelated noise", 1, 1);
    thread.ask("another noise", 2, 2);
    assert_eq!(
        retrieval_wchan(),
        "wait_for_partner",
        "the worker left the fifo while the learned command was pending"
    );
    assert_eq!(
        task_wchan(&tid).trim(),
        "futex_wait_queue",
        "an ask replaced the pending learned command"
    );

    release.finish(PENDING_CARD.as_bytes());
    grant.join().expect("grant thread").expect("grant");
    pending.join().expect("learned thread");
    assert!(
        thread.cell().is_empty(),
        "a discarded ask published {:?}",
        thread.cell()
    );
    assert!(thread.last_error().is_none(), "{:?}", thread.last_error());

    thread.ask_wait("marigold", 3, 3).expect("ask");
    let cell = thread.cell();
    assert_eq!(cell.state, CellState::Hits, "{:?}", cell);
    assert!(
        cell.hits.iter().any(|hit| {
            hit.pool == HitPool::Memory
                && hit.source.starts_with("learned;")
                && hit.text.contains("marigold")
        }),
        "{:?}",
        cell.hits
    );
    assert!(cell.hits.len() <= MEMORY_HIT_CAP);
    drop(thread);
    let _ = fs::remove_dir_all(&dir);
}

#[cfg(target_os = "linux")]
fn linux_tid() -> String {
    fs::read_link("/proc/thread-self")
        .expect("thread-self")
        .file_name()
        .expect("tid")
        .to_string_lossy()
        .into_owned()
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
