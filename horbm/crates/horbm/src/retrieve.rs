// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Retrieval thread for one HORBM turn.
//!
//! This is phase 2 of `docs/horbm/research/R4-cpu-realtime-integration.md`:
//! a RAM fixture, `remember` denied unless granted, and a cell published off
//! the heartbeat thread. A granted ask scores with [`horbm_brain::DiaryScan`]:
//! the HKB1 file when one is configured, and the person-diary rows loaded
//! from the in-memory store. `Stack::open` is not used: it creates
//! `revisions.sqlite`.
//!
//! The scan writes into a `[ScanHit; MEMORY_HIT_CAP]` the worker allocates
//! once. After the grant builds the scan, that call does not allocate.
//! Publishing the cell still builds [`MemoryHit`] strings.
//!
//! Person memory is a second diary. [`search_person_lexical`] reads that
//! file with FTS5. This thread does not mix that file into the scan. A hit
//! whose pool is not memory is dropped from the person search.
//!
//! `LOCKS.toml` `[consent] remember = "deny"`. `[horbm] remember_grant` names
//! the only grant string. This module does not read the lock file. The
//! default matches it. `GALION_HORBM_REMEMBER=grant` is the explicit grant
//! for a process that is allowed to open the diary. `GALION_HORBM_KEYS` is
//! the HKB1 file, opened only while that grant holds.
//!
//! Do not call [`RetrievalThread::ask`], [`RetrievalThread::ask_wait`],
//! [`RetrievalThread::set_remember`], or [`RetrievalThread::cell`] from
//! `Heartbeat::step`. That step does not allocate and does not block.
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
    DiaryArena, DiaryRow, DiaryScan, KeyPrefault, ScanHit, ScanPool, StoredKeys, StoredPack,
    DIARY_DIM, SCAN_CAP,
};

use crate::UNKNOWN_REPLY;
use crate::WINDOWS_KNOWLEDGE_DIR;

/// Published hits. Same cap as `horbm_memory::MAX_HITS`.
pub const MEMORY_HIT_CAP: usize = 4;

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
/// A granted ask calls [`DiaryScan::scan_selected_into`] instead.
/// [`crate::keys`] still implements this for a version-1 `StringKeyArena`
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
    /// HKB1 file. Opened only while `remember` is granted. Not created.
    /// `None` scores no key rows.
    pub keys_path: Option<PathBuf>,
}

impl RetrievalConfig {
    /// `GALION_HORBM_REMEMBER=grant` grants. Any other value, including an
    /// unset variable, denies. `GALION_HORBM_KNOWLEDGE` is the card directory
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
}

impl Command {
    fn take_done(self) -> Option<Sender<()>> {
        match self {
            Command::Ask { done, .. } | Command::Consent { done, .. } => done,
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

    /// Queue an ask. Does not wait. A newer ask or a consent change replaces
    /// an ask that has not started.
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
    /// cell. Grant opens an in-memory diary and loads the fixture or the card
    /// directory from the config passed to [`RetrievalThread::spawn`].
    pub fn set_remember(&self, grant: bool) -> Result<(), RetrieveError> {
        self.wait(Command::Consent { grant, done: None })
    }

    /// Last diary error. Cleared on a successful grant and on withdraw.
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
            Command::Ask { done, .. } | Command::Consent { done, .. } => *done = Some(tx),
        }
        self.enqueue(cmd);
        rx.recv()
            .map_err(|_| RetrieveError::new("retrieval thread stopped"))
    }

    fn enqueue(&self, cmd: Command) {
        let mut slot = self.shared.slot.lock().expect("retrieval mailbox");
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
        match open_store(&config) {
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
                apply_consent(shared, &mut store, &config, grant);
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
            *shared.last_error.lock().expect("retrieval error") = Some(err.message);
            publish(shared, MemoryHitCell::miss(turn_id, epoch, text, 0), false);
        }
    }
}

