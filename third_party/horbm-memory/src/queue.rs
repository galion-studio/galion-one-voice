// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Background queue for transcript flushes, gap rows, extraction, summaries,
//! pins, and revisions.
//!
//! `push_turn` and `query` only enqueue. A worker thread owns a second
//! SQLite connection. The voice path never waits on it. This is the
//! in-process form of the old Galion Research split: recall on the turn,
//! capture after. It is not an HTTP call to `openmind-memory`.

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use rusqlite::{params, Connection, OpenFlags, OptionalExtension};

use crate::card::sha256_hex;
use crate::embed::HashingEmbedder;
use crate::error::{Error, Result};
use crate::extract::{
    extract_line, is_single_value_slot, summarise_session, SummaryTurn, SUMMARY_IMPORTANCE,
};
use crate::text::index_tokens;

const WRITE_BUSY: Duration = Duration::from_millis(2_000);

#[derive(Clone, Debug)]
pub(crate) struct DbTarget {
    pub uri: String,
    pub memory: bool,
}

pub(crate) fn open_connection(target: &DbTarget) -> Result<Connection> {
    let conn = if target.memory {
        Connection::open_with_flags(
            &target.uri,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_URI,
        )?
    } else {
        Connection::open(&target.uri)?
    };
    // Set this before the WAL pragma. A second connection can arrive while
    // the first is still reading, and a zero timeout turns that into a
    // dead worker.
    conn.busy_timeout(WRITE_BUSY)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    Ok(conn)
}

#[derive(Clone)]
pub(crate) struct FactJob {
    pub item_id: String,
    pub text: String,
    pub said_by: String,
    pub date: String,
    pub source: String,
    pub pinned: bool,
    pub importance: f32,
    pub kind: String,
    pub extractor: String,
    pub session_id: String,
    pub subject_key: String,
    pub object_value: String,
}

enum Job {
    PersistTurn {
        session_id: String,
        turn_id: String,
        said_by: String,
        text: String,
        date: String,
        source: String,
    },
    Extract {
        session_id: String,
        said_by: String,
        text: String,
        date: String,
        source: String,
    },
    Summarise {
        session_id: String,
    },
    LogGap {
        query: String,
        reason: String,
        nearest: String,
        advice: String,
        detail: String,
    },
    CommitPin(FactJob),
    Revise {
        old_id: String,
        new_fact: FactJob,
    },
    SetKeep {
        session_id: String,
        keep: bool,
    },
    Unpromote {
        session_id: String,
    },
    Shutdown,
}

struct Signal {
    lock: Mutex<()>,
    cv: Condvar,
}

pub(crate) struct JobQueue {
    tx: Sender<Job>,
    pending: Arc<AtomicUsize>,
    paused: Arc<AtomicBool>,
    signal: Arc<Signal>,
    last_error: Arc<Mutex<Option<String>>>,
    handle: Option<JoinHandle<()>>,
}

impl JobQueue {
    pub(crate) fn spawn(target: DbTarget, seq: Arc<AtomicU64>) -> Result<Self> {
        let (tx, rx) = mpsc::channel();
        let pending = Arc::new(AtomicUsize::new(0));
        let paused = Arc::new(AtomicBool::new(false));
        let signal = Arc::new(Signal {
            lock: Mutex::new(()),
            cv: Condvar::new(),
        });
        let last_error = Arc::new(Mutex::new(None));
        let pending_w = Arc::clone(&pending);
        let paused_w = Arc::clone(&paused);
        let signal_w = Arc::clone(&signal);
        let error_w = Arc::clone(&last_error);
        let handle = thread::Builder::new()
            .name("horbm-memory".into())
            .spawn(move || worker(target, seq, rx, pending_w, paused_w, signal_w, error_w))
            .map_err(Error::Io)?;
        Ok(Self {
            tx,
            pending,
            paused,
            signal,
            last_error,
            handle: Some(handle),
        })
    }

    pub(crate) fn handle(&self) -> QueueHandle {
        QueueHandle {
            tx: self.tx.clone(),
            pending: Arc::clone(&self.pending),
        }
    }

