// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Reload re-reads the card directory without restarting the retrieval thread.
//!
//! Cards here are one line written into a temp directory. Nothing here is a
//! committed card. The 50-card case prints its reload and ask times.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[cfg(target_os = "linux")]
use std::fs::OpenOptions;
#[cfg(target_os = "linux")]
use std::io::Write;
#[cfg(target_os = "linux")]
use std::sync::{mpsc, Arc};
#[cfg(target_os = "linux")]
use std::thread;
#[cfg(target_os = "linux")]
use std::time::Duration;

use horbm::{
    CellState, HitPool, LearnedRow, RetrievalConfig, RetrievalThread, RetrievedCells,
    MEMORY_HIT_CAP,
};

fn gate() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().expect("reload tests")
}

fn scratch(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "horbm-reload-{name}-{}-{nanos}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).expect("scratch");
    dir
}

fn card_line(id: &str, sentence: &str) -> String {
    format!(
        r#"{{"id":"{id}","title":"{sentence}","section":"horbm","category":"fixture","summary":"{sentence}","body":"{sentence}","claim_type":"fact","sources":[],"evidence_strength":"strong","said_by":"northwind","as_of_date":"2026-11-25","status":"draft","contested_views":[],"related":[]}}"#
    )
}

fn write_card(dir: &Path, id: &str, sentence: &str) {
    fs::create_dir_all(dir).expect("cards");
    fs::write(dir.join(format!("{id}.json")), card_line(id, sentence)).expect("card");
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

fn names(dir: &Path) -> Vec<String> {
    let mut found = fs::read_dir(dir)
        .expect("names")
        .flatten()
        .map(|ent| ent.file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    found.sort();
    found
}

fn learned_row() -> LearnedRow {
    LearnedRow {
        item_id: "pack-1".to_string(),
        text: "The spare oar is in the locker.".to_string(),
        said_by: "northwind".to_string(),
        date: "2026-11-26T00:00:00Z".to_string(),
        source: "session=sess;turn=1;time=2026-11-26T00:00:00Z;consent=learn-v1".to_string(),
    }
}

#[test]
fn added_card_is_a_hit_after_reload() {
    let _guard = gate();
    let root = scratch("add");
    let cards = root.join("cards");
    fs::create_dir_all(&cards).expect("cards");
    let thread = spawn(true, &cards);
    thread.ask_wait("kettle", 1, 1).expect("ask empty");
    let miss = thread.cell();
    assert_eq!(miss.state, CellState::Miss, "{miss:?}");
    assert!(miss.hits.is_empty());
    assert_eq!(miss.answer, horbm::UNKNOWN_REPLY);

    write_card(&cards, "kettle", "The kettle is on the hob.");
    thread.reload().expect("reload");
    assert!(thread.last_error().is_none(), "{:?}", thread.last_error());
    thread.ask_wait("kettle", 2, 2).expect("ask");
    let cell = thread.cell();
    assert_eq!(cell.state, CellState::Hits, "{cell:?}");
    assert_eq!(cell.hits.len(), 1, "{:?}", cell.hits);
    assert!(cell.hits.len() <= MEMORY_HIT_CAP);
    assert_eq!(cell.hits[0].item_id, "kettle");
    assert_eq!(cell.hits[0].pool, HitPool::Knowledge);
    assert_eq!(cell.hits[0].clause(), "The kettle is on the hob.");
    assert!(cell.answer.is_empty());
    drop(thread);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn removed_card_is_a_miss_after_reload() {
    let _guard = gate();
    let root = scratch("remove");
    let cards = root.join("cards");
    write_card(&cards, "kettle", "The kettle is on the hob.");
    write_card(&cards, "lantern", "The lantern is in the shed.");
    let thread = spawn(true, &cards);
    thread.ask_wait("kettle", 1, 1).expect("ask");
    assert_eq!(thread.cell().hits[0].item_id, "kettle");

    fs::remove_file(cards.join("kettle.json")).expect("remove card");
    thread.reload().expect("reload");
    assert!(thread.last_error().is_none(), "{:?}", thread.last_error());
    thread.ask_wait("kettle", 2, 2).expect("ask gone");
    let miss = thread.cell();
    assert_eq!(miss.state, CellState::Miss, "{miss:?}");
    assert!(miss.hits.is_empty());
    assert_eq!(miss.answer, horbm::UNKNOWN_REPLY);
    thread.ask_wait("lantern", 3, 3).expect("ask kept");
    let kept = thread.cell();
    assert_eq!(kept.state, CellState::Hits, "{kept:?}");
    assert_eq!(kept.hits[0].item_id, "lantern");
    assert_eq!(kept.hits[0].pool, HitPool::Knowledge);
    drop(thread);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn learned_rows_survive_a_reload() {
    let _guard = gate();
    let root = scratch("learned");
    let cards = root.join("cards");
    write_card(&cards, "kettle", "The kettle is on the hob.");
    let thread = spawn(true, &cards);
    let row = learned_row();
    let label = row.source.clone();
    thread.set_learned(vec![row]).expect("set learned");
    thread.ask_wait("spare oar", 1, 1).expect("ask learned");
    assert_eq!(thread.cell().state, CellState::Hits);

    write_card(&cards, "lantern", "The lantern is in the shed.");
    thread.reload().expect("reload");
    assert!(thread.last_error().is_none(), "{:?}", thread.last_error());
    thread
        .ask_wait("spare oar", 2, 2)
        .expect("ask learned again");
    let cell = thread.cell();
    assert_eq!(cell.state, CellState::Hits, "{cell:?}");
    let learned: Vec<_> = cell
        .hits
        .iter()
        .filter(|hit| hit.source.starts_with("learned;"))
        .collect();
    assert_eq!(learned.len(), 1, "{:?}", cell.hits);
    assert_eq!(learned[0].pool, HitPool::Memory);
    assert_eq!(learned[0].text, "The spare oar is in the locker.");
    assert_eq!(learned[0].said_by, "northwind");
    assert_eq!(learned[0].source, format!("learned;{label}"));
    thread.ask_wait("lantern", 3, 3).expect("ask new card");
    assert_eq!(thread.cell().hits[0].item_id, "lantern");
    thread.ask_wait("kettle", 4, 4).expect("ask old card");
    assert_eq!(thread.cell().hits[0].item_id, "kettle");
    drop(thread);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn denied_reload_opens_nothing() {
    let _guard = gate();
    let root = scratch("deny");
    let cards = root.join("cards");
    write_card(&cards, "kettle", "The kettle is on the hob.");
    let before = names(&cards);
    let thread = spawn(false, &cards);
    thread
        .set_learned(vec![learned_row()])
        .expect("set learned");
    thread.reload().expect("reload");
    assert!(thread.last_error().is_none(), "{:?}", thread.last_error());
    thread.ask_wait("kettle", 1, 1).expect("ask card");
    assert!(thread.cell().is_empty(), "{:?}", thread.cell());
    thread.ask_wait("spare oar", 2, 2).expect("ask learned");
    assert!(thread.cell().is_empty(), "{:?}", thread.cell());
    assert_eq!(names(&cards), before, "denied reload wrote the card dir");
    assert!(before.iter().all(|name| !name.contains("sqlite")));
    assert!(!cards.join("revisions.sqlite").exists());

    thread.set_remember(true).expect("grant");
    thread.ask_wait("spare oar", 3, 3).expect("ask after grant");
    let cell = thread.cell();
    assert_eq!(cell.state, CellState::Hits, "{cell:?}");
    assert!(
        cell.hits.iter().any(|hit| {
            hit.pool == HitPool::Memory
                && hit.source.starts_with("learned;")
                && hit.text.contains("spare oar")
        }),
        "{:?}",
        cell.hits
    );
    drop(thread);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_failed_reload_keeps_the_old_hits() {
    let _guard = gate();
    let root = scratch("fail");
    let cards = root.join("cards");
    write_card(&cards, "kettle", "The kettle is on the hob.");
    let thread = spawn(true, &cards);
    thread.ask_wait("kettle", 1, 1).expect("ask");
    let before = thread.cell();
    assert_eq!(before.state, CellState::Hits, "{before:?}");
    assert_eq!(before.hits[0].item_id, "kettle");
    assert_eq!(before.hits[0].clause(), "The kettle is on the hob.");

    write_card(&cards, "lantern", "The lantern is in the shed.");
    fs::write(cards.join("broken.json"), "{").expect("broken card");
    thread.reload().expect("failed reload still returns");
    let err = thread.last_error().expect("failed reload sets last_error");
    assert!(err.contains("broken.json"), "{err}");
    let kept = thread.cell();
    assert_eq!(kept.state, CellState::Hits, "{kept:?}");
    assert_eq!(kept.query, before.query);
    assert_eq!(kept.hits[0].item_id, "kettle");
    assert_eq!(kept.hits[0].clause(), "The kettle is on the hob.");

    thread.ask_wait("kettle", 2, 2).expect("ask old");
    let again = thread.cell();
    assert_eq!(again.state, CellState::Hits, "{again:?}");
    assert_eq!(again.hits[0].item_id, "kettle");
    assert_eq!(again.hits[0].pool, HitPool::Knowledge);
    thread.ask_wait("lantern", 3, 3).expect("ask new");
    let missed = thread.cell();
    assert_eq!(missed.state, CellState::Miss, "{missed:?}");
    assert!(missed.hits.iter().all(|hit| hit.item_id != "lantern"));

    fs::remove_file(cards.join("broken.json")).expect("drop broken card");
    thread.reload().expect("reload after the fix");
    assert!(thread.last_error().is_none(), "{:?}", thread.last_error());
    thread.ask_wait("lantern", 4, 4).expect("ask lantern");
    assert_eq!(thread.cell().hits[0].item_id, "lantern");
    thread.ask_wait("kettle", 5, 5).expect("ask kettle");
    assert_eq!(thread.cell().hits[0].item_id, "kettle");
    drop(thread);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn fifty_cards_reload_prints_the_timing() {
    let _guard = gate();
    let root = scratch("fifty");
    let cards = root.join("cards");
    fs::create_dir_all(&cards).expect("cards");
    for i in 0..50 {
        write_card(
            &cards,
            &format!("marker-{i:02}"),
            &format!("Marker m{i:02} sits beside the shelf."),
        );
    }
    let thread = spawn(true, &cards);
    let started = Instant::now();
    thread.reload().expect("reload");
    let reload_us = started.elapsed().as_micros();
    assert!(thread.last_error().is_none(), "{:?}", thread.last_error());
    let ask_started = Instant::now();
    thread.ask_wait("m17", 1, 1).expect("ask");
    let ask_us = ask_started.elapsed().as_micros();
    eprintln!("reload 50 cards: {reload_us} us; ask: {ask_us} us");
    let cell = thread.cell();
    assert_eq!(cell.state, CellState::Hits, "{cell:?}");
    assert_eq!(cell.hits.len(), 1, "{:?}", cell.hits);
    assert!(cell.hits.len() <= MEMORY_HIT_CAP);
    assert_eq!(cell.hits[0].item_id, "marker-17");
    assert_eq!(cell.hits[0].pool, HitPool::Knowledge);
    thread.ask_wait("m99", 2, 2).expect("miss");
    assert_eq!(thread.cell().state, CellState::Miss);
    drop(thread);
    let _ = fs::remove_dir_all(&root);
}

/// One-line card the fifo stall turns into a regular file before reload
/// reads the directory again.
#[cfg(target_os = "linux")]
const PENDING_CARD: &str = r#"{"id":"kettle","title":"The kettle is on the hob.","section":"horbm","category":"fixture","summary":"The kettle is on the hob.","body":"The kettle is on the hob.","claim_type":"fact","sources":[],"evidence_strength":"strong","said_by":"northwind","as_of_date":"2026-11-25","status":"draft","contested_views":[],"related":[]}"#;

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

#[cfg(target_os = "linux")]
fn linux_tid() -> String {
    fs::read_link("/proc/thread-self")
        .expect("thread-self")
        .file_name()
        .expect("tid")
        .to_string_lossy()
        .into_owned()
}

/// The worker is stuck reading a fifo card, so `reload` sits in the one slot.
/// Asks fired then must leave that command in place.
#[cfg(target_os = "linux")]
#[test]
fn a_pending_ask_does_not_replace_a_reload() {
    let _guard = gate();
    let root = scratch("pending");
    let cards = root.join("cards");
    fs::create_dir(&cards).expect("cards");
    let fifo = cards.join("kettle.json");
    let made = std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .expect("mkfifo");
    assert!(made.success(), "mkfifo {}", fifo.display());

    let thread = Arc::new(spawn(false, &cards));
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
    let reloading = Arc::clone(&thread);
    let pending = thread::spawn(move || {
        tid_tx.send(linux_tid()).expect("tid");
        reloading.reload().expect("reload");
    });
    let tid = tid_rx.recv().expect("tid");
    wait_wchan(
        || task_wchan(&tid),
        "futex_wait_queue",
        "reload did not wait with the command still pending",
    );

    thread.ask("unrelated noise", 1, 1);
    thread.ask("another noise", 2, 2);
    assert_eq!(
        retrieval_wchan(),
        "wait_for_partner",
        "the worker left the fifo while the reload command was pending"
    );
    assert_eq!(
        task_wchan(&tid).trim(),
        "futex_wait_queue",
        "an ask replaced the pending reload"
    );

    release.finish(PENDING_CARD.as_bytes());
    grant.join().expect("grant thread").expect("grant");
    pending.join().expect("reload thread");
    assert!(
        thread.cell().is_empty(),
        "a discarded ask published {:?}",
        thread.cell()
    );
    assert!(thread.last_error().is_none(), "{:?}", thread.last_error());

    thread.ask_wait("kettle", 3, 3).expect("ask kettle");
    let cell = thread.cell();
    assert_eq!(cell.state, CellState::Hits, "{cell:?}");
    assert_eq!(cell.hits[0].item_id, "kettle");
    assert_eq!(cell.hits[0].pool, HitPool::Knowledge);
    assert!(cell.hits.len() <= MEMORY_HIT_CAP);
    drop(thread);
    let _ = fs::remove_dir_all(&root);
}
