// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Lock-free snapshot of the latest retrieval publish.
//!
//! Writers replace an `Arc`. Readers clone that `Arc`. They do not lock the
//! FTS connection.

use std::sync::Arc;

use arc_swap::ArcSwap;

use crate::fts::RetrievalHit;

/// One published snapshot. `generation` starts at 0 and increments on every
/// publish, including a publish that stores no hits.
#[derive(Clone, Debug, PartialEq)]
pub struct CellSnapshot {
    pub hits: Vec<RetrievalHit>,
    pub generation: u64,
    pub stable_prefix: String,
}

impl CellSnapshot {
    pub fn empty() -> Self {
        Self {
            hits: Vec::new(),
            generation: 0,
            stable_prefix: String::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.hits.is_empty()
    }
}

/// Latest snapshot. Load and store do not take a mutex.
pub struct HitCell {
    slot: ArcSwap<CellSnapshot>,
}

impl HitCell {
    pub fn new() -> Self {
        Self {
            slot: ArcSwap::from_pointee(CellSnapshot::empty()),
        }
    }

    pub fn snapshot(&self) -> Arc<CellSnapshot> {
        self.slot.load_full()
    }

    pub(crate) fn publish(&self, snapshot: CellSnapshot) {
        self.slot.store(Arc::new(snapshot));
    }

    pub fn is_empty(&self) -> bool {
        self.snapshot().hits.is_empty()
    }
}

impl Default for HitCell {
    fn default() -> Self {
        Self::new()
    }
}