    pub(crate) fn pause(&self) {
        let _guard = self
            .signal
            .lock
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        self.paused.store(true, Ordering::SeqCst);
    }

    pub(crate) fn resume(&self) {
        let _guard = self
            .signal
            .lock
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        self.paused.store(false, Ordering::SeqCst);
        self.signal.cv.notify_all();
    }

    pub(crate) fn pending(&self) -> usize {
        self.pending.load(Ordering::SeqCst)
    }

    fn enqueue(&self, job: Job) -> Result<()> {
        self.pending.fetch_add(1, Ordering::SeqCst);
        if self.tx.send(job).is_err() {
            self.pending.fetch_sub(1, Ordering::SeqCst);
            return Err(Error::Invalid("memory worker is not running".to_string()));
        }
        Ok(())
    }

    pub(crate) fn take_error(&self) -> Option<String> {
        self.last_error
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .take()
    }

    pub(crate) fn shutdown(&mut self) {
        self.resume();
        if self.handle.is_some() {
            let _ = self.tx.send(Job::Shutdown);
            if let Some(handle) = self.handle.take() {
                let _ = handle.join();
            }
        }
    }
}

impl Drop for JobQueue {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Cloneable sender for a second thread. The voice test uses it to keep
/// the writer busy while `query` runs. Sending does not touch SQLite.
#[derive(Clone)]
pub struct QueueHandle {
    tx: Sender<Job>,
    pending: Arc<AtomicUsize>,
}

impl QueueHandle {
    pub fn pending(&self) -> usize {
        self.pending.load(Ordering::SeqCst)
    }

    /// One persist, extract, summary, pin, and revise. The words are
    /// synthetic and do not include benchmark tokens (`sample`, `m{i}`).
    pub fn enqueue_background_mix(&self, n: u64) {
        let session_id = format!("bg-{n}");
        let pin_id = format!("bg-pin-{n}");
        let _ = self.enqueue(Job::SetKeep {
            session_id: session_id.clone(),
            keep: true,
        });
        let _ = self.enqueue(Job::PersistTurn {
            session_id: session_id.clone(),
            turn_id: format!("bg-turn-{n}"),
            said_by: "background".into(),
            text: format!("background transcript {n} token bgturn{n}"),
            date: "2026-11-18".into(),
            source: "session-ring".into(),
        });
        let _ = self.enqueue(Job::Extract {
            session_id: session_id.clone(),
            said_by: "background".into(),
            text: format!("I prefer bgpref{n}"),
            date: "2026-11-18".into(),
            source: "session-ring".into(),
        });
        let _ = self.enqueue(Job::Summarise {
            session_id: session_id.clone(),
        });
        let _ = self.enqueue(Job::CommitPin(FactJob {
            item_id: pin_id.clone(),
            text: format!("pinned background fact {n} token bgpin{n}"),
            said_by: "background".into(),
            date: "2026-11-18".into(),
            source: "background".into(),
            pinned: true,
            importance: 1.0,
            kind: "pin".into(),
            extractor: "user".into(),
            session_id: session_id.clone(),
            subject_key: String::new(),
            object_value: String::new(),
        }));
        let _ = self.enqueue(Job::Revise {
            old_id: pin_id,
            new_fact: FactJob {
                item_id: format!("bg-rev-{n}"),
                text: format!("revised background fact {n} token bgrev{n}"),
                said_by: "background".into(),
                date: "2026-11-18".into(),
                source: "background".into(),
                pinned: true,
                importance: 1.0,
                kind: "pin".into(),
                extractor: "user".into(),
                session_id,
                subject_key: String::new(),
                object_value: String::new(),
            },
        });
    }

