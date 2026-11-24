// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::card::{parse_cards, provenance_from_document, Card, ContestedView, Source};
use crate::embed::{cosine, dot_i8, HashingEmbedder, Int8Vec, EMB_DIM};
use crate::error::{Error, Result};
use crate::extract::extract_line;
use crate::queue::{self, alloc_id, DbTarget, FactJob, JobQueue, QueueHandle};
use crate::rank::{self, DEFAULT_IMPORTANCE, TRANSCRIPT_IMPORTANCE};
use crate::rrf::fuse;
use crate::session::{SessionRing, Turn};
use crate::text::{index_tokens, is_stopword, tokens};

/// Reads do not wait on the writer. A busy lock returns the ring and any
/// in-memory pins. Five milliseconds is the ceiling the voice budget allows;
/// zero means the call does not spend that ceiling inside SQLite.
const READ_BUSY: Duration = Duration::from_millis(0);
const WRITE_BUSY: Duration = Duration::from_millis(2_000);

/// Published hits. HORBM R1's voice packet is four.
pub const MAX_HITS: usize = 4;
/// Internal pool each leg may contribute before fusion.
pub const POOL: usize = 32;
/// Cormack, Clarke, and Büttcher, SIGIR 2009. The constant the V2 plan uses.
pub const RRF_K: f32 = 60.0;
/// Short-term ring capacity.
pub const RING_CAP: usize = 32;

