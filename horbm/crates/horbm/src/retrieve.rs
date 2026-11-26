// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Retrieval thread for one HORBM turn.
//!
//! This is phase 2 of `docs/horbm/research/R4-cpu-realtime-integration.md`:
//! a RAM fixture, `remember` denied unless granted, and a cell published off
//! the heartbeat thread. `Stack::open` is not used: it creates
//! `revisions.sqlite`.
//!
//! [`RetrievalConfig::keys_path`] unset keeps the lexical ask:
//! `search_lexical`, then [`covers`], then the blocked-id filter. No key
//! file is opened and no scan is built. When the path is set and `remember`
//! is granted, setup builds [`horbm_brain::DiaryScan`] with
//! [`DiaryScan::open_capped`] once. A query calls
//! [`DiaryScan::scan_selected_into`] into a `[ScanHit; MEMORY_HIT_CAP]` the
//! worker allocates once. That call does not allocate. A hit is published
//! only when the open store can already resolve its id. The clause is that
//! stored text. An id the store cannot resolve is dropped.
//!
//! Publishing the cell still builds [`MemoryHit`] strings.
//!
//! Person memory is a second diary. [`search_person_lexical`] reads that
//! file with FTS5. This thread does not mix that file into the scan. A hit
//! whose pool is not memory is dropped from the person search.
//!
//! Learned rows are diary lines the learn pack already wrote.
//! [`RetrievalThread::set_learned`] keeps them on the worker. A granted
//! thread inserts each one and rebuilds the scan when a key file is set.
//! The stored source starts with `learned;`. Withdraw keeps the rows and
//! drops the diary. The next grant inserts them again. Deny keeps the rows
//! and opens nothing.
//!
//! `LOCKS.toml` `[consent] remember = "deny"`. `[horbm] remember_grant` names
//! the only grant string. This module does not read the lock file. The
//! default matches it. `GALION_HORBM_REMEMBER=grant` is the explicit grant
//! for a process that is allowed to open the diary. `GALION_HORBM_KEYS` is
//! the HKB1 file, opened only while that grant holds.
//!
//! Do not call [`RetrievalThread::ask`], [`RetrievalThread::ask_wait`],
//! [`RetrievalThread::set_remember`], [`RetrievalThread::set_learned`], or
//! [`RetrievalThread::cell`] from `Heartbeat::step`. That step does not
//! allocate and does not block.
//! [`RetrievalThread::try_cell`] is the non-waiting read for a conductor
//! thread between frames. A `None` means the lock was busy, not that the
//! cell is empty.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::thread::{self, JoinHandle};
use std::time::Instant;

use horbm_brain::{
    DiaryArena, DiaryRow, DiaryScan, KeyPrefault, ScanHit, ScanPool, StoredKeys, DIARY_DIM,
};

use crate::learn::LearnedRow;
use crate::UNKNOWN_REPLY;
use crate::WINDOWS_KNOWLEDGE_DIR;

/// Published hits. Same cap as `horbm_memory::MAX_HITS`.
pub const MEMORY_HIT_CAP: usize = 4;

/// Learned rows kept on this thread.
///
/// A longer [`RetrievalThread::set_learned`] list keeps the newest
/// [`crate::LearnedRow::date`] values. The number of rows that did
/// not fit is `dropped N learned rows` in [`RetrievalThread::last_error`].
pub const LEARNED_ROW_CAP: usize = 256;

/// Where a hit came from. The two pools are not fused into one score.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HitPool {
    /// A knowledge card.
    Knowledge,
    /// A pin, a diary line, a transcript row, or the RAM ring.
    Memory,
}

/// One retrieved row. F4 speaks [`MemoryHit::clause`]. F3 keys the arena by
/// [`MemoryHit::item_id`] (a string, the card id or `ep-*` / `turn-*`).
#[derive(Clone, Debug, PartialEq)]
pub struct MemoryHit {
    pub item_id: String,
    pub pool: HitPool,
    pub title: String,
    pub text: String,
    pub said_by: String,
    pub date: String,
    pub source: String,
    pub claim_type: String,
    pub score: f32,
    pub pinned: bool,
}

impl MemoryHit {
    /// Gist when the card has one, otherwise the title.
    pub fn clause(&self) -> &str {
        if self.text.is_empty() {
            &self.title
        } else {
            &self.text
        }
    }

    fn from_hit(hit: &horbm_memory::Hit) -> Self {
        let pool = if hit.pool == "memory" {
            HitPool::Memory
        } else {
            HitPool::Knowledge
        };
        Self {
            item_id: hit.item_id.clone(),
            pool,
            title: hit.title.clone(),
            text: hit.text.clone(),
            said_by: hit.said_by.clone(),
            date: hit.date.clone(),
            source: hit.source.clone(),
            claim_type: hit.claim_type.clone(),
            score: hit.score,
            pinned: hit.pinned,
        }
    }
}

/// What the published cell is. `Idle` is the deny state and the state before
/// any granted ask. `Miss` is a granted ask that covered nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CellState {
    Idle,
    Miss,
    Hits,
}

