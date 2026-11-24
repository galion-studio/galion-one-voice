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
//! `LOCKS.toml` records the deny default. This module does not parse that file.
//!
//! Do not call [`LearnPass::write_clause`], [`LearnPass::withdraw`], or
//! [`LearnPass::audit`] from `Heartbeat::step` or from the `horbm-retrieval`
//! thread. The call never waits on this pass. The pass does not call Galion
//! Research and it does not read or write audio.

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

/// Provenance stored on the one learned clause. Every field except `turn_id`
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

    /// May write one sourced row into `pack_dir`. Still refused when that
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

    /// The pack path is empty or is a card folder. Nothing was written.
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
    /// onto `live/`. A second call replaces the live pack, so the live pack
    /// still holds one row. Fresh consent returns [`LearnError::is_denied`]
    /// and creates nothing.
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
        let next_db = self.child(LEARN_PACK_NEXT).join(LEARN_PACK_DB);
        let written = match insert_one(&next_db, session, said_by, text, date, &label) {
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

fn insert_one(
    db: &Path,
    session: &str,
    said_by: &str,
    text: &str,
    date: &str,
    label: &str,
) -> Result<LearnedRow, LearnError> {
    let mut store = horbm_memory::Store::open(db).map_err(LearnError::store)?;
    store.pause_worker();
    let result = (|| {
        store.set_session(session).map_err(LearnError::store)?;
        store
            .diary_add(text, date, said_by, label)
            .map_err(LearnError::store)?;
        checked_row(&store, said_by, text, date, label)
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
    said_by: &str,
    text: &str,
    date: &str,
    label: &str,
) -> Result<LearnedRow, LearnError> {
    let rows = store.diary_list().map_err(LearnError::store)?;
    if rows.len() != 1 {
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
    let Some(entry) = rows.into_iter().next() else {
        return Err(LearnError::store_msg("learn pack diary row missing"));
    };
    let row = learned_row(entry);
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
    Ok(row)
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
}