    fn enqueue(&self, job: Job) -> Result<()> {
        self.pending.fetch_add(1, Ordering::SeqCst);
        if self.tx.send(job).is_err() {
            self.pending.fetch_sub(1, Ordering::SeqCst);
            return Err(Error::Invalid("memory worker is not running".to_string()));
        }
        Ok(())
    }
}

pub(crate) fn alloc_id(seq: &AtomicU64, prefix: &str) -> String {
    let n = seq.fetch_add(1, Ordering::SeqCst) + 1;
    format!("{prefix}-{n}")
}

fn worker(
    target: DbTarget,
    seq: Arc<AtomicU64>,
    rx: Receiver<Job>,
    pending: Arc<AtomicUsize>,
    paused: Arc<AtomicBool>,
    signal: Arc<Signal>,
    last_error: Arc<Mutex<Option<String>>>,
) {
    let conn = match open_connection(&target) {
        Ok(conn) => conn,
        Err(err) => {
            remember_error(&last_error, &err.to_string());
            while let Ok(job) = rx.recv() {
                if matches!(job, Job::Shutdown) {
                    break;
                }
                pending.fetch_sub(1, Ordering::SeqCst);
            }
            return;
        }
    };
    if let Err(err) = conn.busy_timeout(WRITE_BUSY) {
        remember_error(&last_error, &err.to_string());
    }
    while let Ok(job) = rx.recv() {
        if matches!(job, Job::Shutdown) {
            break;
        }
        while paused.load(Ordering::SeqCst) {
            let guard = signal.lock.lock().unwrap_or_else(|err| err.into_inner());
            if !paused.load(Ordering::SeqCst) {
                break;
            }
            let _guard = signal.cv.wait(guard).unwrap_or_else(|err| err.into_inner());
        }
        if let Err(err) = run_job(&conn, &seq, &job) {
            remember_error(&last_error, &err.to_string());
        }
        pending.fetch_sub(1, Ordering::SeqCst);
    }
}

fn remember_error(slot: &Mutex<Option<String>>, message: &str) {
    let mut guard = slot.lock().unwrap_or_else(|err| err.into_inner());
    if guard.is_none() {
        *guard = Some(message.to_string());
    }
}

fn run_job(conn: &Connection, seq: &AtomicU64, job: &Job) -> Result<()> {
    conn.execute_batch("BEGIN IMMEDIATE")?;
    let result = (|| {
        dispatch(conn, seq, job)?;
        let watermark = seq.load(Ordering::SeqCst) as i64;
        conn.execute(
            "UPDATE meta SET v = MAX(v, ?1) WHERE k = 'seq'",
            params![watermark],
        )?;
        record_job(conn, job)?;
        Ok(())
    })();
    match result {
        Ok(()) => {
            conn.execute_batch("COMMIT")?;
            Ok(())
        }
        Err(err) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(err)
        }
    }
}

fn dispatch(conn: &Connection, seq: &AtomicU64, job: &Job) -> Result<()> {
    match job {
        Job::PersistTurn {
            session_id,
            turn_id,
            said_by,
            text,
            date,
            source,
        } => insert_turn(conn, session_id, turn_id, said_by, text, date, source),
        Job::Extract {
            session_id,
            said_by,
            text,
            date,
            source,
        } => insert_candidates(conn, seq, session_id, said_by, text, date, source),
        Job::Summarise { session_id } => write_summary(conn, seq, session_id),
        Job::LogGap {
            query,
            reason,
            nearest,
            advice,
            detail,
        } => insert_gap(conn, query, reason, nearest, advice, detail),
        Job::CommitPin(fact) => insert_fact(conn, fact),
        Job::Revise { old_id, new_fact } => {
            archive_for_revision(conn, old_id)?;
            insert_fact(conn, new_fact)
        }
        Job::SetKeep { session_id, keep } => set_keep(conn, session_id, *keep),
        Job::Unpromote { session_id } => unpromote(conn, session_id),
        Job::Shutdown => Ok(()),
    }
}

