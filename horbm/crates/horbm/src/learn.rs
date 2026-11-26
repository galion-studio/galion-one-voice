// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Offline learn pass for one HORBM pack.
//!
//! This is phase 7 of `docs/horbm/research/R4-cpu-realtime-integration.md`:
//! a separate learn consent, a source on every clause, a pack swap, and a
//! withdrawal that leaves no row. The diary is `horbm-memory`
//! ([`horbm_memory::Store`]). This module does not open a second store, does
//! not call [`horbm_memory::Stack::open`] (that creates `revisions.sqlite`),
//! and does not call [`horbm_memory::Store::forget`] (that keeps the text in
//! `archive`).
//!
//! Learn consent is not recall. `GALION_HORBM_LEARN=grant` is the only value
//! that lets this pass write. This module does not read a retrieval config.
//! `LOCKS.toml` `[consent] learn` is `"deny"` and `learn_in_band` is false.
//! This module does not parse that file. The default matches it. The process
//! grant does not rewrite the file, and it does not set an EmotionWire bit.
//!
//! Do not call [`LearnPass::write_clause`], [`LearnPass::withdraw`],
//! [`LearnPass::audit`], [`SessionSeal::note`], [`SessionSeal::note_taught`],
//! or [`SessionSeal::seal`] from `Heartbeat::step` or from [`crate::Brain::turn`].
//! The call never waits on this pass. The pass does not call Galion Research
//! and it does not read or write audio. [`SessionSeal`] keeps one clause in
//! RAM. It is not encrypted. The live pack keeps every sourced clause, up to
//! `LEARN_PACK_CAP`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::WINDOWS_KNOWLEDGE_DIR;

/// Directory name of the pack a reader may open. Writers do not open it.
pub const LEARN_PACK_LIVE: &str = "live";
/// Directory the pass fills, then renames onto [`LEARN_PACK_LIVE`].
pub const LEARN_PACK_NEXT: &str = "next";
/// Previous live pack, removed after a swap completes.
pub const LEARN_PACK_RETIRING: &str = "retiring";
/// SQLite file inside the live or next directory. `horbm-memory` owns the schema.
pub const LEARN_PACK_DB: &str = "learn-pack.sqlite";

/// Most rows a learn pack holds.
///
/// 256, the same number as S111's `LEARNED_ROW_CAP`. A further distinct clause
/// is refused. A sourced row is never dropped to make room.
pub const LEARN_PACK_CAP: usize = 256;

/// First words that mark a question. A word table, not a persona line.
const QUESTION_WORDS: [&str; 15] = [
    "what", "where", "who", "when", "why", "how", "is", "are", "do", "does", "can", "could",
    "would", "will", "did",
];

/// Provenance stored on each learned clause. Every field except `turn_id`
/// must be non-empty. `turn_id` may be zero.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClauseSource {
    pub session: String,
    pub turn_id: u64,
    pub time: String,
    pub consent_record: String,
}

impl ClauseSource {
    /// `session=…;turn=…;time=…;consent=…`. Empty pieces are refused.
    pub fn label(&self) -> Result<String, LearnError> {
        let session = self.session.trim();
        let time = self.time.trim();
        let consent = self.consent_record.trim();
        if session.is_empty() || time.is_empty() || consent.is_empty() {
            return Err(LearnError::unsourced("source is empty"));
        }
        Ok(format!(
            "session={session};turn={};time={time};consent={consent}",
            self.turn_id
        ))
    }
}

/// One diary row read back from the learn pack.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LearnedRow {
    pub item_id: String,
    pub text: String,
    pub said_by: String,
    pub date: String,
    pub source: String,
}

/// What [`LearnPass::audit`] sees. `archived` is the `horbm-memory` archive
/// table. A withdrawal does not use that table. It removes the pack.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LearnAudit {
    pub rows: Vec<LearnedRow>,
    pub archived: usize,
}

