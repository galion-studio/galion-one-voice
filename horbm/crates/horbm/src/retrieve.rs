// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Retrieval thread for one HORBM turn.
//!
//! This is phase 2 of `docs/horbm/research/R4-cpu-realtime-integration.md`:
//! FTS5 in-process, a RAM fixture, `remember` denied unless granted, and a
//! cell published off the heartbeat thread. No mapped int8 arena. The diary
//! is `horbm-memory` (`Store::open_in_memory` and `Store::search_lexical`).
//! `Stack::open` is not used: it creates `revisions.sqlite`.
//!
//! `LOCKS.toml` `[consent] remember = "deny"`. `[horbm] remember_grant` names
//! the only grant string. This module does not read the lock file. The
//! default matches it. `GALION_HORBM_REMEMBER=grant` is the explicit grant
//! for a process that is allowed to open the diary.
//!
//! Do not call [`RetrievalThread::ask`], [`RetrievalThread::ask_wait`],
//! [`RetrievalThread::set_remember`], or [`RetrievalThread::cell`] from
//! `Heartbeat::step`. That step does not allocate and does not block.
//! [`RetrievalThread::try_cell`] is the non-waiting read for a conductor
//! thread between frames. A `None` means the lock was busy, not that the
//! cell is empty.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::thread::{self, JoinHandle};

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

/// F3's mapped int8 arena. This phase does not implement it and does not
/// call it. `search_lexical` does not scan the diary's hashing vectors.
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
}

impl RetrievalConfig {
    /// `GALION_HORBM_REMEMBER=grant` grants. Any other value, including an
    /// unset variable, denies. `GALION_HORBM_KNOWLEDGE` is the card directory
    /// when it is set. When it is unset, [`WINDOWS_KNOWLEDGE_DIR`] is used
    /// only if that directory already exists.
    pub fn from_env() -> Self {
        let flag = std::env::var("GALION_HORBM_REMEMBER").ok();
        let knowledge = std::env::var("GALION_HORBM_KNOWLEDGE").ok();
        Self::from_flags(flag.as_deref(), knowledge.as_deref())
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
        }
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
    while let Some(cmd) = recv(shared) {
        match cmd {
            Command::Ask {
                text,
                turn_id,
                epoch,
                done,
            } => {
                if !superseded(shared, epoch) {
                    publish_ask(shared, store.as_ref(), &text, turn_id, epoch);
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

fn publish_ask(shared: &Shared, store: Option<&Opened>, text: &str, turn_id: u64, epoch: u64) {
    let Some(opened) = store else {
        publish(shared, MemoryHitCell::idle(), true);
        return;
    };
    match asked(&opened.store, &opened.blocked, text, turn_id, epoch) {
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
    store: horbm_memory::Store,
    /// Card ids that must not be published as `Hits[0]`.
    blocked: std::collections::BTreeSet<String>,
}

fn open_store(config: &RetrievalConfig) -> Result<Opened, RetrieveError> {
    let mut store = horbm_memory::Store::open_in_memory().map_err(RetrieveError::memory)?;
    if config.fixture_rows > 0 {
        let cards: Vec<_> = (0..config.fixture_rows)
            .map(horbm_memory::synthetic_card)
            .collect();
        store.upsert_cards(&cards).map_err(RetrieveError::memory)?;
        return Ok(Opened {
            store,
            blocked: std::collections::BTreeSet::new(),
        });
    }
    let blocked = match &config.knowledge_dir {
        Some(dir) => {
            let blocked = crate::cards::blocked_answer_ids(dir);
            store.ingest_dir(dir).map_err(RetrieveError::memory)?;
            blocked
        }
        None => std::collections::BTreeSet::new(),
    };
    Ok(Opened { store, blocked })
}

fn asked(
    store: &horbm_memory::Store,
    blocked: &std::collections::BTreeSet<String>,
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

/// Same predicate as `horbm-memory` `stack.rs` `covers`: every content token
/// of the query is in the hit haystack. An empty term list does not match.
fn covers(haystack: &str, terms: &[String]) -> bool {
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
        assert_eq!(UNKNOWN_REPLY, "I don't know.");
    }

    #[test]
    fn deny_flag_is_the_default_and_only_grant_is_grant() {
        let denied = RetrievalConfig::from_flags(None, None);
        assert!(!denied.remember);
        assert!(denied.knowledge_dir.is_none());
        assert_eq!(denied.fixture_rows, 0);
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
    fn an_older_epoch_does_not_replace_the_cell() {
        assert!(accept_cell(0, 0, false));
        assert!(accept_cell(1, 2, false));
        assert!(accept_cell(2, 2, false));
        assert!(!accept_cell(5, 4, false));
        assert!(accept_cell(5, 0, true));
    }
}