fn record_job(conn: &Connection, job: &Job) -> Result<()> {
    let kind = match job {
        Job::PersistTurn { .. } => "persist_turn",
        Job::Extract { .. } => "extract",
        Job::Summarise { .. } => "summarise",
        Job::LogGap { .. } => "log_gap",
        Job::CommitPin(_) => "commit_pin",
        Job::Revise { .. } => "revise",
        Job::SetKeep { .. } => "set_keep",
        Job::Unpromote { .. } => "unpromote",
        Job::Shutdown => "shutdown",
    };
    let payload = match job {
        Job::PersistTurn {
            session_id,
            turn_id,
            ..
        } => format!(r#"{{"session_id":"{session_id}","turn_id":"{turn_id}"}}"#),
        Job::Extract { session_id, .. }
        | Job::Summarise { session_id }
        | Job::SetKeep { session_id, .. }
        | Job::Unpromote { session_id } => format!(r#"{{"session_id":"{session_id}"}}"#),
        Job::CommitPin(fact) => format!(r#"{{"item_id":"{}"}}"#, fact.item_id),
        Job::Revise { old_id, new_fact } => {
            format!(
                r#"{{"old_id":"{old_id}","item_id":"{}"}}"#,
                new_fact.item_id
            )
        }
        Job::LogGap { .. } | Job::Shutdown => "{}".to_string(),
    };
    conn.execute(
        "INSERT INTO memory_jobs (kind, payload, enqueued_at, finished_at, status)
         VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%fZ','now'), strftime('%Y-%m-%dT%H:%M:%fZ','now'), 'done')",
        params![kind, payload],
    )?;
    Ok(())
}

fn insert_turn(
    conn: &Connection,
    session_id: &str,
    turn_id: &str,
    said_by: &str,
    text: &str,
    date: &str,
    source: &str,
) -> Result<()> {
    let inserted = conn.execute(
        "INSERT OR IGNORE INTO session_turns (
            session_id, turn_id, said_by, body, source_date, source, recorded_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
        params![session_id, turn_id, said_by, text, date, source],
    )?;
    if inserted == 0 {
        return Ok(());
    }
    let rowid = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO session_turns_fts(rowid, body, said_by, source) VALUES (?1, ?2, ?3, ?4)",
        params![
            rowid,
            fts_field(text),
            fts_field(said_by),
            fts_field(source)
        ],
    )?;
    Ok(())
}

fn insert_candidates(
    conn: &Connection,
    seq: &AtomicU64,
    session_id: &str,
    said_by: &str,
    text: &str,
    date: &str,
    source: &str,
) -> Result<()> {
    for candidate in extract_line(text) {
        supersede_conflicts(conn, &candidate.subject_key, &candidate.object_value)?;
        let fact = FactJob {
            item_id: alloc_id(seq, "cand"),
            text: candidate.text,
            said_by: said_by.to_string(),
            date: date.to_string(),
            source: source.to_string(),
            pinned: false,
            importance: candidate.importance,
            kind: "candidate".into(),
            extractor: "system".into(),
            session_id: session_id.to_string(),
            subject_key: candidate.subject_key,
            object_value: candidate.object_value,
        };
        insert_fact(conn, &fact)?;
    }
    Ok(())
}

fn supersede_conflicts(conn: &Connection, subject_key: &str, object_value: &str) -> Result<()> {
    if !is_single_value_slot(subject_key) || object_value.is_empty() {
        return Ok(());
    }
    let mut stmt = conn.prepare(
        "SELECT item_id, object_value FROM items
         WHERE subject_key = ?1 AND current = 1 AND marked = 0 AND object_value != ''",
    )?;
    let rows = stmt.query_map(params![subject_key], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut stale = Vec::new();
    for row in rows {
        let (item_id, object) = row?;
        if object != object_value {
            stale.push(item_id);
        }
    }
    drop(stmt);
    for item_id in stale {
        archive_for_revision(conn, &item_id)?;
    }
    Ok(())
}

/// Copy the live text into `archive` and `memory_revisions`, then hide the
/// row from retrieval. The `items` body is kept. Nothing deletes that text.
pub(crate) fn archive_for_revision(conn: &Connection, item_id: &str) -> Result<()> {
    let moved = conn.execute(
        "INSERT INTO archive (
            item_id, layer, section, category, title, body, summary, claim_type,
            said_by, source_date, source, evidence_strength, status, reason,
            emb, emb_scale, emb_norm, document, content_hash
         )
         SELECT item_id, layer, section, category, title, body, summary, claim_type,
            said_by, source_date, source, evidence_strength, status, 'revised',
            emb, emb_scale, emb_norm, document, content_hash
         FROM items WHERE item_id = ?1 AND current = 1",
        params![item_id],
    )?;
    if moved != 1 {
        return Err(Error::NotFound(item_id.to_string()));
    }
    conn.execute(
        "INSERT INTO memory_revisions (
            item_id, body, said_by, source_date, source, archived_at, reason, document
         )
         SELECT item_id, body, said_by, source_date, source,
            strftime('%Y-%m-%dT%H:%M:%fZ','now'), 'revised', document
         FROM items WHERE item_id = ?1",
        params![item_id],
    )?;
    let rowid: i64 = conn.query_row(
        "SELECT rowid FROM items WHERE item_id = ?1",
        params![item_id],
        |row| row.get(0),
    )?;
    conn.execute(
        "UPDATE items SET current = 0, marked = 1 WHERE item_id = ?1",
        params![item_id],
    )?;
    conn.execute("DELETE FROM items_fts WHERE rowid = ?1", params![rowid])?;
    Ok(())
}

fn insert_fact(conn: &Connection, fact: &FactJob) -> Result<()> {
    let emb = HashingEmbedder.embed(&fact.text);
    let document = serde_json::json!({
        "kind": fact.kind,
        "text": fact.text,
        "said_by": fact.said_by,
        "date": fact.date,
        "source": fact.source,
        "pinned": fact.pinned,
        "importance": fact.importance,
        "extractor": fact.extractor,
        "subject_key": fact.subject_key,
    })
    .to_string();
    let hash = sha256_hex(document.as_bytes());
    let blob: Vec<u8> = emb.values.iter().map(|b| *b as u8).collect();
    conn.execute(
        "INSERT INTO items (
            item_id, layer, section, category, title, body, summary, claim_type,
            said_by, source_date, source, evidence_strength, status, marked,
            emb, emb_scale, emb_norm, document, content_hash,
            pinned, importance, kind, session_id, current, extractor,
            subject_key, object_value, last_access
         ) VALUES (
            ?1, 'episode', 'memory', ?2, '', ?3, ?3, 'user-said',
            ?4, ?5, ?6, '', '', 0,
            ?7, ?8, ?9, ?10, ?11,
            ?12, ?13, ?14, ?15, 1, ?16,
            ?17, ?18, ''
         )",
        params![
            fact.item_id,
            fact.kind,
            fact.text,
            fact.said_by,
            fact.date,
            fact.source,
            blob,
            emb.scale,
            emb.norm,
            document,
            hash,
            i64::from(fact.pinned),
            fact.importance,
            fact.kind,
            fact.session_id,
            fact.extractor,
            fact.subject_key,
            fact.object_value,
        ],
    )?;
    let rowid = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO items_fts(rowid, title, summary, body, said_by, source)
         VALUES (?1, '', ?2, ?2, ?3, ?4)",
        params![
            rowid,
            fts_field(&fact.text),
            fts_field(&fact.said_by),
            fts_field(&fact.source),
        ],
    )?;
    Ok(())
}