/// Learn consent and the directory that will hold `live/`. Not the card folder.
///
/// `granted` defaults to false. Recall consent is not a field of this config.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LearnConfig {
    pub granted: bool,
    pub pack_dir: PathBuf,
}

impl LearnConfig {
    /// Writes nothing. `pack_dir` is not created.
    pub fn fresh(pack_dir: impl AsRef<Path>) -> Self {
        Self {
            granted: false,
            pack_dir: pack_dir.as_ref().to_path_buf(),
        }
    }

    /// May write a sourced row into `pack_dir`. Still refused when that
    /// path is a card folder.
    pub fn granted(pack_dir: impl AsRef<Path>) -> Self {
        Self {
            granted: true,
            pack_dir: pack_dir.as_ref().to_path_buf(),
        }
    }

    /// `grant` after trim is the only grant. Any other flag, including
    /// `None`, is fresh. An empty pack path stays empty.
    pub fn from_flags(learn_flag: Option<&str>, pack: Option<&str>) -> Self {
        let granted = learn_flag.map(str::trim) == Some("grant");
        let pack_dir = match pack {
            Some(raw) if !raw.trim().is_empty() => PathBuf::from(raw.trim()),
            _ => PathBuf::new(),
        };
        Self { granted, pack_dir }
    }

    /// `GALION_HORBM_LEARN` and `GALION_HORBM_LEARN_PACK`. Does not read
    /// `GALION_HORBM_REMEMBER` or `GALION_HORBM_KNOWLEDGE`.
    pub fn from_env() -> Self {
        let flag = std::env::var("GALION_HORBM_LEARN").ok();
        let pack = std::env::var("GALION_HORBM_LEARN_PACK").ok();
        Self::from_flags(flag.as_deref(), pack.as_deref())
    }
}

/// Why a learn call did not publish a row.
#[derive(Debug)]
pub struct LearnError {
    kind: LearnKind,
    message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LearnKind {
    Denied,
    Unsourced,
    Invalid,
    Refused,
    Store,
}

impl LearnError {
    fn denied() -> Self {
        Self {
            kind: LearnKind::Denied,
            message: "learn consent is deny".to_string(),
        }
    }

    fn unsourced(message: impl Into<String>) -> Self {
        Self {
            kind: LearnKind::Unsourced,
            message: message.into(),
        }
    }

    fn invalid(message: impl Into<String>) -> Self {
        Self {
            kind: LearnKind::Invalid,
            message: message.into(),
        }
    }

    fn refused(message: impl Into<String>) -> Self {
        Self {
            kind: LearnKind::Refused,
            message: message.into(),
        }
    }

    fn store(err: horbm_memory::Error) -> Self {
        Self {
            kind: LearnKind::Store,
            message: err.to_string(),
        }
    }

    fn store_msg(message: impl Into<String>) -> Self {
        Self {
            kind: LearnKind::Store,
            message: message.into(),
        }
    }

