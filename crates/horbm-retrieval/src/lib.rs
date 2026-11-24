// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Phase 2 retrieval thread.
//!
//! FTS5 runs in-process over a RAM database. The schema of `items_fts` is the
//! one in `horbm-memory` (`galion-knowledge` `v2/memory`). The vector leg of
//! that crate is not called. Consent bit `remember` defaults to deny, matching
//! `LOCKS.toml` `[consent]`. A denied publish leaves the lock-free cell empty
//! and does not create a database file.

#![forbid(unsafe_code)]

mod cell;
mod consent;
mod disk;
mod error;
mod fts;
mod knowledge;
mod run;
mod thread;

pub use cell::{CellSnapshot, HitCell};
pub use consent::SessionConsent;
pub use disk::DiskStatus;
pub use error::{Error, Result};
pub use fts::{FtsIndex, RetrievalHit, MAX_HITS};
pub use knowledge::{memory_db_path, resolve_knowledge_home, LoadReport, WINDOWS_KNOWLEDGE_DIR};
pub use run::{run_retrieval, FixtureRow, RunOutcome, RunRequest};
pub use thread::RetrievalThread;

/// Swarm 4 calls this. `stable_prefix` is the ASR text that has stayed put.
/// Returns at most [`MAX_HITS`] hits, best score first: card id, clause,
/// source, and score. No vectors. The call does not publish the cell and
/// does not write a file.
pub fn query(index: &FtsIndex, stable_prefix: &str) -> Result<Vec<RetrievalHit>> {
    index.query(stable_prefix)
}