/// The cell F4 reads. At most [`MEMORY_HIT_CAP`] hits.
///
/// `Idle` keeps `query` and `answer` empty, including after `remember` is
/// withdrawn. A caller that already cloned a snapshot still holds that copy.
/// Drop it when the bit falls. The thread drops the diary at the same time.
#[derive(Clone, Debug, PartialEq)]
pub struct MemoryHitCell {
    pub state: CellState,
    pub epoch: u64,
    pub turn_id: u64,
    pub query: String,
    pub hits: Vec<MemoryHit>,
    pub answer: String,
    pub took_us: u32,
}

impl MemoryHitCell {
    pub fn idle() -> Self {
        Self {
            state: CellState::Idle,
            epoch: 0,
            turn_id: 0,
            query: String::new(),
            hits: Vec::new(),
            answer: String::new(),
            took_us: 0,
        }
    }

    /// Denied, withdrawn, or not yet asked. A miss is not empty: it carries
    /// [`UNKNOWN_REPLY`].
    pub fn is_empty(&self) -> bool {
        self.state == CellState::Idle
            && self.hits.is_empty()
            && self.answer.is_empty()
            && self.query.is_empty()
    }

    fn miss(turn_id: u64, epoch: u64, query: &str, took_us: u32) -> Self {
        Self {
            state: CellState::Miss,
            epoch,
            turn_id,
            query: query.to_string(),
            hits: Vec::new(),
            answer: UNKNOWN_REPLY.to_string(),
            took_us,
        }
    }

    fn hits(turn_id: u64, epoch: u64, query: &str, hits: Vec<MemoryHit>, took_us: u32) -> Self {
        Self {
            state: CellState::Hits,
            epoch,
            turn_id,
            query: query.to_string(),
            hits,
            answer: String::new(),
            took_us,
        }
    }
}

impl Default for MemoryHitCell {
    fn default() -> Self {
        Self::idle()
    }
}

/// F4 reads the latest cell. The retrieval thread is the publisher.
pub trait RetrievedCells: Send + Sync {
    fn cell(&self) -> Arc<MemoryHitCell>;
    /// `None` only when the publisher holds the lock. An empty diary is
    /// `Some` of an idle cell.
    fn try_cell(&self) -> Option<Arc<MemoryHitCell>>;
}

/// F3's mapped int8 arena. [`RetrievalThread`] does not call this trait.
/// A granted ask with a key file calls [`DiaryScan::scan_selected_into`].
/// An unset key file calls `search_lexical` instead.
/// [`crate::StringKeyArena`] still implements this for a version-1 arena
/// and returns the raw `i32` dot.
pub trait Int8Scorer: Send + Sync {
    /// `None` when the arena has no row for `item_id`.
    fn score(&self, item_id: &str, query: &str) -> Option<i32>;
}

/// How the thread opens. `remember` defaults to false.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RetrievalConfig {
    /// `false` matches `LOCKS.toml` `[consent] remember = "deny"`.
    pub remember: bool,
    /// Card directory. Read only when `remember` is granted and
    /// [`RetrievalConfig::fixture_rows`] is zero. Not created.
    pub knowledge_dir: Option<PathBuf>,
    /// Synthetic cards loaded into the RAM diary. Ignored when `remember`
    /// is false. When this is non-zero the card directory is not opened.
    pub fixture_rows: usize,
    /// HKB1 file. Opened only while `remember` is granted, once at setup.
    /// Not created. `None` keeps lexical search and does not build a scan.
    pub keys_path: Option<PathBuf>,
}

impl RetrievalConfig {
    /// `GALION_HORBM_REMEMBER=grant` grants. Any other value, including when
    /// the name is unset, denies. `GALION_HORBM_KNOWLEDGE` is the card directory
    /// when it is set. When it is unset, [`WINDOWS_KNOWLEDGE_DIR`] is used
    /// only if that directory already exists. `GALION_HORBM_KEYS` is the HKB1
    /// path when it is set and non-empty. It is not opened here.
    pub fn from_env() -> Self {
        let flag = std::env::var("GALION_HORBM_REMEMBER").ok();
        let knowledge = std::env::var("GALION_HORBM_KNOWLEDGE").ok();
        let keys = std::env::var("GALION_HORBM_KEYS").ok();
        let mut config = Self::from_flags(flag.as_deref(), knowledge.as_deref());
        config.keys_path = keys_path_from_flag(keys.as_deref());
        config
    }

    pub fn from_flags(remember_flag: Option<&str>, knowledge: Option<&str>) -> Self {
        let remember = remember_flag.map(str::trim) == Some("grant");
        let knowledge_dir = match knowledge {
            Some(raw) => {
                let raw = raw.trim();
                if raw.is_empty() {
                    None
                } else {
                    Some(PathBuf::from(raw))
                }
            }
            None => {
                let fallback = PathBuf::from(WINDOWS_KNOWLEDGE_DIR);
                if fallback.is_dir() {
                    Some(fallback)
                } else {
                    None
                }
            }
        };
        Self {
            remember,
            knowledge_dir,
            fixture_rows: 0,
            keys_path: None,
        }
    }
}

fn keys_path_from_flag(raw: Option<&str>) -> Option<PathBuf> {
    let raw = raw?.trim();
    if raw.is_empty() {
        None
    } else {
        Some(PathBuf::from(raw))
    }
}