fn insert_gap(
    conn: &Connection,
    query: &str,
    reason: &str,
    nearest: &str,
    advice: &str,
    detail: &str,
) -> Result<()> {
    conn.execute(
        "INSERT INTO gaps (query, asked_at, reason, nearest, advice, detail)
         VALUES (?1, strftime('%Y-%m-%dT%H:%M:%fZ','now'), ?2, ?3, ?4, ?5)",
        params![query, reason, nearest, advice, detail],
    )?;
    Ok(())
}

fn set_keep(conn: &Connection, session_id: &str, keep: bool) -> Result<()> {
    conn.execute(
        "INSERT INTO session_meta (session_id, keep_whole, summary, summary_item_id)
         VALUES (?1, ?2, '', '')
         ON CONFLICT(session_id) DO UPDATE SET keep_whole = excluded.keep_whole",
        params![session_id, i64::from(keep)],
    )?;
    Ok(())
}

fn unpromote(conn: &Connection, session_id: &str) -> Result<()> {
    let item_id: Option<String> = conn
        .query_row(
            "SELECT summary_item_id FROM session_meta WHERE session_id = ?1",
            params![session_id],
            |row| row.get(0),
        )
        .optional()?;
    let Some(item_id) = item_id.filter(|id| !id.is_empty()) else {
        return Ok(());
    };
    let rowid: Option<i64> = conn
        .query_row(
            "SELECT rowid FROM items WHERE item_id = ?1",
            params![item_id],
            |row| row.get(0),
        )
        .optional()?;
    conn.execute(
        "UPDATE items SET marked = 1, current = 0 WHERE item_id = ?1",
        params![item_id],
    )?;
    if let Some(rowid) = rowid {
        conn.execute("DELETE FROM items_fts WHERE rowid = ?1", params![rowid])?;
    }
    Ok(())
}

