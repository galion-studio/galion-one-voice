// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! One retrieval pass: load cards, optional RAM rows, query, publish.
//! No path under this function is created.

use std::path::{Path, PathBuf};

use crate::cell::CellSnapshot;
use crate::consent::SessionConsent;
use crate::disk::{self, DiskStatus, Stamp};
use crate::error::{Error, Result};
use crate::fts::{row_from_parts, FtsIndex, RetrievalHit};
use crate::knowledge::{self, LoadReport};
use crate::thread::RetrievalThread;

/// What the gate and the laptop binary ask the thread to do.
pub struct RunRequest {
    pub knowledge_dir: PathBuf,
    pub memory_db: Option<PathBuf>,
    pub consent: SessionConsent,
    pub stable_prefix: String,
    /// Extra RAM rows. The 1,000-row gate fixture is passed here. They are
    /// not written to `knowledge_dir` or to `memory_db`.
    pub extra_rows: Vec<FixtureRow>,
}

pub struct FixtureRow {
    pub card_id: String,
    pub clause: String,
    pub source: String,
}

pub struct RunOutcome {
    pub hits: Vec<RetrievalHit>,
    pub cell: CellSnapshot,
    pub report: LoadReport,
    pub memory_db: DiskStatus,
    pub revisions: DiskStatus,
}

pub fn run_retrieval(request: RunRequest) -> Result<RunOutcome> {
    let memory_stamps = request
        .memory_db
        .as_deref()
        .map(stamp_family)
        .unwrap_or_default();
    let revisions_stamps = stamp_family(&request.knowledge_dir.join("revisions.sqlite"));
    let knowledge_artifacts = disk::artifact_rel_paths(&request.knowledge_dir)?;

    let mut index = FtsIndex::open_in_memory()?;
    let report = knowledge::load_into(&mut index, &request.knowledge_dir)?;
    if !request.extra_rows.is_empty() {
        let rows: Vec<_> = request
            .extra_rows
            .iter()
            .map(|row| row_from_parts(&row.card_id, &row.clause, &row.source))
            .collect();
        index.insert_rows(&rows)?;
    }
    let thread = RetrievalThread::spawn(index, request.consent)?;
    let hits = thread.query(&request.stable_prefix)?;
    let cell = thread.publish(&request.stable_prefix)?;
    if !request.consent.remember() && !cell.hits.is_empty() {
        return Err(Error::message(
            "remember is denied but the retrieval cell has hits",
        ));
    }
    if request.consent.remember() && cell.generation == 0 {
        return Err(Error::message(
            "remember is granted but the cell was not published",
        ));
    }

    let memory_db = check_family(&memory_stamps)?.unwrap_or(DiskStatus::Unset);
    let revisions = check_family(&revisions_stamps)?.unwrap_or(DiskStatus::NotCreated);
    disk::artifacts_unchanged(&request.knowledge_dir, &knowledge_artifacts)?;
    drop(thread);
    Ok(RunOutcome {
        hits,
        cell,
        report,
        memory_db,
        revisions,
    })
}

fn stamp_family(path: &Path) -> Vec<Stamp> {
    vec![
        Stamp::capture(path),
        Stamp::capture(&sibling(path, "-wal")),
        Stamp::capture(&sibling(path, "-shm")),
    ]
}

fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(suffix);
    path.with_file_name(name)
}

fn check_family(stamps: &[Stamp]) -> Result<Option<DiskStatus>> {
    if stamps.is_empty() {
        return Ok(None);
    }
    let main = stamps[0].check()?;
    for stamp in stamps.iter().skip(1) {
        stamp.check()?;
    }
    Ok(Some(main))
}
