// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Std thread. The audio clock is not this thread. It publishes an `Arc`
//! snapshot into [`crate::HitCell`] and does not lock that cell.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::cell::{CellSnapshot, HitCell};
use crate::consent::SessionConsent;
use crate::error::{Error, Result};
use crate::fts::{FtsIndex, RetrievalHit};

enum Job {
    Publish {
        prefix: String,
        ack: Sender<Result<CellSnapshot>>,
    },
    Shutdown,
}

pub struct RetrievalThread {
    tx: Sender<Job>,
    join: Option<JoinHandle<()>>,
    index: Arc<Mutex<FtsIndex>>,
    cell: Arc<HitCell>,
    remember: Arc<AtomicBool>,
}

impl RetrievalThread {
    pub(crate) fn spawn(index: FtsIndex, consent: SessionConsent) -> Result<Self> {
        index.freeze()?;
        let index = Arc::new(Mutex::new(index));
        let cell = Arc::new(HitCell::new());
        let remember = Arc::new(AtomicBool::new(consent.remember()));
        let (tx, rx) = mpsc::channel();
        let index_thread = Arc::clone(&index);
        let cell_thread = Arc::clone(&cell);
        let remember_thread = Arc::clone(&remember);
        let join = thread::Builder::new()
            .name("horbm-retrieval".to_string())
            .spawn(move || worker(rx, index_thread, cell_thread, remember_thread))
            .map_err(|err| Error::message(format!("retrieval thread: {err}")))?;
        Ok(Self {
            tx,
            join: Some(join),
            index,
            cell,
            remember,
        })
    }

    pub fn cell(&self) -> Arc<HitCell> {
        Arc::clone(&self.cell)
    }

    /// In-process query. Does not publish and does not read consent.
    pub fn query(&self, stable_prefix: &str) -> Result<Vec<RetrievalHit>> {
        let index = self
            .index
            .lock()
            .map_err(|_| Error::message("retrieval index lock poisoned"))?;
        index.query(stable_prefix)
    }

    /// Publish the latest prefix. When `remember` is denied the cell is
    /// replaced with an empty snapshot. The index is not written.
    pub fn publish(&self, stable_prefix: &str) -> Result<CellSnapshot> {
        let (ack_tx, ack_rx) = mpsc::channel();
        self.tx
            .send(Job::Publish {
                prefix: stable_prefix.to_string(),
                ack: ack_tx,
            })
            .map_err(|_| Error::message("retrieval thread stopped"))?;
        // Hang guard only. The phase 2 gate is untimed.
        match ack_rx.recv_timeout(Duration::from_secs(30)) {
            Ok(result) => result,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                Err(Error::message("retrieval publish did not return"))
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                Err(Error::message("retrieval thread stopped"))
            }
        }
    }

    /// Withdrawal clears the cell on the next publish. RAM hits are not kept.
    pub fn withdraw(&self) -> Result<CellSnapshot> {
        self.remember.store(false, Ordering::SeqCst);
        self.publish("")
    }
}

impl Drop for RetrievalThread {
    fn drop(&mut self) {
        let _ = self.tx.send(Job::Shutdown);
        if let Some(handle) = self.join.take() {
            let _ = handle.join();
        }
    }
}

fn worker(
    rx: Receiver<Job>,
    index: Arc<Mutex<FtsIndex>>,
    cell: Arc<HitCell>,
    remember: Arc<AtomicBool>,
) {
    while let Ok(job) = rx.recv() {
        match job {
            Job::Shutdown => break,
            Job::Publish { prefix, ack } => {
                let result = publish_one(&index, &cell, &remember, &prefix);
                let _ = ack.send(result);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consent::SessionConsent;
    use crate::fts::row_from_parts;

    #[test]
    fn withdraw_clears_a_granted_cell() {
        let mut index = FtsIndex::open_in_memory().unwrap();
        index
            .insert_rows(&[row_from_parts("a", "token withdrawme", "s")])
            .unwrap();
        let thread = RetrievalThread::spawn(index, SessionConsent::granted_remember()).unwrap();
        let cell = thread.publish("withdrawme").unwrap();
        assert!(!cell.hits.is_empty());
        let cleared = thread.withdraw().unwrap();
        assert!(cleared.hits.is_empty());
        assert!(cleared.generation > cell.generation);
    }
}

fn publish_one(
    index: &Mutex<FtsIndex>,
    cell: &HitCell,
    remember: &AtomicBool,
    prefix: &str,
) -> Result<CellSnapshot> {
    let generation = cell.snapshot().generation.saturating_add(1);
    let hits = if remember.load(Ordering::SeqCst) {
        let guard = index
            .lock()
            .map_err(|_| Error::message("retrieval index lock poisoned"))?;
        guard.query(prefix)?
    } else {
        Vec::new()
    };
    let snapshot = CellSnapshot {
        hits,
        generation,
        stable_prefix: prefix.to_string(),
    };
    cell.publish(snapshot.clone());
    Ok(snapshot)
}