fn write_summary(conn: &Connection, seq: &AtomicU64, session_id: &str) -> Result<()> {
    let keep: Option<i64> = conn
        .query_row(
            "SELECT keep_whole FROM session_meta WHERE session_id = ?1",
            params![session_id],
            |row| row.get(0),
        )
        .optional()?;
    if keep.unwrap_or(0) == 0 {
        return Ok(());
    }
    let mut stmt = conn.prepare(
        "SELECT said_by, source_date, body FROM session_turns
         WHERE session_id = ?1 ORDER BY id ASC",
    )?;
    let rows = stmt.query_map(params![session_id], |row| {
        Ok(SummaryTurn {
            said_by: row.get(0)?,
            date: row.get(1)?,
            text: row.get(2)?,
        })
    })?;
    let mut turns = Vec::new();
    for row in rows {
        turns.push(row?);
    }
    drop(stmt);
    let summary = summarise_session(session_id, &turns);
    let said_by = turns
        .iter()
        .find(|turn| !turn.said_by.trim().is_empty())
        .map(|turn| turn.said_by.clone())
        .unwrap_or_else(|| "session".to_string());
    let date = turns
        .iter()
        .rev()
        .find(|turn| !turn.date.is_empty())
        .map(|turn| turn.date.clone())
        .unwrap_or_else(|| "unknown".to_string());
    let existing: Option<String> = conn
        .query_row(
            "SELECT summary_item_id FROM session_meta WHERE session_id = ?1",
            params![session_id],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(item_id) = existing.filter(|id| !id.is_empty()) {
        if item_is_live(conn, &item_id)? {
            revise_summary_text(conn, &item_id, &summary)?;
            conn.execute(
                "UPDATE session_meta SET summary = ?2 WHERE session_id = ?1",
                params![session_id, summary],
            )?;
            return Ok(());
        }
    }
    let fact = FactJob {
        item_id: alloc_id(seq, "sum"),
        text: summary.clone(),
        said_by,
        date,
        source: "session-summary".into(),
        pinned: false,
        importance: SUMMARY_IMPORTANCE,
        kind: "summary".into(),
        extractor: "system".into(),
        session_id: session_id.to_string(),
        subject_key: String::new(),
        object_value: String::new(),
    };
    insert_fact(conn, &fact)?;
    conn.execute(
        "UPDATE session_meta SET summary = ?2, summary_item_id = ?3 WHERE session_id = ?1",
        params![session_id, summary, fact.item_id],
    )?;
    Ok(())
}

fn item_is_live(conn: &Connection, item_id: &str) -> Result<bool> {
    let row: Option<i64> = conn
        .query_row(
            "SELECT current FROM items WHERE item_id = ?1",
            params![item_id],
            |row| row.get(0),
        )
        .optional()?;
    Ok(row.unwrap_or(0) == 1)
}

fn revise_summary_text(conn: &Connection, item_id: &str, summary: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO memory_revisions (
            item_id, body, said_by, source_date, source, archived_at, reason, document
         )
         SELECT item_id, body, said_by, source_date, source,
            strftime('%Y-%m-%dT%H:%M:%fZ','now'), 'revised', document
         FROM items WHERE item_id = ?1",
        params![item_id],
    )?;
    let emb = HashingEmbedder.embed(summary);
    let blob: Vec<u8> = emb.values.iter().map(|b| *b as u8).collect();
    let document = serde_json::json!({
        "kind": "summary",
        "text": summary,
        "source": "session-summary",
    })
    .to_string();
    let hash = sha256_hex(document.as_bytes());
    conn.execute(
        "UPDATE items SET body = ?2, summary = ?2, document = ?3, content_hash = ?4,
            emb = ?5, emb_scale = ?6, emb_norm = ?7, marked = 0, current = 1,
            importance = ?8
         WHERE item_id = ?1",
        params![
            item_id,
            summary,
            document,
            hash,
            blob,
            emb.scale,
            emb.norm,
            SUMMARY_IMPORTANCE,
        ],
    )?;
    let rowid: i64 = conn.query_row(
        "SELECT rowid FROM items WHERE item_id = ?1",
        params![item_id],
        |row| row.get(0),
    )?;
    conn.execute("DELETE FROM items_fts WHERE rowid = ?1", params![rowid])?;
    conn.execute(
        "INSERT INTO items_fts(rowid, title, summary, body, said_by, source)
         SELECT ?1, '', ?2, ?2, said_by, source FROM items WHERE item_id = ?3",
        params![rowid, fts_field(summary), item_id],
    )?;
    Ok(())
}