fn apply_consent(
    shared: &Shared,
    store: &mut Option<Opened>,
    config: &RetrievalConfig,
    grant: bool,
) {
    if grant {
        if store.is_none() {
            match open_store(config) {
                Ok(opened) => {
                    *store = Some(opened);
                    *shared.last_error.lock().expect("retrieval error") = None;
                }
                Err(err) => {
                    *shared.last_error.lock().expect("retrieval error") = Some(err.message);
                }
            }
        }
        return;
    }
    *store = None;
    *shared.last_error.lock().expect("retrieval error") = None;
    publish(shared, MemoryHitCell::idle(), true);
}

struct Opened {
    scan: DiaryScan,
    diary: Vec<CatalogRow>,
    keys: Vec<CatalogRow>,
    /// Card ids that must not be published. Also stored as `allow: false`.
    blocked: BTreeSet<String>,
    /// In-memory diary. Dropped with the scan when the grant falls.
    _store: horbm_memory::Store,
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

impl CatalogRow {
    fn named(item_id: &str) -> Self {
        Self {
            item_id: item_id.to_string(),
            title: String::new(),
            text: String::new(),
            said_by: String::new(),
            date: String::new(),
            source: String::new(),
            claim_type: String::new(),
            pinned: false,
            haystack: item_id.to_string(),
        }
    }
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
    let diary_rows = embed_store(&store, &blocked)?;
    let diary = DiaryArena::from_rows(diary_rows.iter().map(|row| DiaryRow {
        item_id: row.item_id.as_str(),
        values: row.values.as_slice(),
        scale: row.scale,
        norm: row.norm,
        allow: row.allow,
    }))
    .map_err(|err| RetrieveError::new(format!("diary arena: {err}")))?;
    let (keys, key_rows) = load_keys(config.keys_path.as_deref(), &diary_rows)?;
    // The grant is what opened this store. `remember` false leaves the diary
    // leg out; callers only reach here while the grant holds.
    let scan = DiaryScan::open(keys, diary, config.remember, SCAN_CAP);
    let diary_catalog = diary_rows.into_iter().map(|row| row.catalog).collect();
    Ok(Opened {
        scan,
        diary: diary_catalog,
        keys: key_rows,
        blocked,
        _store: store,
    })
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

/// Width of the in-memory key pack built when no HKB1 file is configured.
const MADE_KEY_DIM: usize = 32;

fn load_keys(
    path: Option<&Path>,
    diary_rows: &[EmbeddedRow],
) -> Result<(StoredKeys, Vec<CatalogRow>), RetrieveError> {
    let keys = match path {
        Some(path) => StoredKeys::open(path, KeyPrefault::Populate)
            .map_err(|err| RetrieveError::new(format!("open keys: {err}")))?,
        None => keys_for_rows(diary_rows)?,
    };
    let mut rows = Vec::with_capacity(keys.len());
    for index in 0..keys.len() {
        let id = keys.item_id(index).ok().flatten().unwrap_or("");
        let catalog = diary_rows
            .iter()
            .find(|row| row.item_id == id)
            .map(|row| row.catalog.clone())
            .unwrap_or_else(|| CatalogRow::named(id));
        rows.push(catalog);
    }
    Ok((keys, rows))
}

/// One key per diary row, used only when no HKB1 file is configured.
///
/// The vector is `encode_query_into` of that row's rarest content token, so a
/// one-token ask for the token ranks the row. The diary leg still embeds the
/// full line. This runs at setup.
fn keys_for_rows(rows: &[EmbeddedRow]) -> Result<StoredKeys, RetrieveError> {
    if rows.is_empty() {
        return empty_keys();
    }
    let frequency = document_frequency(rows);
    let n = rows.len();
    let mut packed = vec![0i8; n * MADE_KEY_DIM];
    let mut allow = Vec::with_capacity(n);
    let mut item_id = Vec::with_capacity(n);
    let mut clause_id = Vec::with_capacity(n);
    let source = vec![0u64; n];
    let scale = vec![1i8; n];
    for (i, row) in rows.iter().enumerate() {
        let probe =
            rarest_token(&row.catalog.haystack, &frequency).unwrap_or_else(|| row.item_id.clone());
        let start = i * MADE_KEY_DIM;
        horbm_brain::encode_query_into(&probe, &mut packed[start..start + MADE_KEY_DIM]);
        allow.push(row.allow);
        item_id.push(row.item_id.clone());
        clause_id.push(u64::try_from(i).unwrap_or(u64::MAX));
    }
    let pack = StoredPack {
        k: MADE_KEY_DIM,
        keys: packed,
        clause_id,
        allow,
        source,
        item_id,
        scale,
    };
    let bytes = pack
        .encode()
        .map_err(|err| RetrieveError::new(format!("key pack: {err}")))?;
    StoredKeys::from_bytes(&bytes).map_err(|err| RetrieveError::new(format!("key pack: {err}")))
}

fn document_frequency(rows: &[EmbeddedRow]) -> HashMap<String, usize> {
    let mut frequency = HashMap::new();
    for row in rows {
        let mut seen = BTreeSet::new();
        for token in horbm_memory::index_tokens(&row.catalog.haystack) {
            if seen.insert(token.clone()) {
                *frequency.entry(token).or_insert(0) += 1;
            }
        }
    }
    frequency
}

/// Lowest document frequency, then the longer token, then byte order.
fn rarest_token(haystack: &str, frequency: &HashMap<String, usize>) -> Option<String> {
    let mut best: Option<String> = None;
    for token in horbm_memory::index_tokens(haystack) {
        let replace = match best.as_deref() {
            None => true,
            Some(current) => {
                let next_df = frequency.get(&token).copied().unwrap_or(0);
                let current_df = frequency.get(current).copied().unwrap_or(0);
                next_df < current_df
                    || (next_df == current_df && token.len() > current.len())
                    || (next_df == current_df
                        && token.len() == current.len()
                        && token.as_str() < current)
            }
        };
        if replace {
            best = Some(token);
        }
    }
    best
}

fn empty_keys() -> Result<StoredKeys, RetrieveError> {
    let pack = StoredPack {
        k: 1,
        keys: Vec::new(),
        clause_id: Vec::new(),
        allow: Vec::new(),
        source: Vec::new(),
        item_id: Vec::new(),
        scale: Vec::new(),
    };
    let bytes = pack
        .encode()
        .map_err(|err| RetrieveError::new(format!("empty keys: {err}")))?;
    StoredKeys::from_bytes(&bytes).map_err(|err| RetrieveError::new(format!("empty keys: {err}")))
}

fn asked(
    opened: &mut Opened,
    hit_buf: &mut [ScanHit; MEMORY_HIT_CAP],
    text: &str,
    turn_id: u64,
    epoch: u64,
) -> Result<MemoryHitCell, RetrieveError> {
    let started = Instant::now();
    // Caller-owned buffer. This call does not allocate.
    let n = opened.scan.scan_selected_into(text, hit_buf);
    let took_us = elapsed_us(started.elapsed());
    let terms = horbm_memory::index_tokens(text);
    let mut hits = Vec::new();
    for hit in &hit_buf[..n] {
        let hit = *hit;
        let Some(item_id) = opened.scan.item_id(hit).map(str::to_string) else {
            continue;
        };
        if opened.blocked.contains(&item_id) {
            continue;
        }
        let Some(row) = catalog_row(opened, hit) else {
            continue;
        };
        if row.item_id != item_id || !covers(&row.haystack, &terms) {
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

fn catalog_row(opened: &Opened, hit: ScanHit) -> Option<&CatalogRow> {
    let rows = match hit.pool {
        ScanPool::Diary => &opened.diary,
        ScanPool::Keys => &opened.keys,
    };
    rows.get(hit.row as usize)
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
        Some(Command::Consent { .. }) => true,
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
        assert_eq!(MEMORY_HIT_CAP, SCAN_CAP);
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