    fn io(what: &str, err: io::Error) -> Self {
        Self {
            kind: LearnKind::Store,
            message: format!("{what}: {err}"),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    /// Fresh config. No file was created.
    pub fn is_denied(&self) -> bool {
        self.kind == LearnKind::Denied
    }

    /// Source session, time, or consent record was empty.
    pub fn is_unsourced(&self) -> bool {
        self.kind == LearnKind::Unsourced
    }

    /// Clause text or speaker was empty.
    pub fn is_invalid(&self) -> bool {
        self.kind == LearnKind::Invalid
    }

    /// The pack path is empty, the path is a card folder, or the pack already
    /// holds `LEARN_PACK_CAP` rows. Nothing was written.
    pub fn is_refused(&self) -> bool {
        self.kind == LearnKind::Refused
    }
}

impl std::fmt::Display for LearnError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for LearnError {}

/// Offline writer for one learn pack. Holds no diary between calls.
pub struct LearnPass {
    config: LearnConfig,
    forbidden: Vec<PathBuf>,
    gate: Mutex<()>,
}

impl LearnPass {
    /// Does not create `pack_dir` and does not open SQLite.
    pub fn open(config: LearnConfig) -> Self {
        Self {
            config,
            forbidden: Vec::new(),
            gate: Mutex::new(()),
        }
    }

    /// Same as [`LearnPass::open`]. Each path in `card_dirs` is a card folder
    /// this pass refuses to write or delete. Recall consent is not a parameter.
    pub fn open_outside(config: LearnConfig, card_dirs: &[PathBuf]) -> Self {
        Self {
            config,
            forbidden: card_dirs.to_vec(),
            gate: Mutex::new(()),
        }
    }

    pub fn is_granted(&self) -> bool {
        self.config.granted
    }

    /// `live/learn-pack.sqlite`. Does not create it.
    pub fn live_pack(&self) -> PathBuf {
        self.child(LEARN_PACK_LIVE).join(LEARN_PACK_DB)
    }

    /// Write one sourced clause into a sibling pack, then swap that directory
    /// onto `live/`.
    ///
    /// Rows already in the live pack are copied into `next/` (text, `said_by`,
    /// date, and source label), then the new clause is added. The pass checks
    /// the new pack has one more row than the old pack and that the new row's
    /// fields match, then swaps as before. An identical clause (the same
    /// trimmed text and the same trimmed `said_by`) is not added twice: the
    /// existing row is returned and the pack is not rewritten. At
    /// `LEARN_PACK_CAP` rows a further distinct clause is refused and the live
    /// rows stay. Fresh consent returns [`LearnError::is_denied`] and creates
    /// nothing.
    pub fn write_clause(
        &self,
        said_by: &str,
        text: &str,
        source: &ClauseSource,
    ) -> Result<LearnedRow, LearnError> {
        let _guard = self.gate.lock().expect("learn pack");
        if !self.config.granted {
            return Err(LearnError::denied());
        }
        let label = source.label()?;
        let said_by = said_by.trim();
        let text = text.trim();
        let date = source.time.trim();
        let session = source.session.trim();
        if said_by.is_empty() {
            return Err(LearnError::invalid("said_by is empty"));
        }
        if text.is_empty() {
            return Err(LearnError::invalid("text is empty"));
        }
        self.refuse_card_folder()?;
        self.prepare()?;
        let prior = self.live_rows()?;
        if let Some(existing) = prior.iter().find(|row| same_clause(row, said_by, text)) {
            return Ok(existing.clone());
        }
        if prior.len() >= LEARN_PACK_CAP {
            return Err(LearnError::refused(format!(
                "learn pack holds {LEARN_PACK_CAP} rows"
            )));
        }
        let next_db = self.child(LEARN_PACK_NEXT).join(LEARN_PACK_DB);
        let written = match insert_clause(&next_db, session, &prior, said_by, text, date, &label) {
            Ok(row) => row,
            Err(err) => {
                let _ = fs::remove_dir_all(self.child(LEARN_PACK_NEXT));
                return Err(err);
            }
        };
        if let Err(err) = self.swap_in() {
            let _ = fs::remove_dir_all(self.child(LEARN_PACK_NEXT));
            return Err(err);
        }
        Ok(written)
    }

    /// Delete the learn pack directories. After this, [`LearnPass::audit`]
    /// sees no row. Does not create a path that was absent. Refuses a card
    /// folder without deleting anything inside it.
    pub fn withdraw(&self) -> Result<(), LearnError> {
        let _guard = self.gate.lock().expect("learn pack");
        let pack = &self.config.pack_dir;
        if pack.as_os_str().is_empty() || !pack.exists() {
            return Ok(());
        }
        self.refuse_card_folder()?;
        for name in [LEARN_PACK_LIVE, LEARN_PACK_NEXT, LEARN_PACK_RETIRING] {
            let dir = self.child(name);
            if dir.exists() {
                fs::remove_dir_all(&dir)
                    .map_err(|err| LearnError::io("withdraw learn pack", err))?;
            }
        }
        Ok(())
    }

    /// Read the live pack. A missing file is an empty audit and is not created.
    pub fn audit(&self) -> Result<LearnAudit, LearnError> {
        let _guard = self.gate.lock().expect("learn pack");
        let db = self.live_pack();
        if !db.is_file() {
            return Ok(LearnAudit {
                rows: Vec::new(),
                archived: 0,
            });
        }
        read_audit(&db)
    }

    fn child(&self, name: &str) -> PathBuf {
        self.config.pack_dir.join(name)
    }

    /// Rows in the live pack. A missing file is an empty list and is not created.
    fn live_rows(&self) -> Result<Vec<LearnedRow>, LearnError> {
        let db = self.live_pack();
        if !db.is_file() {
            return Ok(Vec::new());
        }
        Ok(read_audit(&db)?.rows)
    }

    fn refuse_card_folder(&self) -> Result<(), LearnError> {
        let pack = &self.config.pack_dir;
        if pack.as_os_str().is_empty() {
            return Err(LearnError::refused("learn pack path is empty"));
        }
        if is_windows_knowledge(pack) {
            return Err(LearnError::refused(
                "learn pack is the card folder and is not written",
            ));
        }
        if pack.join("manifest.json").is_file() || pack.join("revisions.sqlite").is_file() {
            return Err(LearnError::refused(
                "learn pack is a card folder and is not written",
            ));
        }
        for dir in &self.forbidden {
            if dir.as_os_str().is_empty() {
                continue;
            }
            if same_dir(pack, dir) || pack.starts_with(dir) {
                return Err(LearnError::refused(
                    "learn pack is inside the card folder and is not written",
                ));
            }
        }
        Ok(())
    }

    fn prepare(&self) -> Result<(), LearnError> {
        let live = self.child(LEARN_PACK_LIVE);
        let next = self.child(LEARN_PACK_NEXT);
        let retiring = self.child(LEARN_PACK_RETIRING);
        if retiring.exists() && !live.exists() {
            fs::rename(&retiring, &live)
                .map_err(|err| LearnError::io("restore learn pack", err))?;
        }
        if next.exists() {
            fs::remove_dir_all(&next)
                .map_err(|err| LearnError::io("clear next learn pack", err))?;
        }
        Ok(())
    }

    fn swap_in(&self) -> Result<(), LearnError> {
        let live = self.child(LEARN_PACK_LIVE);
        let next = self.child(LEARN_PACK_NEXT);
        let retiring = self.child(LEARN_PACK_RETIRING);
        if retiring.exists() {
            fs::remove_dir_all(&retiring)
                .map_err(|err| LearnError::io("clear retiring learn pack", err))?;
        }
        if live.exists() {
            fs::rename(&live, &retiring).map_err(|err| LearnError::io("retire learn pack", err))?;
        }
        if let Err(err) = fs::rename(&next, &live) {
            if !live.exists() && retiring.exists() {
                let _ = fs::rename(&retiring, &live);
            }
            return Err(LearnError::io("swap learn pack", err));
        }
        if retiring.exists() {
            fs::remove_dir_all(&retiring)
                .map_err(|err| LearnError::io("remove retired learn pack", err))?;
        }
        Ok(())
    }
}

/// One clause held until [`SessionSeal::seal`]. Plain text. Not a diary row.
struct NotedClause {
    said_by: String,
    text: String,
    source: ClauseSource,
}

/// One RAM slot in front of [`LearnPass`].
///
/// [`SessionSeal::note`] keeps one clause and replaces it on a second note.
/// [`SessionSeal::note_taught`] keeps `heard` only when it teaches the previous
/// miss. [`SessionSeal::seal`] calls [`LearnPass::write_clause`] once and clears
/// the slot. A later seal, with the slot empty, writes nothing. A denied
/// pass writes nothing and creates no file. The slot is plain text. This
/// type does not encrypt. Not called from [`crate::Brain::turn`].
// Nothing outside this module names this type yet. The allow keeps a
// non-test build quiet. The learn tests construct it.
#[cfg_attr(not(test), allow(dead_code))]
pub struct SessionSeal {
    pass: LearnPass,
    slot: Option<NotedClause>,
}

#[cfg_attr(not(test), allow(dead_code))]
impl SessionSeal {
    /// Does not create `pack_dir` and does not open SQLite.
    pub fn open(config: LearnConfig) -> Self {
        Self {
            pass: LearnPass::open(config),
            slot: None,
        }
    }

    /// Keep one clause in RAM. A second call replaces it. Nothing is written.
    pub fn note(&mut self, said_by: &str, text: &str, source: &ClauseSource) {
        self.slot = Some(NotedClause {
            said_by: said_by.to_string(),
            text: text.to_string(),
            source: source.clone(),
        });
    }

    /// Keep `heard` when it teaches the missed turn, and return whether the slot kept it.
    ///
    /// The slot keeps `heard` only when all three hold. `missed` is non-empty, so the
    /// previous turn was a miss. `heard` is not a question: it contains no `?`, and its
    /// first word is not in the fixed English table `what`, `where`, `who`, `when`,
    /// `why`, `how`, `is`, `are`, `do`, `does`, `can`, `could`, `would`, `will`, `did`.
    /// That table is a word list, not a persona line. The first word is the first
    /// whitespace-separated token, with non-letters trimmed off its ends, compared
    /// without regard to ASCII case. `crate::retrieve::covers` is true for `heard`
    /// against [`horbm_memory::index_tokens`] of `missed`, the same rule recall uses,
    /// so the kept line is a row that question will later find.
    ///
    /// A kept line replaces the one clause in the slot. A line that is not kept leaves
    /// the slot as it was. Nothing is written. Not called from [`crate::Brain::turn`].
    pub fn note_taught(
        &mut self,
        missed: &str,
        said_by: &str,
        heard: &str,
        source: &ClauseSource,
    ) -> bool {
        if !taught_line(missed, heard) {
            return false;
        }
        self.note(said_by, heard, source);
        true
    }

    /// Write the held clause once and clear the slot.
    ///
    /// An empty slot returns `Ok(None)` and does not open the pack. A full
    /// slot calls [`LearnPass::write_clause`] once. The slot is cleared
    /// either way, so a second call writes nothing. Denial returns
    /// [`LearnError::is_denied`] and creates no file.
    pub fn seal(&mut self) -> Result<Option<LearnedRow>, LearnError> {
        let Some(held) = self.slot.take() else {
            return Ok(None);
        };
        self.pass
            .write_clause(&held.said_by, &held.text, &held.source)
            .map(Some)
    }
}

fn insert_clause(
    db: &Path,
    session: &str,
    prior: &[LearnedRow],
    said_by: &str,
    text: &str,
    date: &str,
    label: &str,
) -> Result<LearnedRow, LearnError> {
    let mut store = horbm_memory::Store::open(db).map_err(LearnError::store)?;
    store.pause_worker();
    let result = (|| {
        store.set_session(session).map_err(LearnError::store)?;
        for row in prior {
            store
                .diary_add(
                    row.text.as_str(),
                    row.date.as_str(),
                    row.said_by.as_str(),
                    row.source.as_str(),
                )
                .map_err(LearnError::store)?;
        }
        store
            .diary_add(text, date, said_by, label)
            .map_err(LearnError::store)?;
        checked_row(&store, prior, said_by, text, date, label)
    })();
    drop(store);
    result
}

fn read_audit(db: &Path) -> Result<LearnAudit, LearnError> {
    let store = horbm_memory::Store::open(db).map_err(LearnError::store)?;
    store.pause_worker();
    let result = (|| {
        let rows = store.diary_list().map_err(LearnError::store)?;
        let archived = store.archive_list().map_err(LearnError::store)?;
        Ok(LearnAudit {
            rows: rows.into_iter().map(learned_row).collect(),
            archived: archived.len(),
        })
    })();
    drop(store);
    result
}

fn checked_row(
    store: &horbm_memory::Store,
    prior: &[LearnedRow],
    said_by: &str,
    text: &str,
    date: &str,
    label: &str,
) -> Result<LearnedRow, LearnError> {
    let rows = store.diary_list().map_err(LearnError::store)?;
    let expected = prior.len() + 1;
    if rows.len() != expected {
        return Err(LearnError::store_msg(format!(
            "learn pack has {} diary rows",
            rows.len()
        )));
    }
    let archived = store.archive_list().map_err(LearnError::store)?;
    if !archived.is_empty() {
        return Err(LearnError::store_msg(format!(
            "learn pack archive has {} rows",
            archived.len()
        )));
    }
    let rows: Vec<LearnedRow> = rows.into_iter().map(learned_row).collect();
    let mut used = vec![false; rows.len()];
    for old in prior {
        let Some(idx) = rows.iter().enumerate().position(|(i, row)| {
            !used[i]
                && !row.item_id.is_empty()
                && row.text == old.text
                && row.said_by == old.said_by
                && row.date == old.date
                && row.source == old.source
        }) else {
            return Err(LearnError::store_msg(
                "stored row does not match the sourced clause",
            ));
        };
        used[idx] = true;
    }
    let Some(idx) = used.iter().position(|flag| !*flag) else {
        return Err(LearnError::store_msg("learn pack diary row missing"));
    };
    let row = &rows[idx];
    if row.item_id.is_empty()
        || row.source != label
        || row.text != text
        || row.said_by != said_by
        || row.date != date
    {
        return Err(LearnError::store_msg(
            "stored row does not match the sourced clause",
        ));
    }
    Ok(row.clone())
}

/// Trimmed text and trimmed speaker. That is the clause identity.
fn same_clause(row: &LearnedRow, said_by: &str, text: &str) -> bool {
    row.said_by.trim() == said_by && row.text.trim() == text
}

/// `missed` names a previous miss, and `heard` is a non-question line that covers it.
fn taught_line(missed: &str, heard: &str) -> bool {
    if missed.trim().is_empty() {
        return false;
    }
    if is_question(heard) {
        return false;
    }
    crate::retrieve::covers(heard, &horbm_memory::index_tokens(missed))
}

fn is_question(heard: &str) -> bool {
    if heard.contains('?') {
        return true;
    }
    let Some(word) = first_word(heard) else {
        return false;
    };
    QUESTION_WORDS
        .iter()
        .any(|question| word.eq_ignore_ascii_case(question))
}

/// First whitespace-separated token, with non-letters trimmed off its ends.
fn first_word(heard: &str) -> Option<&str> {
    let token = heard.split_whitespace().next()?;
    let word = token.trim_matches(|ch: char| !ch.is_ascii_alphabetic());
    if word.is_empty() {
        None
    } else {
        Some(word)
    }
}

fn learned_row(entry: horbm_memory::DiaryEntry) -> LearnedRow {
    LearnedRow {
        item_id: entry.item_id,
        text: entry.text,
        said_by: entry.said_by,
        date: entry.date,
        source: entry.source,
    }
}

fn is_windows_knowledge(path: &Path) -> bool {
    if path == Path::new(WINDOWS_KNOWLEDGE_DIR) {
        return true;
    }
    path.to_string_lossy()
        .replace('/', "\\")
        .eq_ignore_ascii_case(WINDOWS_KNOWLEDGE_DIR)
}

fn same_dir(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source() -> ClauseSource {
        ClauseSource {
            session: "sess-7".to_string(),
            turn_id: 7,
            time: "2026-11-18T21:00:00Z".to_string(),
            consent_record: "learn-v1".to_string(),
        }
    }

    #[test]
    fn from_flags_grants_only_the_exact_word() {
        assert!(!LearnConfig::from_flags(None, Some("/tmp/learn")).granted);
        assert!(!LearnConfig::from_flags(Some("deny"), Some("/tmp/learn")).granted);
        assert!(!LearnConfig::from_flags(Some("GRANT"), Some("/tmp/learn")).granted);
        assert!(LearnConfig::from_flags(Some(" grant "), Some(" /tmp/learn ")).granted);
        assert_eq!(
            LearnConfig::from_flags(Some("grant"), Some(" /tmp/learn ")).pack_dir,
            PathBuf::from("/tmp/learn")
        );
        assert!(LearnConfig::from_flags(Some("grant"), None)
            .pack_dir
            .as_os_str()
            .is_empty());
    }

    #[test]
    fn label_requires_session_time_and_consent_record() {
        let label = source().label().expect("label");
        assert_eq!(
            label,
            "session=sess-7;turn=7;time=2026-11-18T21:00:00Z;consent=learn-v1"
        );
        let mut blank = source();
        blank.consent_record = "  ".to_string();
        assert!(blank.label().unwrap_err().is_unsourced());
    }

    #[test]
    fn empty_pack_path_is_refused_without_a_file() {
        let pass = LearnPass::open(LearnConfig::from_flags(Some("grant"), None));
        let err = pass
            .write_clause("user", "The kettle is on the hob.", &source())
            .expect_err("empty pack");
        assert!(err.is_refused());
        assert!(!Path::new("learn-pack.sqlite").exists());
        assert!(!Path::new("revisions.sqlite").exists());
    }

    fn scratch(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("horbm-seal-{name}-{}-{nanos}", std::process::id()));
        fs::create_dir_all(&dir).expect("scratch");
        dir
    }

    fn sqlite_under(dir: &Path) -> Vec<String> {
        let mut out = Vec::new();
        fn walk(dir: &Path, out: &mut Vec<String>) {
            let Ok(rd) = fs::read_dir(dir) else {
                return;
            };
            for ent in rd.flatten() {
                let path = ent.path();
                if path.is_dir() {
                    walk(&path, out);
                    continue;
                }
                let name = ent.file_name().to_string_lossy().to_ascii_lowercase();
                if name.contains("sqlite") || name.ends_with(".db") || name.contains("horbm-mem") {
                    out.push(path.to_string_lossy().into_owned());
                }
            }
        }
        walk(dir, &mut out);
        out.sort();
        out
    }

    #[test]
    fn seal_writes_one_row_and_a_second_seal_writes_nothing() {
        let root = scratch("one");
        let pack = root.join("pack");
        let mut first = source();
        first.turn_id = 4;
        let mut second = source();
        second.turn_id = 5;
        let alpha = "alpha-token the kettle is on the hob.";
        let beta = "beta-token the kettle is off the hob.";

        let mut seal = SessionSeal::open(LearnConfig::granted(&pack));
        assert!(seal.seal().expect("empty slot").is_none());
        assert!(!pack.exists(), "empty seal created {}", pack.display());

        seal.note("user", alpha, &first);
        seal.note("user", beta, &second);
        let held = seal.slot.as_ref().expect("one slot");
        assert_eq!(held.text, beta);
        assert_eq!(held.said_by, "user");
        assert_eq!(held.source.turn_id, 5);
        assert!(!pack.exists(), "note created {}", pack.display());
        assert!(sqlite_under(&root).is_empty());

        let row = seal.seal().expect("seal").expect("one row");
        let label = second.label().expect("label");
        assert_eq!(row.text, beta);
        assert_eq!(row.source, label);
        assert_eq!(row.said_by, "user");
        assert_eq!(row.date, second.time);
        assert!(!row.text.contains("alpha-token"));
        assert!(seal.slot.is_none());
        let audit = seal.pass.audit().expect("audit");
        assert_eq!(audit.rows.len(), 1);
        assert_eq!(audit.archived, 0);
        assert_eq!(audit.rows[0], row);
        assert!(seal.pass.live_pack().is_file());
        assert!(!pack.join(LEARN_PACK_NEXT).exists());
        assert!(!pack.join(LEARN_PACK_RETIRING).exists());
        assert!(!pack.join("revisions.sqlite").exists());

        let marker = seal.pass.live_pack().with_file_name("kept.txt");
        fs::write(&marker, b"kept").expect("marker");
        assert!(seal.seal().expect("second seal").is_none());
        assert_eq!(fs::read(&marker).expect("marker stayed"), b"kept");
        let again = seal.pass.audit().expect("audit after second seal");
        assert_eq!(again.rows.len(), 1);
        assert_eq!(again.archived, 0);
        assert_eq!(again.rows[0], row);
        assert!(!pack.join(LEARN_PACK_NEXT).exists());
        assert!(!pack.join(LEARN_PACK_RETIRING).exists());
        assert!(!pack.join("revisions.sqlite").exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn unset_learn_seal_creates_no_pack_file() {
        let root = scratch("unset");
        let pack = root.join("pack");
        let pack_flag = pack.to_str().expect("utf-8 pack path");
        // None is an unset GALION_HORBM_LEARN. Only the word "grant" may write.
        let mut seal = SessionSeal::open(LearnConfig::from_flags(None, Some(pack_flag)));
        assert!(!seal.pass.is_granted());
        seal.note("user", "The kettle is on the hob.", &source());
        let err = seal.seal().expect_err("unset learn");
        assert!(err.is_denied(), "{err}");
        assert!(seal.slot.is_none(), "a refused seal still clears the slot");
        assert!(!pack.exists(), "unset learn created {}", pack.display());
        assert!(
            sqlite_under(&root).is_empty(),
            "unset learn left {:?}",
            sqlite_under(&root)
        );
        assert!(seal.seal().expect("empty after deny").is_none());
        assert!(!pack.exists());
        assert!(sqlite_under(&root).is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn learn_pack_cap_is_the_same_number_as_learned_row_cap() {
        assert_eq!(LEARN_PACK_CAP, 256);
    }

    #[test]
    fn question_words_are_the_fixed_english_table() {
        assert!(!is_question("the kettle is on the stove"));
        assert!(is_question("the kettle is on the stove?"));
        assert!(is_question("Where is the kettle"));
        for word in QUESTION_WORDS {
            assert!(is_question(word), "{word}");
            let line = format!("{word} the kettle is on the stove");
            assert!(is_question(&line), "{word}");
            assert!(!taught_line("where is the kettle", &line), "{word}");
        }
    }

    #[test]
    fn note_taught_keeps_a_covering_line_and_leaves_the_slot_on_a_miss() {
        let root = scratch("taught");
        let pack = root.join("pack");
        let missed = "where is the kettle";
        let heard = "the kettle is on the stove";
        let mut seal = SessionSeal::open(LearnConfig::granted(&pack));
        assert!(seal.note_taught(missed, "user", heard, &source()));
        assert_eq!(seal.slot.as_ref().expect("kept").text, heard);
        assert!(!pack.exists(), "note_taught created {}", pack.display());

        assert!(!seal.note_taught(missed, "user", "where is the kettle", &source()));
        assert!(!seal.note_taught("", "user", heard, &source()));
        assert!(!seal.note_taught("   ", "user", heard, &source()));
        assert!(!seal.note_taught(missed, "user", "the window is open", &source()));
        assert!(!seal.note_taught(missed, "user", "the kettle is on the stove?", &source()));
        assert_eq!(seal.slot.as_ref().expect("still kept").text, heard);

        let row = seal.seal().expect("seal").expect("taught row");
        assert_eq!(row.text, heard);
        assert_eq!(seal.pass.audit().expect("audit").rows.len(), 1);
        let _ = fs::remove_dir_all(&root);
    }
}