fn fts_field(raw: &str) -> String {
    index_tokens(raw).join(" ")
}

pub(crate) fn enqueue_persist(
    queue: &JobQueue,
    session_id: &str,
    turn_id: &str,
    said_by: &str,
    text: &str,
    date: &str,
) -> Result<()> {
    queue.enqueue(Job::PersistTurn {
        session_id: session_id.to_string(),
        turn_id: turn_id.to_string(),
        said_by: said_by.to_string(),
        text: text.to_string(),
        date: date.to_string(),
        source: "session-ring".into(),
    })
}

pub(crate) fn enqueue_extract(
    queue: &JobQueue,
    session_id: &str,
    said_by: &str,
    text: &str,
    date: &str,
) -> Result<()> {
    queue.enqueue(Job::Extract {
        session_id: session_id.to_string(),
        said_by: said_by.to_string(),
        text: text.to_string(),
        date: date.to_string(),
        source: "session-ring".into(),
    })
}

pub(crate) fn enqueue_summary(queue: &JobQueue, session_id: &str) -> Result<()> {
    queue.enqueue(Job::Summarise {
        session_id: session_id.to_string(),
    })
}

pub(crate) fn enqueue_gap(
    queue: &JobQueue,
    query: &str,
    reason: &str,
    nearest: &str,
    advice: &str,
    detail: &str,
) -> Result<()> {
    queue.enqueue(Job::LogGap {
        query: query.to_string(),
        reason: reason.to_string(),
        nearest: nearest.to_string(),
        advice: advice.to_string(),
        detail: detail.to_string(),
    })
}

pub(crate) fn enqueue_pin(queue: &JobQueue, fact: FactJob) -> Result<()> {
    queue.enqueue(Job::CommitPin(fact))
}

pub(crate) fn enqueue_revise(queue: &JobQueue, old_id: &str, new_fact: FactJob) -> Result<()> {
    queue.enqueue(Job::Revise {
        old_id: old_id.to_string(),
        new_fact,
    })
}

pub(crate) fn enqueue_keep(queue: &JobQueue, session_id: &str, keep: bool) -> Result<()> {
    queue.enqueue(Job::SetKeep {
        session_id: session_id.to_string(),
        keep,
    })?;
    if keep {
        enqueue_summary(queue, session_id)
    } else {
        queue.enqueue(Job::Unpromote {
            session_id: session_id.to_string(),
        })
    }
}