pub const LAYER_SEMANTIC: &str = "semantic";
pub const LAYER_EPISODE: &str = "episode";
pub const LAYER_DIARY: &str = "diary";

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS items (
    rowid INTEGER PRIMARY KEY,
    item_id TEXT NOT NULL UNIQUE,
    layer TEXT NOT NULL CHECK (layer IN ('semantic', 'episode', 'diary')),
    section TEXT NOT NULL,
    category TEXT NOT NULL DEFAULT '',
    title TEXT NOT NULL DEFAULT '',
    body TEXT NOT NULL,
    summary TEXT NOT NULL DEFAULT '',
    claim_type TEXT NOT NULL DEFAULT '',
    said_by TEXT NOT NULL,
    source_date TEXT NOT NULL,
    source TEXT NOT NULL,
    evidence_strength TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT '',
    marked INTEGER NOT NULL DEFAULT 0,
    emb BLOB NOT NULL,
    emb_scale REAL NOT NULL,
    emb_norm REAL NOT NULL,
    document TEXT NOT NULL DEFAULT '',
    content_hash TEXT NOT NULL DEFAULT '',
    pinned INTEGER NOT NULL DEFAULT 0,
    importance REAL NOT NULL DEFAULT 0,
    kind TEXT NOT NULL DEFAULT '',
    session_id TEXT NOT NULL DEFAULT '',
    current INTEGER NOT NULL DEFAULT 1,
    extractor TEXT NOT NULL DEFAULT '',
    subject_key TEXT NOT NULL DEFAULT '',
    object_value TEXT NOT NULL DEFAULT '',
    last_access TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS items_layer ON items(layer);
CREATE TABLE IF NOT EXISTS meta (
    k TEXT PRIMARY KEY,
    v INTEGER NOT NULL
);
INSERT OR IGNORE INTO meta(k, v) VALUES ('seq', 0);
CREATE VIRTUAL TABLE IF NOT EXISTS items_fts USING fts5(
    title,
    summary,
    body,
    said_by,
    source,
    tokenize = 'unicode61'
);
CREATE VIRTUAL TABLE IF NOT EXISTS items_fts_vocab USING fts5vocab(items_fts, 'row');
CREATE TABLE IF NOT EXISTS archive (
    rowid INTEGER PRIMARY KEY,
    item_id TEXT NOT NULL,
    layer TEXT NOT NULL,
    section TEXT NOT NULL,
    category TEXT NOT NULL DEFAULT '',
    title TEXT NOT NULL DEFAULT '',
    body TEXT NOT NULL,
    summary TEXT NOT NULL DEFAULT '',
    claim_type TEXT NOT NULL DEFAULT '',
    said_by TEXT NOT NULL,
    source_date TEXT NOT NULL,
    source TEXT NOT NULL,
    evidence_strength TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT '',
    reason TEXT NOT NULL,
    emb BLOB NOT NULL,
    emb_scale REAL NOT NULL,
    emb_norm REAL NOT NULL,
    document TEXT NOT NULL DEFAULT '',
    content_hash TEXT NOT NULL DEFAULT ''
);
CREATE TABLE IF NOT EXISTS card_revisions (
    revision_id INTEGER PRIMARY KEY,
    item_id TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    document TEXT NOT NULL,
    claim_type TEXT NOT NULL DEFAULT '',
    evidence_strength TEXT NOT NULL DEFAULT '',
    said_by TEXT NOT NULL DEFAULT '',
    source_date TEXT NOT NULL DEFAULT '',
    source TEXT NOT NULL DEFAULT '',
    archived_at TEXT NOT NULL,
    reason TEXT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS card_revisions_once
    ON card_revisions(item_id, content_hash, reason);
CREATE TABLE IF NOT EXISTS gaps (
    id INTEGER PRIMARY KEY,
    query TEXT NOT NULL,
    asked_at TEXT NOT NULL,
    reason TEXT NOT NULL,
    nearest TEXT NOT NULL,
    advice TEXT NOT NULL,
    detail TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS session_turns (
    id INTEGER PRIMARY KEY,
    session_id TEXT NOT NULL,
    turn_id TEXT NOT NULL UNIQUE,
    said_by TEXT NOT NULL,
    body TEXT NOT NULL,
    source_date TEXT NOT NULL,
    source TEXT NOT NULL DEFAULT 'session-ring',
    recorded_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS session_turns_session ON session_turns(session_id);
CREATE VIRTUAL TABLE IF NOT EXISTS session_turns_fts USING fts5(
    body,
    said_by,
    source,
    tokenize = 'unicode61'
);
CREATE TABLE IF NOT EXISTS session_meta (
    session_id TEXT PRIMARY KEY,
    keep_whole INTEGER NOT NULL DEFAULT 0,
    summary TEXT NOT NULL DEFAULT '',
    summary_item_id TEXT NOT NULL DEFAULT ''
);
CREATE TABLE IF NOT EXISTS memory_jobs (
    id INTEGER PRIMARY KEY,
    kind TEXT NOT NULL,
    payload TEXT NOT NULL,
    enqueued_at TEXT NOT NULL,
    finished_at TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS memory_revisions (
    revision_id INTEGER PRIMARY KEY,
    item_id TEXT NOT NULL,
    body TEXT NOT NULL,
    said_by TEXT NOT NULL,
    source_date TEXT NOT NULL,
    source TEXT NOT NULL,
    archived_at TEXT NOT NULL,
    reason TEXT NOT NULL,
    document TEXT NOT NULL DEFAULT ''
);
"#;

const SCHEMA_VERSION: i32 = 5;
/// Skip a term that hits more than 2% of a diary once the diary is large
/// enough for that fraction to be a real scan. BM25 of a near-universal
/// term does not separate rows.
const DF_FRACTION_DEN: i64 = 50;
const DF_PLAN_MIN_ROWS: i64 = 64;
const MAX_LEXICAL_TERMS: usize = 8;

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Hit {
    pub item_id: String,
    pub layer: String,
    /// `memory` is what the user told us (ring, episode, diary).
    /// `knowledge` is a card in the core pack or a shard.
    /// The two are never fused into one score.
    pub pool: String,
    pub title: String,
    pub text: String,
    pub said_by: String,
    pub date: String,
    pub source: String,
    pub claim_type: String,
    pub evidence: String,
    pub sources: Vec<Source>,
    pub contested_views: Vec<ContestedView>,
    pub score: f32,
    /// User pin. Knowledge hits are false.
    pub pinned: bool,
    /// `0` on a knowledge hit. Pins are 1. Candidates carry the extractor score.
    pub importance: f32,
    /// `user` for a pin, `system` for an extracted candidate, empty otherwise.
    pub extractor: String,
    /// `pin`, `candidate`, `summary`, `diary`, `transcript`, or empty.
    pub kind: String,
    /// Not part of the JSON. Used only for the memory rerank.
    #[serde(skip)]
    pub last_access: String,
    /// Title, gist, body, speaker, source, and contested views. Used to
    /// decide whether a hit actually covers the query. Not part of the JSON.
    #[serde(skip)]
    pub haystack: String,
}

impl Hit {
    pub(crate) fn make(
        item_id: impl Into<String>,
        layer: &str,
        title: impl Into<String>,
        text: impl Into<String>,
        body: impl Into<String>,
        said_by: impl Into<String>,
        date: impl Into<String>,
        source: impl Into<String>,
        claim_type: impl Into<String>,
        evidence: impl Into<String>,
        sources: Vec<Source>,
        contested_views: Vec<ContestedView>,
        score: f32,
    ) -> Self {
        let title = title.into();
        let text = text.into();
        let body = body.into();
        let said_by_raw = said_by.into();
        let date_raw = date.into();
        let source_raw = source.into();
        let mut haystack = String::new();
        for part in [
            title.as_str(),
            text.as_str(),
            body.as_str(),
            said_by_raw.as_str(),
            source_raw.as_str(),
        ] {
            haystack.push_str(part);
            haystack.push(' ');
        }
        for view in &contested_views {
            haystack.push_str(&view.label);
            haystack.push(' ');
            haystack.push_str(&view.summary);
            haystack.push(' ');
        }
        Self {
            item_id: item_id.into(),
            layer: layer.to_string(),
            pool: pool_of(layer).to_string(),
            title,
            text,
            said_by: unknown_if_empty(&said_by_raw),
            date: unknown_if_empty(&date_raw),
            source: unknown_if_empty(&source_raw),
            claim_type: claim_type.into(),
            evidence: evidence.into(),
            sources,
            contested_views,
            score,
            pinned: false,
            importance: 0.0,
            extractor: String::new(),
            kind: String::new(),
            last_access: String::new(),
            haystack,
        }
    }
}

/// `memory` is a turn, an episode, or a diary line. Everything else is a card.
pub fn pool_of(layer: &str) -> &'static str {
    match layer {
        "session" | "episode" | "diary" | "transcript" => "memory",
        _ => "knowledge",
    }
}

/// A fact the caller can already search, before the worker commits it.
struct LiveFact {
    id: String,
    text: String,
    said_by: String,
    date: String,
    source: String,
    pinned: bool,
    importance: f32,
    extractor: String,
    kind: String,
    layer: String,
    emb: Int8Vec,
}

/// One archived memory fact. Append-only. Search does not read this.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct MemoryRevision {
    pub item_id: String,
    pub text: String,
    pub said_by: String,
    pub date: String,
    pub source: String,
    pub reason: String,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct SearchResponse {
    pub query: String,
    pub hits: Vec<Hit>,
    pub took_us: u32,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct DiaryEntry {
    pub item_id: String,
    pub date: String,
    pub said_by: String,
    pub source: String,
    pub text: String,
    pub marked: bool,
}

/// A stored row an admin `forget` moved out of retrieval.
/// The text is kept. Search does not read this table.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct ArchivedRow {
    pub item_id: String,
    pub layer: String,
    pub text: String,
    pub said_by: String,
    pub date: String,
    pub source: String,
    pub reason: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct IngestReport {
    pub files_read: usize,
    pub upserted: usize,
    pub revised: usize,
    pub unchanged: usize,
    pub skipped: Vec<String>,
}

/// One archived card version. Append-only. Search does not read this.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct CardRevision {
    pub item_id: String,
    pub content_hash: String,
    pub document: String,
    pub archived_at: String,
    pub reason: String,
}

/// The same query, counted. The latest advice is the one to act on.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct GapRank {
    pub query: String,
    pub times: i64,
    pub latest: String,
    pub advice: String,
    pub detail: String,
    pub nearest: String,
}

struct Arena {
    rowid: Vec<i64>,
    item_id: Vec<String>,
    scale: Vec<f32>,
    norm: Vec<f32>,
    /// True for episode and diary rows. Semantic cards are knowledge.
    memory: Vec<bool>,
    /// Contiguous int8 matrix, stride [`EMB_DIM`].
    emb: Vec<i8>,
}

impl Arena {
    fn empty() -> Self {
        Self {
            rowid: Vec::new(),
            item_id: Vec::new(),
            scale: Vec::new(),
            norm: Vec::new(),
            memory: Vec::new(),
            emb: Vec::new(),
        }
    }

    fn len(&self) -> usize {
        self.rowid.len()
    }
}

/// One SQLite file: semantic cards, remembered episodes, and the diary.
///
/// The worker thread owns the writes that must not stall a voice turn.
/// `conn` is the reader. Its busy timeout is zero.
pub struct Store {
    jobs: JobQueue,
    conn: Connection,
    arena: Arena,
    ring: SessionRing,
    embedder: HashingEmbedder,
    overlay: Vec<LiveFact>,
    hidden: HashSet<String>,
    session_id: String,
    keep_whole: HashMap<String, bool>,
    seq: Arc<AtomicU64>,
}

impl Store {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        if let Some(parent) = path.as_ref().parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }
        let target = DbTarget {
            uri: path.as_ref().to_string_lossy().into_owned(),
            memory: false,
        };
        Self::open_target(target)
    }

    pub fn open_in_memory() -> Result<Self> {
        static N: AtomicU64 = AtomicU64::new(1);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let target = DbTarget {
            uri: format!(
                "file:horbm-mem-{}-{}?mode=memory&cache=shared",
                std::process::id(),
                n
            ),
            memory: true,
        };
        Self::open_target(target)
    }

    fn open_target(target: DbTarget) -> Result<Self> {
        let conn = queue::open_connection(&target)?;
        conn.busy_timeout(WRITE_BUSY)?;
        prepare_schema(&conn)?;
        conn.busy_timeout(READ_BUSY)?;
        let seq_v: i64 =
            conn.query_row("SELECT v FROM meta WHERE k = 'seq'", [], |row| row.get(0))?;
        let seq = Arc::new(AtomicU64::new(seq_v.max(0) as u64));
        let keep_whole = load_keep_whole(&conn)?;
        // Read the arena before the worker opens its connection so the two
        // connections are not taking the first lock at the same time.
        let arena = read_arena(&conn)?;
        let jobs = JobQueue::spawn(target, Arc::clone(&seq))?;
        Ok(Self {
            jobs,
            conn,
            arena,
            ring: SessionRing::new(RING_CAP),
            embedder: HashingEmbedder,
            overlay: Vec::new(),
            hidden: HashSet::new(),
            session_id: "default".into(),
            keep_whole,
            seq,
        })
    }

    /// Push a turn into the RAM ring and enqueue a transcript append.
    ///
    /// Returns when the ring is updated. The SQLite write, extraction, and
    /// any session summary run on the worker.
    pub fn push_turn(
        &mut self,
        text: impl Into<String>,
        said_by: impl Into<String>,
        date: impl Into<String>,
    ) -> Turn {
        let text = text.into();
        let said_by = said_by.into();
        let date = date.into();
        let turn = self.ring.push(text.clone(), said_by.clone(), date.clone());
        let turn_id = alloc_id(&self.seq, "turn");
        let session_id = self.session_id.clone();
        let _ = queue::enqueue_persist(&self.jobs, &session_id, &turn_id, &said_by, &text, &date);
        let _ = queue::enqueue_extract(&self.jobs, &session_id, &said_by, &text, &date);
        if self.keep_whole.get(&session_id).copied().unwrap_or(false) {
            let _ = queue::enqueue_summary(&self.jobs, &session_id);
        }
        turn
    }

    pub fn set_session(&mut self, session_id: &str) -> Result<()> {
        if session_id.trim().is_empty() {
            return Err(Error::Invalid("session_id is empty".into()));
        }
        self.session_id = session_id.to_string();
        Ok(())
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Keep the whole session: a background summary, and the transcript
    /// stays in live retrieval. `false` leaves candidates, and does not
    /// promote the raw transcript. Default is off. The text is not deleted
    /// when the switch turns off.
    pub fn keep_whole_session(&mut self, session_id: &str, keep: bool) -> Result<()> {
        if session_id.trim().is_empty() {
            return Err(Error::Invalid("session_id is empty".into()));
        }
        self.keep_whole.insert(session_id.to_string(), keep);
        if let Some(item_id) = self.summary_item_id(session_id)? {
            if keep {
                self.hidden.remove(&item_id);
            } else {
                self.hidden.insert(item_id);
            }
        }
        queue::enqueue_keep(&self.jobs, session_id, keep)
    }

    pub fn session_summary(&self, session_id: &str) -> Result<String> {
        let summary: Option<String> = self
            .conn
            .query_row(
                "SELECT summary FROM session_meta WHERE session_id = ?1",
                params![session_id],
                |row| row.get(0),
            )
            .optional()?;
        Ok(summary.unwrap_or_default())
    }

    pub fn pause_worker(&self) {
        self.jobs.pause();
    }

    pub fn resume_worker(&self) {
        self.jobs.resume();
    }

    pub fn queue_len(&self) -> usize {
        self.jobs.pending()
    }

    pub fn queue_handle(&self) -> QueueHandle {
        self.jobs.handle()
    }

    /// Wait until the worker has finished every queued job, then reload the
    /// vector arena. Resumes a paused worker. Not on the voice path.
    pub fn drain(&mut self) -> Result<()> {
        self.jobs.resume();
        let started = Instant::now();
        while self.jobs.pending() > 0 {
            if started.elapsed() > Duration::from_secs(60) {
                return Err(Error::Invalid(format!(
                    "memory queue still has {} jobs after 60s",
                    self.jobs.pending()
                )));
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        if let Some(err) = self.jobs.take_error() {
            return Err(Error::Invalid(err));
        }
        self.reload_arena()?;
        self.overlay.clear();
        self.hidden.clear();
        self.keep_whole = load_keep_whole(&self.conn)?;
        Ok(())
    }

    pub fn schema_version(&self) -> Result<i32> {
        let version = self
            .conn
            .pragma_query_value(None, "user_version", |row| row.get(0))?;
        Ok(version)
    }

    pub fn session_len(&self) -> usize {
        self.ring.len()
    }

    /// Ingest every `*.json` file under `dir`. Card files upsert. JSON that
    /// is not a card is skipped and named in the report.
    pub fn ingest_dir(&mut self, dir: impl AsRef<Path>) -> Result<IngestReport> {
        let dir = dir.as_ref();
        if !dir.is_dir() {
            return Err(Error::Invalid(format!(
                "{} is not a directory",
                dir.display()
            )));
        }
        let mut files = Vec::new();
        collect_json(dir, &mut files)?;
        files.sort();
        let report = self.with_write(|store| {
            store.in_tx(|store| {
                let mut report = IngestReport {
                    files_read: 0,
                    upserted: 0,
                    revised: 0,
                    unchanged: 0,
                    skipped: Vec::new(),
                };
                for path in &files {
                    let raw = fs::read_to_string(path)?;
                    let value: serde_json::Value = serde_json::from_str(&raw)
                        .map_err(|e| Error::Invalid(format!("{}: {e}", path.display())))?;
                    report.files_read += 1;
                    let hint = path.to_string_lossy().to_string();
                    let cards = parse_cards(&value, &hint)?;
                    if cards.is_empty() {
                        report.skipped.push(hint);
                        continue;
                    }
                    for card in cards {
                        match store.upsert_card_tx(&card)? {
                            UpsertKind::Inserted | UpsertKind::Restored => {}
                            UpsertKind::Revised => report.revised += 1,
                            UpsertKind::Unchanged => report.unchanged += 1,
                        }
                        report.upserted += 1;
                    }
                }
                Ok(report)
            })
        })?;
        self.reload_arena()?;
        Ok(report)
    }

    pub fn upsert_card(&mut self, card: &Card) -> Result<()> {
        self.upsert_cards(std::slice::from_ref(card))
    }

    pub fn upsert_cards(&mut self, cards: &[Card]) -> Result<()> {
        self.with_write(|store| {
            store.in_tx(|store| {
                for card in cards {
                    store.upsert_card_tx(card)?;
                }
                Ok(())
            })
        })?;
        self.reload_arena()?;
        Ok(())
    }

    /// Pin one fact. The caller can search it immediately. The SQLite commit
    /// runs on the worker, so a voice turn is not waiting on `reload_arena`.
    pub fn remember(
        &mut self,
        text: &str,
        said_by: &str,
        date: &str,
        source: &str,
    ) -> Result<String> {
        require_fields(text, said_by, date, source)?;
        let id = alloc_id(&self.seq, "ep");
        let (subject_key, object_value) = pin_subject(text, said_by, date, source);
        let emb = self.embedder.embed(text);
        self.overlay.push(LiveFact {
            id: id.clone(),
            text: text.to_string(),
            said_by: said_by.to_string(),
            date: date.to_string(),
            source: source.to_string(),
            pinned: true,
            importance: 1.0,
            extractor: "user".into(),
            kind: "pin".into(),
            layer: LAYER_EPISODE.into(),
            emb,
        });
        queue::enqueue_pin(
            &self.jobs,
            FactJob {
                item_id: id.clone(),
                text: text.to_string(),
                said_by: said_by.to_string(),
                date: date.to_string(),
                source: source.to_string(),
                pinned: true,
                importance: 1.0,
                kind: "pin".into(),
                extractor: "user".into(),
                session_id: self.session_id.clone(),
                subject_key,
                object_value,
            },
        )?;
        Ok(id)
    }

    /// Archive `old_id` (`reason = revised`) and insert a new current fact.
    /// The old text stays in `items`, `archive`, and `memory_revisions`.
    pub fn revise_fact(
        &mut self,
        old_id: &str,
        text: &str,
        said_by: &str,
        date: &str,
        source: &str,
    ) -> Result<String> {
        require_fields(text, said_by, date, source)?;
        let known = self.overlay.iter().any(|fact| fact.id == old_id)
            || self.existing_rowid(old_id)?.is_some();
        if !known {
            return Err(Error::NotFound(old_id.to_string()));
        }
        let id = alloc_id(&self.seq, "ep");
        let (subject_key, object_value) = pin_subject(text, said_by, date, source);
        let emb = self.embedder.embed(text);
        self.hidden.insert(old_id.to_string());
        self.overlay.retain(|fact| fact.id != old_id);
        self.overlay.push(LiveFact {
            id: id.clone(),
            text: text.to_string(),
            said_by: said_by.to_string(),
            date: date.to_string(),
            source: source.to_string(),
            pinned: true,
            importance: 1.0,
            extractor: "user".into(),
            kind: "pin".into(),
            layer: LAYER_EPISODE.into(),
            emb,
        });
        queue::enqueue_revise(
            &self.jobs,
            old_id,
            FactJob {
                item_id: id.clone(),
                text: text.to_string(),
                said_by: said_by.to_string(),
                date: date.to_string(),
                source: source.to_string(),
                pinned: true,
                importance: 1.0,
                kind: "pin".into(),
                extractor: "user".into(),
                session_id: self.session_id.clone(),
                subject_key,
                object_value,
            },
        )?;
        Ok(id)
    }

    /// Append one diary entry. There is no update path.
    pub fn diary_add(
        &mut self,
        text: &str,
        date: &str,
        said_by: &str,
        source: &str,
    ) -> Result<String> {
        let said_by = if said_by.is_empty() { "HORBM" } else { said_by };
        let source = if source.is_empty() { "diary" } else { source };
        require_fields(text, said_by, date, source)?;
        let emb = self.embedder.embed(text);
        let id = self.with_write(|store| {
            store.in_tx(|store| {
                let id = store.next_id("diary")?;
                let document = memory_document(text, said_by, date, source);
                store.insert_item(
                    &id,
                    LAYER_DIARY,
                    "diary",
                    "entry",
                    date,
                    text,
                    text,
                    "user-said",
                    said_by,
                    date,
                    source,
                    "",
                    "",
                    &document,
                    "",
                    text,
                    &emb,
                )?;
                store.conn.execute(
                    "UPDATE items SET kind = 'diary', importance = ?2, pinned = 0,
                        extractor = '', current = 1, session_id = ?3
                     WHERE item_id = ?1",
                    params![id, DEFAULT_IMPORTANCE, store.session_id],
                )?;
                Ok(id)
            })
        })?;
        self.reload_arena()?;
        Ok(id)
    }

    pub fn diary_list(&self) -> Result<Vec<DiaryEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT item_id, source_date, said_by, source, body, marked
             FROM items WHERE layer = 'diary' ORDER BY rowid ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(DiaryEntry {
                item_id: row.get(0)?,
                date: row.get(1)?,
                said_by: row.get(2)?,
                source: row.get(3)?,
                text: row.get(4)?,
                marked: row.get::<_, i64>(5)? != 0,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Keep the row, drop it from retrieval.
    pub fn mark(&mut self, item_id: &str) -> Result<()> {
        self.drain()?;
        let rowid = self.rowid_of(item_id)?;
        self.with_write(|store| {
            store.in_tx(|store| {
                store.conn.execute(
                    "UPDATE items SET marked = 1 WHERE rowid = ?1",
                    params![rowid],
                )?;
                store
                    .conn
                    .execute("DELETE FROM items_fts WHERE rowid = ?1", params![rowid])?;
                Ok(())
            })
        })?;
        self.reload_arena()?;
        Ok(())
    }

    /// Admin tool. Hide one stored row. The text moves to `archive` and
    /// leaves the retrieval index. The JSON card on disk is not touched.
    pub fn forget(&mut self, item_id: &str) -> Result<()> {
        self.drain()?;
        let rowid = self.rowid_of(item_id)?;
        self.with_write(|store| store.in_tx(|store| store.archive_live_row(rowid, "forget")))?;
        self.reload_arena()?;
        Ok(())
    }

    /// Rows held out of retrieval. Order is archive insertion order.
    pub fn archive_list(&self) -> Result<Vec<ArchivedRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT item_id, layer, body, said_by, source_date, source, reason
             FROM archive ORDER BY rowid ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(ArchivedRow {
                item_id: row.get(0)?,
                layer: row.get(1)?,
                text: row.get(2)?,
                said_by: row.get(3)?,
                date: row.get(4)?,
                source: row.get(5)?,
                reason: row.get(6)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn search(&self, query: &str) -> Result<SearchResponse> {
        self.search_filtered(query, false)
    }

    /// Ring, episodes, and diary only. Knowledge cards in this file are left out
    /// so a caller can rank them on their own.
    pub fn search_memory(&self, query: &str) -> Result<SearchResponse> {
        self.search_filtered(query, true)
    }

    fn search_filtered(&self, query: &str, memory_only: bool) -> Result<SearchResponse> {
        let started = Instant::now();
        let mut lexical = match self.lexical_ids(query, memory_only) {
            Ok(ids) => ids,
            Err(err) if is_busy_err(&err) => Vec::new(),
            Err(err) => return Err(err),
        };
        merge_unique(&mut lexical, self.overlay_lexical(query));
        match self.transcript_lexical(query) {
            Ok(transcript) => merge_unique(&mut lexical, transcript),
            Err(err) if is_busy_err(&err) => {}
            Err(err) => return Err(err),
        }
        lexical.retain(|id| !self.hidden.contains(id));
        let dense = self.dense_ids(query, memory_only);
        let ring_lex = self.ring_lexical_ids(query);
        // The lexical list is counted twice so a rare exact term beats a tied vector neighbor.
        let fused = fuse(&[lexical.clone(), lexical, dense, ring_lex], RRF_K);
        let mut hits = Vec::new();
        for (id, score) in fused.into_iter().take(POOL) {
            if let Some(hit) = self.hydrate(&id, score)? {
                if memory_only && hit.pool != "memory" {
                    continue;
                }
                hits.push(hit);
            }
        }
        rerank_memory(&mut hits);
        hits.truncate(MAX_HITS);
        let took_us = u32::try_from(started.elapsed().as_micros()).unwrap_or(u32::MAX);
        Ok(SearchResponse {
            query: query.to_string(),
            hits,
            took_us,
        })
    }

    /// Search the append-only transcript. This does not require keep-whole.
    pub fn search_transcript(&self, query: &str) -> Result<SearchResponse> {
        let started = Instant::now();
        let ids = match transcript_match(&self.conn, query, None) {
            Ok(ids) => ids,
            Err(err) if is_busy_err(&err) => Vec::new(),
            Err(err) => return Err(err),
        };
        let mut hits = Vec::new();
        for id in ids {
            if let Some(hit) = self.hydrate_turn(&id, 0.0)? {
                hits.push(hit);
                if hits.len() == MAX_HITS {
                    break;
                }
            }
        }
        let took_us = u32::try_from(started.elapsed().as_micros()).unwrap_or(u32::MAX);
        Ok(SearchResponse {
            query: query.to_string(),
            hits,
            took_us,
        })
    }

    pub fn transcript_len(&self) -> Result<usize> {
        let n: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM session_turns", [], |row| row.get(0))?;
        Ok(n as usize)
    }

    pub fn memory_revisions(&self, item_id: &str) -> Result<Vec<MemoryRevision>> {
        let mut stmt = self.conn.prepare(
            "SELECT item_id, body, said_by, source_date, source, reason
             FROM memory_revisions WHERE item_id = ?1 ORDER BY revision_id ASC",
        )?;
        let rows = stmt.query_map(params![item_id], |row| {
            Ok(MemoryRevision {
                item_id: row.get(0)?,
                text: row.get(1)?,
                said_by: row.get(2)?,
                date: row.get(3)?,
                source: row.get(4)?,
                reason: row.get(5)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// True when `snippet` is still stored in a live row, the archive, a
    /// memory revision, or the transcript.
    pub fn retains_text(&self, snippet: &str) -> Result<bool> {
        let like = format!("%{snippet}%");
        let n: i64 = self.conn.query_row(
            "SELECT
                (SELECT COUNT(*) FROM items WHERE body LIKE ?1 OR summary LIKE ?1 OR document LIKE ?1)
              + (SELECT COUNT(*) FROM archive WHERE body LIKE ?1 OR document LIKE ?1)
              + (SELECT COUNT(*) FROM memory_revisions WHERE body LIKE ?1 OR document LIKE ?1)
              + (SELECT COUNT(*) FROM session_turns WHERE body LIKE ?1)",
            params![like],
            |row| row.get(0),
        )?;
        Ok(n > 0)
    }

    pub fn count_extracted(&self) -> Result<usize> {
        let n: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM items
             WHERE extractor = 'system' AND kind = 'candidate' AND marked = 0 AND current = 1",
            [],
            |row| row.get(0),
        )?;
        Ok(n as usize)
    }

    pub(crate) fn enqueue_gap(
        &self,
        query: &str,
        reason: &str,
        nearest_json: &str,
        advice: &str,
        detail: &str,
    ) {
        let _ = queue::enqueue_gap(&self.jobs, query, reason, nearest_json, advice, detail);
    }

    /// Append one gap. Rows are never deleted.
    pub fn log_gap(
        &self,
        query: &str,
        reason: &str,
        nearest_json: &str,
        advice: &str,
        detail: &str,
    ) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO gaps (query, asked_at, reason, nearest, advice, detail)
             VALUES (?1, strftime('%Y-%m-%dT%H:%M:%fZ','now'), ?2, ?3, ?4, ?5)",
            params![query, reason, nearest_json, advice, detail],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Queries we could not answer, most often first.
    pub fn gap_queue(&self) -> Result<Vec<GapRank>> {
        let mut stmt = self.conn.prepare(
            "SELECT g.query, c.n, g.asked_at, g.advice, g.detail, g.nearest
             FROM gaps g
             JOIN (
                SELECT lower(trim(query)) AS q, COUNT(*) AS n, MAX(id) AS max_id
                FROM gaps
                GROUP BY q
             ) c ON g.id = c.max_id
             ORDER BY c.n DESC, g.asked_at DESC, g.id DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(GapRank {
                query: row.get(0)?,
                times: row.get(1)?,
                latest: row.get(2)?,
                advice: row.get(3)?,
                detail: row.get(4)?,
                nearest: row.get(5)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Older card versions, oldest first. The live row is not in this list.
    pub fn revisions_for(&self, item_id: &str) -> Result<Vec<CardRevision>> {
        let mut stmt = self.conn.prepare(
            "SELECT item_id, content_hash, document, archived_at, reason
             FROM card_revisions WHERE item_id = ?1 ORDER BY revision_id ASC",
        )?;
        let rows = stmt.query_map(params![item_id], |row| {
            Ok(CardRevision {
                item_id: row.get(0)?,
                content_hash: row.get(1)?,
                document: row.get(2)?,
                archived_at: row.get(3)?,
                reason: row.get(4)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn revision_count(&self) -> Result<usize> {
        let n: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM card_revisions", [], |r| r.get(0))?;
        Ok(n as usize)
    }

    pub fn document_of(&self, item_id: &str) -> Result<Option<String>> {
        let row = self
            .conn
            .query_row(
                "SELECT document FROM items WHERE item_id = ?1 AND marked = 0",
                params![item_id],
                |r| r.get::<_, String>(0),
            )
            .optional()?;
        Ok(row.filter(|doc| !doc.is_empty()))
    }

    pub fn documents(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT document FROM items WHERE marked = 0 AND document != '' ORDER BY rowid",
        )?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn active_count(&self) -> Result<usize> {
        let n: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM items WHERE marked = 0", [], |r| {
                    r.get(0)
                })?;
        Ok(n as usize)
    }

    fn upsert_card_tx(&self, card: &Card) -> Result<UpsertKind> {
        let emb = self.embedder.embed(&card.index_text());
        if let Some((rowid, hash, marked)) = self.existing_card(&card.id)? {
            if hash == card.content_hash {
                if marked == 0 {
                    return Ok(UpsertKind::Unchanged);
                }
                self.conn.execute(
                    "UPDATE items SET marked = 0 WHERE rowid = ?1",
                    params![rowid],
                )?;
                self.insert_fts(
                    rowid,
                    &card.title,
                    &card.summary,
                    &card.lexical_body(),
                    &card.said_by,
                    &card.source,
                )?;
                return Ok(UpsertKind::Restored);
            }
            self.archive_revision(rowid, "replaced")?;
            self.conn
                .execute("DELETE FROM items_fts WHERE rowid = ?1", params![rowid])?;
            self.conn.execute(
                "UPDATE items SET
                    layer = 'semantic', section = ?1, category = ?2, title = ?3,
                    body = ?4, summary = ?5, claim_type = ?6, said_by = ?7,
                    source_date = ?8, source = ?9, evidence_strength = ?10,
                    status = ?11, marked = 0, emb = ?12, emb_scale = ?13, emb_norm = ?14,
                    document = ?15, content_hash = ?16
                 WHERE rowid = ?17",
                params![
                    card.section,
                    card.category,
                    card.title,
                    card.body,
                    card.summary,
                    card.claim_type,
                    card.said_by,
                    card.date,
                    card.source,
                    card.evidence_strength,
                    card.status,
                    as_blob(&emb),
                    emb.scale,
                    emb.norm,
                    card.document,
                    card.content_hash,
                    rowid,
                ],
            )?;
            self.insert_fts(
                rowid,
                &card.title,
                &card.summary,
                &card.lexical_body(),
                &card.said_by,
                &card.source,
            )?;
            return Ok(UpsertKind::Revised);
        }
        self.insert_item(
            &card.id,
            LAYER_SEMANTIC,
            &card.section,
            &card.category,
            &card.title,
            &card.body,
            &card.summary,
            &card.claim_type,
            &card.said_by,
            &card.date,
            &card.source,
            &card.evidence_strength,
            &card.status,
            &card.document,
            &card.content_hash,
            &card.lexical_body(),
            &emb,
        )?;
        Ok(UpsertKind::Inserted)
    }

    fn insert_item(
        &self,
        item_id: &str,
        layer: &str,
        section: &str,
        category: &str,
        title: &str,
        body: &str,
        summary: &str,
        claim_type: &str,
        said_by: &str,
        date: &str,
        source: &str,
        evidence_strength: &str,
        status: &str,
        document: &str,
        content_hash: &str,
        fts_body: &str,
        emb: &Int8Vec,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO items (
                item_id, layer, section, category, title, body, summary, claim_type,
                said_by, source_date, source, evidence_strength, status, marked,
                emb, emb_scale, emb_norm, document, content_hash
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, 0, ?14, ?15, ?16, ?17, ?18)",
            params![
                item_id,
                layer,
                section,
                category,
                title,
                body,
                summary,
                claim_type,
                said_by,
                date,
                source,
                evidence_strength,
                status,
                as_blob(emb),
                emb.scale,
                emb.norm,
                document,
                content_hash,
            ],
        )?;
        let rowid = self.conn.last_insert_rowid();
        self.insert_fts(rowid, title, summary, fts_body, said_by, source)?;
        Ok(())
    }

    fn insert_fts(
        &self,
        rowid: i64,
        title: &str,
        summary: &str,
        body: &str,
        said_by: &str,
        source: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO items_fts(rowid, title, summary, body, said_by, source)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                rowid,
                fts_field(title),
                fts_field(summary),
                fts_field(body),
                fts_field(said_by),
                fts_field(source),
            ],
        )?;
        Ok(())
    }

    fn lexical_ids(&self, query: &str, memory_only: bool) -> Result<Vec<String>> {
        let Some(match_q) = self.planned_match(query)? else {
            return Ok(Vec::new());
        };
        let layer_sql = if memory_only {
            "AND items.layer IN ('episode', 'diary') AND items.current = 1"
        } else {
            "AND items.current = 1"
        };
        let sql = format!(
            "SELECT items.item_id
             FROM items_fts
             JOIN items ON items.rowid = items_fts.rowid
             WHERE items_fts MATCH ?1 AND items.marked = 0 {layer_sql}
             ORDER BY rank
             LIMIT ?2"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params![match_q, POOL as i64], |row| row.get::<_, String>(0))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Keep the rare terms. A term in more than 1/50 of a large diary is
    /// dropped: its IDF is too small to be worth a full postings scan.
    fn planned_match(&self, query: &str) -> Result<Option<String>> {
        match_query(&self.conn, self.arena.len() as i64, query)
    }

    fn dense_ids(&self, query: &str, memory_only: bool) -> Vec<String> {
        let q = self.embedder.embed(query);
        if q.norm == 0.0 {
            return Vec::new();
        }
        let mut qv = [0i8; EMB_DIM];
        qv.copy_from_slice(&q.values);
        let avx2 = crate::embed::avx2_available();
        let mut top: Vec<(u32, f32)> = Vec::with_capacity(POOL);
        let mut worst = 0f32;
        let mut worst_at = 0usize;
        let emb = &self.arena.emb;
        let n = self.arena.len();
        for i in 0..n {
            if self.hidden.contains(&self.arena.item_id[i]) {
                continue;
            }
            if memory_only && !self.arena.memory[i] {
                continue;
            }
            let start = i * EMB_DIM;
            let row = &emb[start..start + EMB_DIM];
            let dot = dot_i8(&qv, row, avx2);
            let norm = self.arena.norm[i];
            if norm == 0.0 || dot <= 0 {
                continue;
            }
            let score = (dot as f32) * q.scale * self.arena.scale[i] / (q.norm * norm);
            if top.len() < POOL {
                if top.is_empty() || score < worst {
                    worst = score;
                    worst_at = top.len();
                }
                top.push((i as u32, score));
            } else if score > worst {
                top[worst_at] = (i as u32, score);
                worst = f32::MAX;
                for (j, (_, s)) in top.iter().enumerate() {
                    if *s < worst {
                        worst = *s;
                        worst_at = j;
                    }
                }
            }
        }
        let mut scored: Vec<(String, f32)> = top
            .into_iter()
            .map(|(i, score)| (self.arena.item_id[i as usize].clone(), score))
            .collect();
        for turn in self.ring.turns() {
            let emb = self.embedder.embed(&turn.text);
            let score = cosine(&q, &emb.values, emb.scale, emb.norm);
            if score > 0.0 {
                scored.push((turn.id.clone(), score));
            }
        }
        for fact in &self.overlay {
            if self.hidden.contains(&fact.id) {
                continue;
            }
            let score = cosine(&q, &fact.emb.values, fact.emb.scale, fact.emb.norm);
            if score > 0.0 {
                scored.push((fact.id.clone(), score));
            }
        }
        scored.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        scored.truncate(POOL);
        scored.into_iter().map(|(id, _)| id).collect()
    }

    fn ring_lexical_ids(&self, query: &str) -> Vec<String> {
        let q_tokens = tokens(query);
        if q_tokens.is_empty() {
            return Vec::new();
        }
        let mut scored: Vec<(&str, usize)> = Vec::new();
        for turn in self.ring.turns() {
            let have = tokens(&turn.text);
            let overlap = q_tokens
                .iter()
                .filter(|t| have.iter().any(|h| h == *t))
                .count();
            if overlap > 0 {
                scored.push((turn.id.as_str(), overlap));
            }
        }
        scored.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
        scored
            .into_iter()
            .take(POOL)
            .map(|(id, _)| id.to_string())
            .collect()
    }

    fn hydrate(&self, item_id: &str, score: f32) -> Result<Option<Hit>> {
        if self.hidden.contains(item_id) {
            return Ok(None);
        }
        if let Some(fact) = self.overlay.iter().find(|fact| fact.id == item_id) {
            let mut hit = Hit::make(
                fact.id.clone(),
                &fact.layer,
                "",
                fact.text.clone(),
                fact.text.clone(),
                fact.said_by.clone(),
                fact.date.clone(),
                fact.source.clone(),
                "user-said",
                "",
                Vec::new(),
                Vec::new(),
                score,
            );
            hit.pinned = fact.pinned;
            hit.importance = fact.importance;
            hit.extractor = fact.extractor.clone();
            hit.kind = fact.kind.clone();
            return Ok(Some(hit));
        }
        if item_id.starts_with("turn-") || item_id.starts_with("bg-turn-") {
            return self.hydrate_turn(item_id, score);
        }
        if let Some(turn) = self.ring.turns().find(|t| t.id == item_id) {
            return Ok(Some(Hit::make(
                turn.id.clone(),
                "session",
                "",
                turn.text.clone(),
                turn.text.clone(),
                turn.said_by.clone(),
                turn.date.clone(),
                "session-ring",
                "user-said",
                "",
                Vec::new(),
                Vec::new(),
                score,
            )));
        }
        let row = self
            .conn
            .query_row(
                "SELECT layer, title, summary, body, said_by, source_date, source,
                        claim_type, evidence_strength, document, marked,
                        pinned, importance, kind, extractor, current, last_access
                 FROM items WHERE item_id = ?1",
                params![item_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, String>(7)?,
                        row.get::<_, String>(8)?,
                        row.get::<_, String>(9)?,
                        row.get::<_, i64>(10)?,
                        row.get::<_, i64>(11)?,
                        row.get::<_, f32>(12)?,
                        row.get::<_, String>(13)?,
                        row.get::<_, String>(14)?,
                        row.get::<_, i64>(15)?,
                        row.get::<_, String>(16)?,
                    ))
                },
            )
            .optional();
        let row = match row {
            Ok(row) => row,
            Err(err) => {
                let wrapped = Error::from(err);
                if is_busy_err(&wrapped) {
                    return Ok(None);
                }
                return Err(wrapped);
            }
        };
        let Some((
            layer,
            title,
            summary,
            body,
            said_by,
            date,
            source,
            claim_type,
            evidence,
            document,
            marked,
            pinned,
            importance,
            kind,
            extractor,
            current,
            last_access,
        )) = row
        else {
            return self.hydrate_turn(item_id, score);
        };
        if marked != 0 || current == 0 {
            return Ok(None);
        }
        let text = if !summary.is_empty() {
            summary
        } else {
            body.clone()
        };
        let (sources, views) = if layer == LAYER_SEMANTIC {
            provenance_from_document(&document)
        } else {
            (Vec::new(), Vec::new())
        };
        let mut hit = Hit::make(
            item_id, &layer, title, text, body, said_by, date, source, claim_type, evidence,
            sources, views, score,
        );
        hit.pinned = pinned != 0;
        hit.importance = if hit.pool == "memory" && importance <= 0.0 {
            DEFAULT_IMPORTANCE
        } else {
            importance
        };
        hit.extractor = extractor;
        hit.kind = kind;
        hit.last_access = last_access;
        Ok(Some(hit))
    }

    fn hydrate_turn(&self, turn_id: &str, score: f32) -> Result<Option<Hit>> {
        let row = self
            .conn
            .query_row(
                "SELECT body, said_by, source_date, source FROM session_turns WHERE turn_id = ?1",
                params![turn_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )
            .optional();
        let row = match row {
            Ok(row) => row,
            Err(err) => {
                let wrapped = Error::from(err);
                if is_busy_err(&wrapped) {
                    return Ok(None);
                }
                return Err(wrapped);
            }
        };
        let Some((body, said_by, date, source)) = row else {
            return Ok(None);
        };
        let mut hit = Hit::make(
            turn_id,
            "transcript",
            "",
            body.clone(),
            body,
            said_by,
            date,
            source,
            "user-said",
            "",
            Vec::new(),
            Vec::new(),
            score,
        );
        hit.importance = TRANSCRIPT_IMPORTANCE;
        hit.kind = "transcript".into();
        Ok(Some(hit))
    }

    fn reload_arena(&mut self) -> Result<()> {
        self.arena = read_arena(&self.conn)?;
        Ok(())
    }

    fn existing_card(&self, item_id: &str) -> Result<Option<(i64, String, i64)>> {
        let row = self
            .conn
            .query_row(
                "SELECT rowid, content_hash, marked FROM items WHERE item_id = ?1",
                params![item_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        Ok(row)
    }

    fn existing_rowid(&self, item_id: &str) -> Result<Option<i64>> {
        let row = self
            .conn
            .query_row(
                "SELECT rowid FROM items WHERE item_id = ?1",
                params![item_id],
                |r| r.get(0),
            )
            .optional()?;
        Ok(row)
    }

    fn rowid_of(&self, item_id: &str) -> Result<i64> {
        self.existing_rowid(item_id)?
            .ok_or_else(|| Error::NotFound(item_id.to_string()))
    }

    fn in_tx<T>(&mut self, body: impl FnOnce(&Self) -> Result<T>) -> Result<T> {
        self.conn.execute_batch("BEGIN IMMEDIATE")?;
        match body(self) {
            Ok(value) => {
                self.conn.execute_batch("COMMIT")?;
                Ok(value)
            }
            Err(err) => {
                let _ = self.conn.execute_batch("ROLLBACK");
                Err(err)
            }
        }
    }

    /// Copy the live card into `card_revisions` before it is replaced.
    /// The revision row is not removed by anything in this crate.
    fn archive_revision(&self, rowid: i64, reason: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO card_revisions (
                item_id, content_hash, document, claim_type, evidence_strength,
                said_by, source_date, source, archived_at, reason
             )
             SELECT item_id, content_hash, document, claim_type, evidence_strength,
                said_by, source_date, source, strftime('%Y-%m-%dT%H:%M:%fZ','now'), ?2
             FROM items WHERE rowid = ?1 AND content_hash != ''",
            params![rowid, reason],
        )?;
        Ok(())
    }

    /// Copy a live row into `archive`, then drop it from FTS and `items`.
    /// The copy is written first, inside the caller's transaction.
    fn archive_live_row(&self, rowid: i64, reason: &str) -> Result<()> {
        let moved = self.conn.execute(
            "INSERT INTO archive (
                item_id, layer, section, category, title, body, summary, claim_type,
                said_by, source_date, source, evidence_strength, status, reason,
                emb, emb_scale, emb_norm, document, content_hash
             )
             SELECT item_id, layer, section, category, title, body, summary, claim_type,
                said_by, source_date, source, evidence_strength, status, ?2,
                emb, emb_scale, emb_norm, document, content_hash
             FROM items WHERE rowid = ?1",
            params![rowid, reason],
        )?;
        if moved != 1 {
            return Err(Error::NotFound(format!("row {rowid}")));
        }
        self.conn
            .execute("DELETE FROM items_fts WHERE rowid = ?1", params![rowid])?;
        self.conn
            .execute("DELETE FROM items WHERE rowid = ?1", params![rowid])?;
        Ok(())
    }

    fn next_id(&self, prefix: &str) -> Result<String> {
        let id = alloc_id(&self.seq, prefix);
        let watermark = self.seq.load(Ordering::SeqCst) as i64;
        self.conn.execute(
            "UPDATE meta SET v = MAX(v, ?1) WHERE k = 'seq'",
            params![watermark],
        )?;
        Ok(id)
    }

    fn with_write<T>(&mut self, body: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        self.conn.busy_timeout(WRITE_BUSY)?;
        let result = body(self);
        let _ = self.conn.busy_timeout(READ_BUSY);
        result
    }

    fn summary_item_id(&self, session_id: &str) -> Result<Option<String>> {
        let item_id: Option<String> = self
            .conn
            .query_row(
                "SELECT summary_item_id FROM session_meta WHERE session_id = ?1",
                params![session_id],
                |row| row.get(0),
            )
            .optional()?;
        Ok(item_id.filter(|id| !id.is_empty()))
    }

    fn overlay_lexical(&self, query: &str) -> Vec<String> {
        let q_tokens = tokens(query);
        if q_tokens.is_empty() {
            return Vec::new();
        }
        let mut scored: Vec<(&str, usize)> = Vec::new();
        for fact in &self.overlay {
            if self.hidden.contains(&fact.id) {
                continue;
            }
            let have = tokens(&fact.text);
            let overlap = q_tokens
                .iter()
                .filter(|token| have.iter().any(|have| have == *token))
                .count();
            if overlap > 0 {
                scored.push((fact.id.as_str(), overlap));
            }
        }
        scored.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
        scored
            .into_iter()
            .take(POOL)
            .map(|(id, _)| id.to_string())
            .collect()
    }

    fn transcript_lexical(&self, query: &str) -> Result<Vec<String>> {
        let kept: Vec<&str> = self
            .keep_whole
            .iter()
            .filter(|(_, keep)| **keep)
            .map(|(session, _)| session.as_str())
            .collect();
        if kept.is_empty() {
            return Ok(Vec::new());
        }
        transcript_match(&self.conn, query, Some(&kept))
    }
}

impl Drop for Store {
    fn drop(&mut self) {
        self.jobs.shutdown();
    }
}

enum UpsertKind {
    Inserted,
    Revised,
    Unchanged,
    Restored,
}

fn memory_document(text: &str, said_by: &str, date: &str, source: &str) -> String {
    serde_json::json!({
        "kind": "memory",
        "text": text,
        "said_by": said_by,
        "date": date,
        "source": source,
    })
    .to_string()
}

fn ensure_column(conn: &Connection, table: &str, column: &str, decl: &str) -> Result<()> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let names = stmt.query_map([], |row| row.get::<_, String>(1))?;
    for name in names {
        if name? == column {
            return Ok(());
        }
    }
    drop(stmt);
    conn.execute_batch(&format!("ALTER TABLE {table} ADD COLUMN {column} {decl}"))?;
    Ok(())
}

fn fts_field(raw: &str) -> String {
    index_tokens(raw).join(" ")
}

/// Keep the rare terms. A term in more than 1/50 of a large diary is
/// dropped: its IDF is too small to be worth a full postings scan.
pub(crate) fn match_query(conn: &Connection, n_docs: i64, query: &str) -> Result<Option<String>> {
    let mut terms: Vec<String> = index_tokens(query);
    terms.sort();
    terms.dedup();
    if terms.is_empty() {
        return Ok(None);
    }
    let cap = df_cap(n_docs);
    let mut ranked: Vec<(i64, String)> = Vec::new();
    for term in terms {
        if is_stopword(&term) {
            continue;
        }
        let df = doc_freq_on(conn, &term)?;
        if df == 0 || (cap > 0 && df > cap) {
            continue;
        }
        ranked.push((df, term));
    }
    ranked.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    let mut kept = Vec::new();
    let mut budget = if cap == 0 { i64::MAX } else { cap };
    for (df, term) in ranked {
        if !kept.is_empty() && df > budget {
            break;
        }
        budget = budget.saturating_sub(df);
        kept.push(term);
        if kept.len() == MAX_LEXICAL_TERMS {
            break;
        }
    }
    if kept.is_empty() {
        return Ok(None);
    }
    Ok(Some(
        kept.into_iter()
            .map(|t| format!("\"{t}\""))
            .collect::<Vec<_>>()
            .join(" OR "),
    ))
}

pub(crate) fn doc_freq_on(conn: &Connection, term: &str) -> Result<i64> {
    let df = conn
        .query_row(
            "SELECT doc FROM items_fts_vocab WHERE term = ?1",
            params![term],
            |row| row.get(0),
        )
        .optional()?;
    Ok(df.unwrap_or(0))
}

/// `0` means "do not drop terms for frequency" (a tiny diary).
pub(crate) fn df_cap(n: i64) -> i64 {
    if n < DF_PLAN_MIN_ROWS {
        0
    } else {
        (n / DF_FRACTION_DEN).max(64)
    }
}

fn rebuild_fts(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "DROP TABLE IF EXISTS items_fts_vocab;
         DROP TABLE IF EXISTS items_fts;
         CREATE VIRTUAL TABLE items_fts USING fts5(
            title, summary, body, said_by, source,
            tokenize = 'unicode61'
         );
         CREATE VIRTUAL TABLE items_fts_vocab USING fts5vocab(items_fts, 'row');",
    )?;
    let mut stmt = conn.prepare(
        "SELECT rowid, title, summary, body, said_by, source
         FROM items WHERE marked = 0",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, String>(5)?,
        ))
    })?;
    let mut pending = Vec::new();
    for row in rows {
        pending.push(row?);
    }
    drop(stmt);
    for (rowid, title, summary, body, said_by, source) in pending {
        conn.execute(
            "INSERT INTO items_fts(rowid, title, summary, body, said_by, source)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                rowid,
                fts_field(&title),
                fts_field(&summary),
                fts_field(&body),
                fts_field(&said_by),
                fts_field(&source),
            ],
        )?;
    }
    Ok(())
}

fn as_blob(emb: &Int8Vec) -> Vec<u8> {
    emb.values.iter().map(|b| *b as u8).collect()
}

fn require_fields(text: &str, said_by: &str, date: &str, source: &str) -> Result<()> {
    if text.trim().is_empty() {
        return Err(Error::Invalid("text is empty".into()));
    }
    if said_by.trim().is_empty() {
        return Err(Error::Invalid("said_by is empty".into()));
    }
    if date.trim().is_empty() {
        return Err(Error::Invalid("date is empty".into()));
    }
    if source.trim().is_empty() {
        return Err(Error::Invalid("source is empty".into()));
    }
    Ok(())
}

fn unknown_if_empty(value: &str) -> String {
    if value.trim().is_empty() {
        "unknown".to_string()
    } else {
        value.to_string()
    }
}

fn prepare_schema(conn: &Connection) -> Result<()> {
    let version: i32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version == 0 {
        conn.execute_batch(SCHEMA)?;
        migrate_v5(conn)?;
        conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    } else if version < SCHEMA_VERSION {
        conn.execute_batch(SCHEMA)?;
        if version < 4 {
            ensure_column(conn, "items", "document", "TEXT NOT NULL DEFAULT ''")?;
            ensure_column(conn, "items", "content_hash", "TEXT NOT NULL DEFAULT ''")?;
            ensure_column(conn, "archive", "document", "TEXT NOT NULL DEFAULT ''")?;
            ensure_column(conn, "archive", "content_hash", "TEXT NOT NULL DEFAULT ''")?;
            rebuild_fts(conn)?;
        }
        migrate_v5(conn)?;
        conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    } else if version != SCHEMA_VERSION {
        return Err(Error::Invalid(format!(
            "unsupported memory database version {version}"
        )));
    }
    Ok(())
}

fn migrate_v5(conn: &Connection) -> Result<()> {
    ensure_column(conn, "items", "pinned", "INTEGER NOT NULL DEFAULT 0")?;
    ensure_column(conn, "items", "importance", "REAL NOT NULL DEFAULT 0")?;
    ensure_column(conn, "items", "kind", "TEXT NOT NULL DEFAULT ''")?;
    ensure_column(conn, "items", "session_id", "TEXT NOT NULL DEFAULT ''")?;
    ensure_column(conn, "items", "current", "INTEGER NOT NULL DEFAULT 1")?;
    ensure_column(conn, "items", "extractor", "TEXT NOT NULL DEFAULT ''")?;
    ensure_column(conn, "items", "subject_key", "TEXT NOT NULL DEFAULT ''")?;
    ensure_column(conn, "items", "object_value", "TEXT NOT NULL DEFAULT ''")?;
    ensure_column(conn, "items", "last_access", "TEXT NOT NULL DEFAULT ''")?;
    conn.execute_batch("CREATE INDEX IF NOT EXISTS items_subject ON items(subject_key);")?;
    conn.execute(
        "UPDATE items SET pinned = 1, importance = 1.0, kind = 'pin', extractor = 'user'
         WHERE layer = 'episode' AND kind = ''",
        [],
    )?;
    conn.execute(
        "UPDATE items SET kind = 'diary', importance = 0.5
         WHERE layer = 'diary' AND kind = ''",
        [],
    )?;
    Ok(())
}

fn read_arena(conn: &Connection) -> Result<Arena> {
    let mut arena = Arena::empty();
    let mut stmt = conn.prepare(
        "SELECT rowid, item_id, layer, emb, emb_scale, emb_norm
         FROM items WHERE marked = 0 AND current = 1 ORDER BY rowid",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Vec<u8>>(3)?,
            row.get::<_, f32>(4)?,
            row.get::<_, f32>(5)?,
        ))
    })?;
    for row in rows {
        let (rowid, item_id, layer, blob, scale, norm) = row?;
        if blob.len() != EMB_DIM {
            return Err(Error::Invalid(format!(
                "{item_id} has embedding width {}, expected {EMB_DIM}",
                blob.len()
            )));
        }
        arena.rowid.push(rowid);
        arena.item_id.push(item_id);
        arena
            .memory
            .push(layer == LAYER_EPISODE || layer == LAYER_DIARY);
        arena.scale.push(scale);
        arena.norm.push(norm);
        arena.emb.extend(blob.into_iter().map(|byte| byte as i8));
    }
    Ok(arena)
}

fn load_keep_whole(conn: &Connection) -> Result<HashMap<String, bool>> {
    let mut stmt = conn.prepare("SELECT session_id, keep_whole FROM session_meta")?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? != 0))
    })?;
    let mut out = HashMap::new();
    for row in rows {
        let (id, keep) = row?;
        out.insert(id, keep);
    }
    Ok(out)
}

fn rerank_memory(hits: &mut [Hit]) {
    let today = rank::today_utc_days();
    for hit in hits.iter_mut() {
        if hit.pool != "memory" || hit.layer == "session" {
            continue;
        }
        let recency = rank::recency_factor(&hit.date, &hit.last_access, today);
        hit.score = rank::rescale(hit.score, hit.importance, hit.pinned, recency);
    }
    hits.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| a.item_id.cmp(&b.item_id))
    });
}

fn merge_unique(dst: &mut Vec<String>, extra: Vec<String>) {
    for id in extra {
        if !dst.iter().any(|have| have == &id) {
            dst.push(id);
        }
    }
}

fn transcript_match(
    conn: &Connection,
    query: &str,
    sessions: Option<&[&str]>,
) -> Result<Vec<String>> {
    let Some(match_q) = simple_fts(query) else {
        return Ok(Vec::new());
    };
    let mut stmt = conn.prepare(
        "SELECT session_turns.turn_id, session_turns.session_id
         FROM session_turns_fts
         JOIN session_turns ON session_turns.id = session_turns_fts.rowid
         WHERE session_turns_fts MATCH ?1
         ORDER BY rank
         LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![match_q, POOL as i64], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (id, session) = row?;
        if let Some(sessions) = sessions {
            if !sessions.iter().any(|kept| *kept == session) {
                continue;
            }
        }
        out.push(id);
    }
    Ok(out)
}

fn simple_fts(query: &str) -> Option<String> {
    let mut terms = index_tokens(query);
    terms.sort();
    terms.dedup();
    terms.truncate(MAX_LEXICAL_TERMS);
    if terms.is_empty() {
        return None;
    }
    Some(
        terms
            .into_iter()
            .map(|term| format!("\"{term}\""))
            .collect::<Vec<_>>()
            .join(" OR "),
    )
}

fn is_busy_err(err: &Error) -> bool {
    match err {
        Error::Sqlite(rusqlite::Error::SqliteFailure(code, _)) => {
            code.code == rusqlite::ErrorCode::DatabaseBusy
                || code.code == rusqlite::ErrorCode::DatabaseLocked
        }
        _ => false,
    }
}

fn pin_subject(text: &str, _said_by: &str, _date: &str, _source: &str) -> (String, String) {
    let found = extract_line(text);
    if found.len() == 1 {
        (found[0].subject_key.clone(), found[0].object_value.clone())
    } else {
        (String::new(), String::new())
    }
}

fn collect_json(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_json(&path, out)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("json") {
            out.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embed::HashingEmbedder;

    #[test]
    fn an_old_episode_stays_searchable_after_migration() {
        let dir = std::env::temp_dir().join(format!("horbm-v4-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("memory.sqlite");
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE items (
                    rowid INTEGER PRIMARY KEY,
                    item_id TEXT NOT NULL UNIQUE,
                    layer TEXT NOT NULL,
                    section TEXT NOT NULL,
                    category TEXT NOT NULL DEFAULT '',
                    title TEXT NOT NULL DEFAULT '',
                    body TEXT NOT NULL,
                    summary TEXT NOT NULL DEFAULT '',
                    claim_type TEXT NOT NULL DEFAULT '',
                    said_by TEXT NOT NULL,
                    source_date TEXT NOT NULL,
                    source TEXT NOT NULL,
                    evidence_strength TEXT NOT NULL DEFAULT '',
                    status TEXT NOT NULL DEFAULT '',
                    marked INTEGER NOT NULL DEFAULT 0,
                    emb BLOB NOT NULL,
                    emb_scale REAL NOT NULL,
                    emb_norm REAL NOT NULL,
                    document TEXT NOT NULL DEFAULT '',
                    content_hash TEXT NOT NULL DEFAULT ''
                );
                CREATE TABLE meta (k TEXT PRIMARY KEY, v INTEGER NOT NULL);
                INSERT INTO meta(k, v) VALUES ('seq', 1);
                CREATE VIRTUAL TABLE items_fts USING fts5(
                    title, summary, body, said_by, source, tokenize = 'unicode61'
                );
                CREATE VIRTUAL TABLE items_fts_vocab USING fts5vocab(items_fts, 'row');
                CREATE TABLE archive (
                    rowid INTEGER PRIMARY KEY,
                    item_id TEXT NOT NULL,
                    layer TEXT NOT NULL,
                    section TEXT NOT NULL,
                    category TEXT NOT NULL DEFAULT '',
                    title TEXT NOT NULL DEFAULT '',
                    body TEXT NOT NULL,
                    summary TEXT NOT NULL DEFAULT '',
                    claim_type TEXT NOT NULL DEFAULT '',
                    said_by TEXT NOT NULL,
                    source_date TEXT NOT NULL,
                    source TEXT NOT NULL,
                    evidence_strength TEXT NOT NULL DEFAULT '',
                    status TEXT NOT NULL DEFAULT '',
                    reason TEXT NOT NULL,
                    emb BLOB NOT NULL,
                    emb_scale REAL NOT NULL,
                    emb_norm REAL NOT NULL,
                    document TEXT NOT NULL DEFAULT '',
                    content_hash TEXT NOT NULL DEFAULT ''
                );
                CREATE TABLE gaps (
                    id INTEGER PRIMARY KEY,
                    query TEXT NOT NULL,
                    asked_at TEXT NOT NULL,
                    reason TEXT NOT NULL,
                    nearest TEXT NOT NULL,
                    advice TEXT NOT NULL,
                    detail TEXT NOT NULL
                );",
            )
            .unwrap();
            let emb = HashingEmbedder.embed("keep the word legacytoken");
            let blob: Vec<u8> = emb.values.iter().map(|byte| *byte as u8).collect();
            conn.execute(
                "INSERT INTO items (
                    item_id, layer, section, category, title, body, summary, claim_type,
                    said_by, source_date, source, evidence_strength, status, marked,
                    emb, emb_scale, emb_norm, document, content_hash
                 ) VALUES (
                    'ep-1', 'episode', 'memory', 'episode', '', ?1, ?1, 'user-said',
                    'Ada', '2026-11-18', 'conversation', '', '', 0,
                    ?2, ?3, ?4, '{}', ''
                 )",
                params!["keep the word legacytoken", blob, emb.scale, emb.norm],
            )
            .unwrap();
            let rowid = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO items_fts(rowid, title, summary, body, said_by, source)
                 VALUES (?1, '', 'keep the word legacytoken', 'keep the word legacytoken', 'ada', 'conversation')",
                params![rowid],
            )
            .unwrap();
            conn.pragma_update(None, "user_version", 4).unwrap();
        }
        let store = Store::open(&path).unwrap();
        assert_eq!(store.schema_version().unwrap(), 5);
        let hits = store.search("legacytoken").unwrap();
        assert!(!hits.hits.is_empty(), "migrated episode was not searchable");
        let hit = hits
            .hits
            .iter()
            .find(|hit| hit.item_id == "ep-1")
            .expect("ep-1");
        assert!(hit.pinned);
        assert!((hit.importance - 1.0).abs() < 0.001);
        assert_eq!(hit.said_by, "Ada");
        assert_eq!(hit.date, "2026-11-18");
        assert_eq!(hit.source, "conversation");
        assert_eq!(hit.pool, "memory");
        drop(store);
        let _ = fs::remove_dir_all(&dir);
    }
}