/// Open or query failed. The message is the diary error. It does not include
/// the ask text.
#[derive(Debug)]
pub struct RetrieveError {
    message: String,
}

impl RetrieveError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    fn memory(err: horbm_memory::Error) -> Self {
        Self {
            message: err.to_string(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for RetrieveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for RetrieveError {}

enum Command {
    Ask {
        text: String,
        turn_id: u64,
        epoch: u64,
        done: Option<Sender<()>>,
    },
    Consent {
        grant: bool,
        done: Option<Sender<()>>,
    },
    // Sent through `wait`, so an ask that is still queued does not replace it.
    Learned {
        rows: Vec<LearnedRow>,
        done: Option<Sender<()>>,
    },
}

impl Command {
    fn take_done(self) -> Option<Sender<()>> {
        match self {
            Command::Ask { done, .. }
            | Command::Consent { done, .. }
            | Command::Learned { done, .. } => done,
        }
    }
}

struct Shared {
    slot: Mutex<Option<Command>>,
    wake: Condvar,
    stop: AtomicBool,
    cell: RwLock<Arc<MemoryHitCell>>,
    last_error: Mutex<Option<String>>,
}

impl Shared {
    fn new() -> Self {
        Self {
            slot: Mutex::new(None),
            wake: Condvar::new(),
            stop: AtomicBool::new(false),
            cell: RwLock::new(Arc::new(MemoryHitCell::idle())),
            last_error: Mutex::new(None),
        }
    }
}

/// Owns the diary on one std thread and publishes [`MemoryHitCell`].
pub struct RetrievalThread {
    shared: Arc<Shared>,
    handle: Option<JoinHandle<()>>,
}

impl RetrievalThread {
    /// Starts the thread and, when `remember` is granted, loads the RAM diary
    /// before returning. A denied config does not open SQLite.
    pub fn spawn(config: RetrievalConfig) -> Result<Self, RetrieveError> {
        let shared = Arc::new(Shared::new());
        let (ready_tx, ready_rx) = mpsc::channel();
        let thread_shared = Arc::clone(&shared);
        let handle = thread::Builder::new()
            .name("horbm-retrieval".to_string())
            .spawn(move || thread_main(config, thread_shared, ready_tx))
            .map_err(|err| RetrieveError::new(format!("spawn retrieval thread: {err}")))?;
        match ready_rx.recv() {
            Ok(Ok(())) => Ok(Self {
                shared,
                handle: Some(handle),
            }),
            Ok(Err(err)) => {
                let _ = handle.join();
                Err(err)
            }
            Err(_) => {
                let _ = handle.join();
                Err(RetrieveError::new(
                    "retrieval thread stopped before it was ready",
                ))
            }
        }
    }

    /// Queue an ask. Does not wait. A newer ask, a consent change, or
    /// [`RetrievalThread::set_learned`] replaces an ask that has not started.
    /// An ask leaves a learned command sitting in the slot.
    pub fn ask(&self, text: impl Into<String>, turn_id: u64, epoch: u64) {
        self.enqueue(Command::Ask {
            text: text.into(),
            turn_id,
            epoch,
            done: None,
        });
    }

    /// Queue an ask and wait until the thread has finished that command.
    pub fn ask_wait(
        &self,
        text: impl Into<String>,
        turn_id: u64,
        epoch: u64,
    ) -> Result<(), RetrieveError> {
        self.wait(Command::Ask {
            text: text.into(),
            turn_id,
            epoch,
            done: None,
        })
    }

    /// Grant or withdraw. Withdraw drops the RAM diary and publishes an idle
    /// cell. Learned rows stay on the worker. Grant opens an in-memory diary,
    /// loads the fixture or the card directory from the config passed to
    /// [`RetrievalThread::spawn`], and inserts those learned rows again.
    pub fn set_remember(&self, grant: bool) -> Result<(), RetrieveError> {
        self.wait(Command::Consent { grant, done: None })
    }

    /// Replace the learned rows this thread recalls.
    ///
    /// The worker keeps `rows`. When recall is already granted it rebuilds
    /// the diary once and, when a key file is set, the scan. Each row is
    /// inserted with `diary_add`. The stored source is `learned;` plus
    /// [`crate::LearnedRow::source`]. An empty list clears the rows.
    /// More than [`LEARNED_ROW_CAP`] keeps the newest dates and records the drop in
    /// [`RetrievalThread::last_error`]. A denied thread stores the rows and
    /// does not open the diary. An ask does not replace this command.
    pub fn set_learned(&self, rows: Vec<LearnedRow>) -> Result<(), RetrieveError> {
        self.wait(Command::Learned { rows, done: None })
    }

    /// Last diary error. Cleared on a successful grant, on withdraw, and when
    /// a learned update drops nothing. A list over [`LEARNED_ROW_CAP`] sets
    /// `dropped N learned rows`.
    pub fn last_error(&self) -> Option<String> {
        self.shared
            .last_error
            .lock()
            .expect("retrieval error")
            .clone()
    }

    fn wait(&self, mut cmd: Command) -> Result<(), RetrieveError> {
        let (tx, rx) = mpsc::channel();
        match &mut cmd {
            Command::Ask { done, .. }
            | Command::Consent { done, .. }
            | Command::Learned { done, .. } => *done = Some(tx),
        }
        self.enqueue(cmd);
        rx.recv()
            .map_err(|_| RetrieveError::new("retrieval thread stopped"))
    }

    fn enqueue(&self, cmd: Command) {
        let mut slot = self.shared.slot.lock().expect("retrieval mailbox");
        // The learned command was sent through `wait`. An ask must not take
        // its slot, or `set_learned` would return before the rows were stored.
        if matches!(cmd, Command::Ask { .. })
            && matches!(slot.as_ref(), Some(Command::Learned { .. }))
        {
            signal(cmd.take_done());
            return;
        }
        if let Some(old) = slot.replace(cmd) {
            signal(old.take_done());
        }
        self.shared.wake.notify_one();
    }
}

impl RetrievedCells for RetrievalThread {
    fn cell(&self) -> Arc<MemoryHitCell> {
        Arc::clone(&self.shared.cell.read().expect("retrieval cell"))
    }

    fn try_cell(&self) -> Option<Arc<MemoryHitCell>> {
        self.shared
            .cell
            .try_read()
            .ok()
            .map(|guard| Arc::clone(&guard))
    }
}

impl Drop for RetrievalThread {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::SeqCst);
        self.shared.wake.notify_one();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn signal(done: Option<Sender<()>>) {
    if let Some(done) = done {
        let _ = done.send(());
    }
}

fn thread_main(
    config: RetrievalConfig,
    shared: Arc<Shared>,
    ready: Sender<Result<(), RetrieveError>>,
) {
    let store = if config.remember {
        match rebuild(&config, &[]) {
            Ok(opened) => Some(opened),
            Err(err) => {
                let _ = ready.send(Err(err));
                return;
            }
        }
    } else {
        None
    };
    if ready.send(Ok(())).is_err() {
        return;
    }
    worker(store, config, &shared);
}

fn worker(mut store: Option<Opened>, config: RetrievalConfig, shared: &Shared) {
    // One hit buffer for the life of the worker. A query reuses it.
    // Learned rows live here, not on `Shared`. An ask does not read them.
    let mut learned: Vec<LearnedRow> = Vec::new();
    let mut hit_buf = [ScanHit::default(); MEMORY_HIT_CAP];
    while let Some(cmd) = recv(shared) {
        match cmd {
            Command::Ask {
                text,
                turn_id,
                epoch,
                done,
            } => {
                if !superseded(shared, epoch) {
                    publish_ask(shared, store.as_mut(), &mut hit_buf, &text, turn_id, epoch);
                }
                signal(done);
            }
            Command::Consent { grant, done } => {
                apply_consent(shared, &mut store, &config, &learned, grant);
                signal(done);
            }
            Command::Learned { rows, done } => {
                apply_learned(shared, &mut store, &config, &mut learned, rows);
                signal(done);
            }
        }
    }
}

fn publish_ask(
    shared: &Shared,
    store: Option<&mut Opened>,
    hit_buf: &mut [ScanHit; MEMORY_HIT_CAP],
    text: &str,
    turn_id: u64,
    epoch: u64,
) {
    let Some(opened) = store else {
        publish(shared, MemoryHitCell::idle(), true);
        return;
    };
    match asked(opened, hit_buf, text, turn_id, epoch) {
        Ok(cell) => publish(shared, cell, false),
        Err(err) => {
            set_last_error(shared, Some(err.message));
            publish(shared, MemoryHitCell::miss(turn_id, epoch, text, 0), false);
        }
    }
}

fn apply_consent(
    shared: &Shared,
    store: &mut Option<Opened>,
    config: &RetrievalConfig,
    learned: &[LearnedRow],
    grant: bool,
) {
    if grant {
        if store.is_none() {
            match rebuild(config, learned) {
                Ok(opened) => {
                    *store = Some(opened);
                    set_last_error(shared, None);
                }
                Err(err) => set_last_error(shared, Some(err.message)),
            }
        }
        return;
    }
    *store = None;
    set_last_error(shared, None);
    publish(shared, MemoryHitCell::idle(), true);
}

fn apply_learned(
    shared: &Shared,
    store: &mut Option<Opened>,
    config: &RetrievalConfig,
    learned: &mut Vec<LearnedRow>,
    rows: Vec<LearnedRow>,
) {
    let (kept, dropped) = keep_newest(rows);
    let drop_error = (dropped > 0).then(|| format!("dropped {dropped} learned rows"));
    // Deny stores the list and does not open a diary.
    if store.is_none() {
        *learned = kept;
        if let Some(message) = drop_error {
            set_last_error(shared, Some(message));
        }
        return;
    }
    match rebuild(config, &kept) {
        Ok(opened) => {
            *store = Some(opened);
            *learned = kept;
            set_last_error(shared, drop_error);
        }
        Err(err) => set_last_error(shared, Some(err.message)),
    }
}

/// Keep at most [`LEARNED_ROW_CAP`] rows. Newest [`crate::LearnedRow::date`] wins.
/// Equal dates keep the later input rows: the sort is stable, and the
/// oldest prefix is what gets dropped. Does not panic.
fn keep_newest(mut rows: Vec<LearnedRow>) -> (Vec<LearnedRow>, usize) {
    if rows.len() <= LEARNED_ROW_CAP {
        return (rows, 0);
    }
    let dropped = rows.len() - LEARNED_ROW_CAP;
    rows.sort_by(|left, right| left.date.cmp(&right.date));
    let kept = rows.split_off(dropped);
    (kept, dropped)
}

fn set_last_error(shared: &Shared, message: Option<String>) {
    *shared.last_error.lock().expect("retrieval error") = message;
}

struct Opened {
    /// In-memory diary. The lexical ask reads it. Dropped when the grant falls.
    store: horbm_memory::Store,
    /// Card ids that must not be published. Also stored as `allow: false`.
    blocked: BTreeSet<String>,
    /// Rebuilt when the diary is opened or learned rows change. Absent on
    /// the lexical path.
    scan: Option<PreparedScan>,
}

/// Key file plus the ids the store could already resolve when the grant opened.
struct PreparedScan {
    scan: DiaryScan,
    /// Clause text captured from the store. A query only reads it.
    resolved: HashMap<String, CatalogRow>,
}

/// Text for one arena row. Built when the grant opens. A query only reads it.
#[derive(Clone)]
struct CatalogRow {
    item_id: String,
    title: String,
    text: String,
    said_by: String,
    date: String,
    source: String,
    claim_type: String,
    pinned: bool,
    haystack: String,
}

struct EmbeddedRow {
    item_id: String,
    values: Vec<i8>,
    scale: f32,
    norm: f32,
    allow: bool,
    catalog: CatalogRow,
}

fn open_store(config: &RetrievalConfig) -> Result<Opened, RetrieveError> {
    let mut store = horbm_memory::Store::open_in_memory().map_err(RetrieveError::memory)?;
    let blocked = load_rows(&mut store, config)?;
    Ok(Opened {
        store,
        blocked,
        scan: None,
    })
}

/// Open the diary, insert `learned`, then build the scan once when a key
/// file is set. Spawn, a grant, and [`RetrievalThread::set_learned`] share
/// this. `open_store` is not copied. The scan is built after the inserts so
/// a learned row is in the arena. This does not run on an ask.
fn rebuild(config: &RetrievalConfig, learned: &[LearnedRow]) -> Result<Opened, RetrieveError> {
    let mut opened = open_store(config)?;
    insert_learned(&mut opened.store, learned)?;
    if let Some(path) = config.keys_path.as_deref() {
        opened.scan = Some(prepare_scan(&opened.store, &opened.blocked, path, true)?);
    }
    Ok(opened)
}

fn insert_learned(
    store: &mut horbm_memory::Store,
    rows: &[LearnedRow],
) -> Result<(), RetrieveError> {
    for row in rows {
        let label = row.source.as_str();
        let source = format!("learned;{label}");
        store
            .diary_add(&row.text, &row.date, &row.said_by, &source)
            .map_err(RetrieveError::memory)?;
    }
    Ok(())
}

/// Open the key file and the diary arena once. Not called on a query.
///
/// The pinned `horbm-memory` rev does not return stored embedding rows.
/// Diary entries are text, and the int8 arena is private. Rows are rebuilt
/// with [`horbm_brain::embed_diary_into`] from text the store already holds,
/// the same hashing vector that rev stores. A store with no rows is an empty
/// [`DiaryArena`], and the scan ranks keys only. The git rev is unchanged.
fn prepare_scan(
    store: &horbm_memory::Store,
    blocked: &BTreeSet<String>,
    path: &Path,
    remember: bool,
) -> Result<PreparedScan, RetrieveError> {
    let keys = StoredKeys::open(path, KeyPrefault::Populate)
        .map_err(|err| RetrieveError::new(format!("open keys: {err}")))?;
    let embedded = embed_store(store, blocked)?;
    let mut resolved = HashMap::with_capacity(embedded.len());
    for row in &embedded {
        resolved.insert(row.item_id.clone(), row.catalog.clone());
    }
    let diary = DiaryArena::from_rows(embedded.iter().map(|row| DiaryRow {
        item_id: row.item_id.as_str(),
        values: row.values.as_slice(),
        scale: row.scale,
        norm: row.norm,
        allow: row.allow,
    }))
    .map_err(|err| RetrieveError::new(format!("diary arena: {err}")))?;
    let scan = DiaryScan::open_capped(keys, diary, remember);
    Ok(PreparedScan { scan, resolved })
}

fn load_rows(
    store: &mut horbm_memory::Store,
    config: &RetrievalConfig,
) -> Result<BTreeSet<String>, RetrieveError> {
    if config.fixture_rows > 0 {
        let cards: Vec<_> = (0..config.fixture_rows)
            .map(horbm_memory::synthetic_card)
            .collect();
        store.upsert_cards(&cards).map_err(RetrieveError::memory)?;
        return Ok(BTreeSet::new());
    }
    match &config.knowledge_dir {
        Some(dir) => {
            let blocked = crate::cards::blocked_answer_ids(dir);
            store.ingest_dir(dir).map_err(RetrieveError::memory)?;
            Ok(blocked)
        }
        None => Ok(BTreeSet::new()),
    }
}

/// Person-diary lines, then card documents. Embeddings match the store's
/// hashing embedder (`embed_diary_into` on the same text). A marked line or
/// an answer-blocked card is `allow: false` and is not dotted.
fn embed_store(
    store: &horbm_memory::Store,
    blocked: &BTreeSet<String>,
) -> Result<Vec<EmbeddedRow>, RetrieveError> {
    let mut prepared = Vec::new();
    let mut seen = BTreeSet::new();
    for entry in store.diary_list().map_err(RetrieveError::memory)? {
        if !seen.insert(entry.item_id.clone()) {
            continue;
        }
        let allow = !entry.marked && !blocked.contains(&entry.item_id);
        let catalog = CatalogRow {
            item_id: entry.item_id.clone(),
            title: String::new(),
            text: entry.text.clone(),
            said_by: entry.said_by.clone(),
            date: entry.date.clone(),
            source: entry.source.clone(),
            claim_type: "user-said".to_string(),
            pinned: false,
            haystack: join_hay(&[
                entry.text.as_str(),
                entry.said_by.as_str(),
                entry.source.as_str(),
                entry.date.as_str(),
            ]),
        };
        prepared.push(embed_catalog(&entry.text, allow, catalog)?);
    }
    for document in store.documents().map_err(RetrieveError::memory)? {
        let card = horbm_memory::card_from_document(&document).map_err(RetrieveError::memory)?;
        if !seen.insert(card.id.clone()) {
            continue;
        }
        let allow = !blocked.contains(&card.id);
        let catalog = catalog_from_card(&card);
        prepared.push(embed_catalog(&card.index_text(), allow, catalog)?);
    }
    Ok(prepared)
}

fn catalog_from_card(card: &horbm_memory::Card) -> CatalogRow {
    let text = card.gist();
    let mut haystack = join_hay(&[
        card.title.as_str(),
        text.as_str(),
        card.body.as_str(),
        card.said_by.as_str(),
        card.source.as_str(),
    ]);
    for view in &card.contested_views {
        haystack.push_str(&view.label);
        haystack.push(' ');
        haystack.push_str(&view.summary);
        haystack.push(' ');
    }
    CatalogRow {
        item_id: card.id.clone(),
        title: card.title.clone(),
        text,
        said_by: card.said_by.clone(),
        date: card.date.clone(),
        source: card.source.clone(),
        claim_type: card.claim_type.clone(),
        pinned: false,
        haystack,
    }
}

fn embed_catalog(
    text: &str,
    allow: bool,
    catalog: CatalogRow,
) -> Result<EmbeddedRow, RetrieveError> {
    let mut values = vec![0i8; DIARY_DIM];
    let embed = horbm_brain::embed_diary_into(text, &mut values)
        .map_err(|err| RetrieveError::new(format!("diary embed: {err}")))?;
    Ok(EmbeddedRow {
        item_id: catalog.item_id.clone(),
        values,
        scale: embed.scale,
        norm: embed.norm,
        allow,
        catalog,
    })
}

fn join_hay(parts: &[&str]) -> String {
    let mut out = String::new();
    for part in parts {
        out.push_str(part);
        out.push(' ');
    }
    out
}

fn asked(
    opened: &mut Opened,
    hit_buf: &mut [ScanHit; MEMORY_HIT_CAP],
    text: &str,
    turn_id: u64,
    epoch: u64,
) -> Result<MemoryHitCell, RetrieveError> {
    if let Some(prepared) = opened.scan.as_mut() {
        return asked_scan(prepared, &opened.blocked, hit_buf, text, turn_id, epoch);
    }
    asked_lexical(&opened.store, &opened.blocked, text, turn_id, epoch)
}

/// `search_lexical`, then [`covers`], then the blocked-id filter.
///
/// This is the ask an unset `keys_path` runs. It does not open a key file
/// and it does not call the scan.
fn asked_lexical(
    store: &horbm_memory::Store,
    blocked: &BTreeSet<String>,
    text: &str,
    turn_id: u64,
    epoch: u64,
) -> Result<MemoryHitCell, RetrieveError> {
    let response = store.search_lexical(text).map_err(RetrieveError::memory)?;
    let terms = horbm_memory::index_tokens(text);
    let mut hits = Vec::new();
    for hit in &response.hits {
        if blocked.contains(&hit.item_id) {
            continue;
        }
        if covers(&hit.haystack, &terms) {
            hits.push(MemoryHit::from_hit(hit));
        }
        if hits.len() == MEMORY_HIT_CAP {
            break;
        }
    }
    if hits.is_empty() {
        Ok(MemoryHitCell::miss(turn_id, epoch, text, response.took_us))
    } else {
        Ok(MemoryHitCell::hits(
            turn_id,
            epoch,
            text,
            hits,
            response.took_us,
        ))
    }
}

/// Scan into the worker's buffer. Publish only ids [`PreparedScan::resolved`]
/// already holds. The clause is that stored text. Nothing is invented.
fn asked_scan(
    prepared: &mut PreparedScan,
    blocked: &BTreeSet<String>,
    hit_buf: &mut [ScanHit; MEMORY_HIT_CAP],
    text: &str,
    turn_id: u64,
    epoch: u64,
) -> Result<MemoryHitCell, RetrieveError> {
    let started = Instant::now();
    // Caller-owned buffer. This call does not allocate.
    let n = prepared.scan.scan_selected_into(text, hit_buf);
    let took_us = elapsed_us(started.elapsed());
    let terms = horbm_memory::index_tokens(text);
    let mut hits = Vec::new();
    for hit in &hit_buf[..n] {
        let hit = *hit;
        let Some(item_id) = prepared.scan.item_id(hit) else {
            continue;
        };
        if blocked.contains(item_id) {
            continue;
        }
        let Some(row) = prepared.resolved.get(item_id) else {
            continue;
        };
        if !covers(&row.haystack, &terms) {
            continue;
        }
        hits.push(MemoryHit {
            item_id: row.item_id.clone(),
            pool: pool_of(hit.pool),
            title: row.title.clone(),
            text: row.text.clone(),
            said_by: row.said_by.clone(),
            date: row.date.clone(),
            source: row.source.clone(),
            claim_type: row.claim_type.clone(),
            score: hit.score,
            pinned: row.pinned,
        });
        if hits.len() == MEMORY_HIT_CAP {
            break;
        }
    }
    if hits.is_empty() {
        Ok(MemoryHitCell::miss(turn_id, epoch, text, took_us))
    } else {
        Ok(MemoryHitCell::hits(turn_id, epoch, text, hits, took_us))
    }
}

/// [`ScanPool::Diary`] is the memory pool. [`ScanPool::Keys`] is knowledge.
fn pool_of(pool: ScanPool) -> HitPool {
    match pool {
        ScanPool::Diary => HitPool::Memory,
        ScanPool::Keys => HitPool::Knowledge,
    }
}

fn elapsed_us(elapsed: std::time::Duration) -> u32 {
    u32::try_from(elapsed.as_micros()).unwrap_or(u32::MAX)
}

/// Lexical FTS5 search of a person-memory diary.
///
/// Pass the person file, not the truth-card diary. Hits whose pool is not
/// memory are dropped, so a card that landed in the wrong file is not a
/// person hit.
pub fn search_person_lexical(
    store: &horbm_memory::Store,
    query: &str,
) -> Result<Vec<MemoryHit>, RetrieveError> {
    let response = store.search_lexical(query).map_err(RetrieveError::memory)?;
    let mut hits = Vec::new();
    for hit in &response.hits {
        let mapped = MemoryHit::from_hit(hit);
        if mapped.pool != HitPool::Memory {
            continue;
        }
        hits.push(mapped);
    }
    Ok(hits)
}

/// Same predicate as `horbm-memory` `stack.rs` `covers`: every content token
/// of the query is in the hit haystack. An empty term list does not match.
pub(crate) fn covers(haystack: &str, terms: &[String]) -> bool {
    if terms.is_empty() {
        return false;
    }
    let hay: std::collections::HashSet<String> =
        horbm_memory::index_tokens(haystack).into_iter().collect();
    terms.iter().all(|term| hay.contains(term))
}

fn publish(shared: &Shared, next: MemoryHitCell, force: bool) {
    let mut guard = shared.cell.write().expect("retrieval cell");
    if !accept_cell(guard.epoch, next.epoch, force) {
        return;
    }
    *guard = Arc::new(next);
}

pub(crate) fn accept_cell(current_epoch: u64, incoming_epoch: u64, force: bool) -> bool {
    force || incoming_epoch >= current_epoch
}

fn superseded(shared: &Shared, epoch: u64) -> bool {
    let slot = shared.slot.lock().expect("retrieval mailbox");
    match slot.as_ref() {
        Some(Command::Ask { epoch: newer, .. }) => *newer > epoch,
        Some(Command::Consent { .. } | Command::Learned { .. }) => true,
        None => false,
    }
}

fn recv(shared: &Shared) -> Option<Command> {
    let mut slot = shared.slot.lock().expect("retrieval mailbox");
    loop {
        if let Some(cmd) = slot.take() {
            return Some(cmd);
        }
        if shared.stop.load(Ordering::SeqCst) {
            return None;
        }
        slot = shared.wake.wait(slot).expect("retrieval mailbox");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cap_matches_the_diary() {
        assert_eq!(MEMORY_HIT_CAP, horbm_memory::MAX_HITS);
        assert_eq!(MEMORY_HIT_CAP, horbm_brain::SCAN_CAP);
        assert_eq!(LEARNED_ROW_CAP, 256);
    }

    fn learned_sample(id: &str, date: &str) -> LearnedRow {
        LearnedRow {
            item_id: id.to_string(),
            text: format!("line {id}"),
            said_by: "user".to_string(),
            date: date.to_string(),
            source: "session=s;turn=1;time=t;consent=c".to_string(),
        }
    }

    #[test]
    fn keep_newest_drops_the_oldest_dates_and_keeps_later_ties() {
        let mut rows = vec![learned_sample("early", "0000")];
        for i in 1..LEARNED_ROW_CAP {
            rows.push(learned_sample(&format!("n{i}"), &format!("{i:04}")));
        }
        rows.push(learned_sample("late", "0000"));
        assert_eq!(rows.len(), LEARNED_ROW_CAP + 1);
        let (kept, dropped) = keep_newest(rows);
        assert_eq!(dropped, 1);
        assert_eq!(kept.len(), LEARNED_ROW_CAP);
        assert!(kept.iter().all(|row| row.item_id != "early"));
        assert_eq!(kept[0].item_id, "late");
        assert_eq!(
            kept.last().expect("kept").date,
            format!("{:04}", LEARNED_ROW_CAP - 1)
        );
        let (same, none) = keep_newest(Vec::new());
        assert!(same.is_empty());
        assert_eq!(none, 0);
        let exact: Vec<_> = (0..LEARNED_ROW_CAP)
            .map(|i| learned_sample(&format!("e{i}"), &format!("{i:04}")))
            .collect();
        let (all, zero) = keep_newest(exact);
        assert_eq!(zero, 0);
        assert_eq!(all.len(), LEARNED_ROW_CAP);
    }

    #[test]
    fn locks_keep_remember_deny_and_name_the_retrieval_hot_path() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../LOCKS.toml");
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("{}: {err}", path.display()));
        assert!(text.contains("remember = \"deny\""), "consent remember");
        assert!(text.contains("default = \"deny\""), "consent default");
        assert!(text.contains("hot_path = \"retrieval\""));
        assert!(text.contains("student_on_tick = false"));
        assert!(text.contains("miss = \"I don't know.\""));
        assert!(text.contains("remember_grant = \"GALION_HORBM_REMEMBER=grant\""));
        assert!(text.contains("research_style = \"deny\""));
        assert!(text.contains("research_style_dir = \"GALION_HORBM_RESEARCH_DIR\""));
        assert_eq!(UNKNOWN_REPLY, "I don't know.");
    }

    #[test]
    fn deny_flag_is_the_default_and_only_grant_is_grant() {
        let denied = RetrievalConfig::from_flags(None, None);
        assert!(!denied.remember);
        assert!(denied.knowledge_dir.is_none());
        assert_eq!(denied.fixture_rows, 0);
        assert!(denied.keys_path.is_none());
        assert!(keys_path_from_flag(None).is_none());
        assert!(keys_path_from_flag(Some("  ")).is_none());
        assert_eq!(
            keys_path_from_flag(Some(" keys.hkb ")).as_deref(),
            Some(std::path::Path::new("keys.hkb"))
        );
        assert!(!RetrievalConfig::from_flags(Some("yes"), None).remember);
        assert!(!RetrievalConfig::from_flags(Some("true"), None).remember);
        assert!(RetrievalConfig::from_flags(Some(" grant "), None).remember);
        let kept = RetrievalConfig::from_flags(Some("grant"), Some("/not/a/real/dir"));
        assert_eq!(
            kept.knowledge_dir.as_deref(),
            Some(std::path::Path::new("/not/a/real/dir"))
        );
    }

    /// Ported from the superseded phase-2 crate: a stopword-only ask is not a query.
    #[test]
    fn stopwords_alone_are_a_miss() {
        let terms = horbm_memory::index_tokens("the and of");
        assert!(terms.is_empty(), "{terms:?}");
        assert!(!covers("the and of a marker", &terms));
    }

    #[test]
    fn idle_is_empty_and_a_miss_is_not() {
        assert!(MemoryHitCell::idle().is_empty());
        let miss = MemoryHitCell::miss(3, 1, "tide", 12);
        assert!(!miss.is_empty());
        assert_eq!(miss.answer, UNKNOWN_REPLY);
        assert!(miss.hits.is_empty());
        assert_eq!(miss.state, CellState::Miss);
    }

    #[test]
    fn person_search_keeps_memory_and_drops_truth_cards() {
        let mut store = horbm_memory::Store::open_in_memory().expect("diary");
        store
            .upsert_cards(&[horbm_memory::synthetic_card(7)])
            .expect("card");
        store
            .remember(
                "my favourite colour is blue",
                "person-alpha",
                "t000001",
                "pin",
            )
            .expect("pin");
        store.drain().expect("drain");
        let people = search_person_lexical(&store, "favourite colour").expect("person search");
        assert!(!people.is_empty());
        assert!(people.iter().all(|hit| hit.pool == HitPool::Memory));
        assert!(people.iter().all(|hit| hit.item_id != "sample-7"));
        assert!(people.iter().any(|hit| hit.text.contains("blue")));
        let cards = store.search_lexical("marker m7").expect("card search");
        assert!(cards.hits.iter().any(|hit| hit.item_id == "sample-7"));
        assert!(cards.hits.iter().all(|hit| !hit.text.contains("favourite")));
    }

    #[test]
    fn an_older_epoch_does_not_replace_the_cell() {
        assert!(accept_cell(0, 0, false));
        assert!(accept_cell(1, 2, false));
        assert!(accept_cell(2, 2, false));
        assert!(!accept_cell(5, 4, false));
        assert!(accept_cell(5, 0, true));
    }
}
