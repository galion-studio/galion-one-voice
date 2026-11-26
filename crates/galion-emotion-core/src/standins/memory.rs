// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Person-memory port.
//!
//! S14 (dialogue manager, layer 4) and S15 (inner life) call this trait.
//! This file is the stand-in under `crates/galion-emotion-core/src/standins/memory.rs`.
//! It does not open a diary. The implementation that indexes FTS5 is
//! `horbm::EpisodicStore`, in a file kept apart from truth cards.
//!
//! `remember` defaults to deny. Denied means no writes, no callbacks, and no
//! extraction. [`DenyMemory`] cannot be granted: it has no diary.

/// Who the state tracker is holding, and the mood point beside those names.
#[derive(Clone, Debug, PartialEq)]
pub struct PersonState {
    pub names: Vec<TrackedName>,
    pub arousal: f32,
    pub valence: f32,
    pub dominance: f32,
    /// Feeling word from the state tracker. Empty means no named feeling.
    pub feeling: Option<String>,
}

/// One name the state tracker already accepted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrackedName {
    pub name: String,
    pub class_label: String,
}

/// What kind of person-memory row a hit is. World cards are not a kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryKind {
    Pin,
    Summary,
    Name,
    Mood,
    /// An extracted, unpinned statement.
    Fact,
}

/// One live row. `person_id` is who it is about. `at` is the caller clock.
/// `source` is `pin`, `session-summary`, or `state-tracker`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemoryItem {
    pub id: String,
    pub person_id: String,
    pub kind: MemoryKind,
    pub text: String,
    pub at: String,
    pub source: String,
    pub pinned: bool,
}

/// A forgotten row. The text stays here. Recall does not return it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tombstone {
    pub id: String,
    pub person_id: String,
    pub text: String,
    pub at: String,
    pub source: String,
    pub reason: String,
}

/// Result of a write. `Refused` is the deny path and runs no callback.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WriteOutcome {
    Stored { id: String },
    Archived { id: String },
    Refused,
    Skipped,
    Failed,
}

/// Called only after a granted write or a granted extractive summary.
/// A denied store does not call it.
pub trait MemorySink: Send {
    fn on_stored(&mut self, item: &MemoryItem);
    fn on_extracted(&mut self, person_id: &str, text: &str);
}

/// The port. Keep it small. The diary and the stand-in both implement it.
pub trait PersonMemory: Send {
    fn remember_granted(&self) -> bool;
    fn set_remember(&mut self, grant: bool);
    fn set_sink(&mut self, sink: Box<dyn MemorySink + Send>);
    fn pin(&mut self, person_id: &str, text: &str, at: &str) -> WriteOutcome;
    fn note_state(&mut self, person_id: &str, state: &PersonState, at: &str) -> WriteOutcome;
    fn note_turn(&mut self, person_id: &str, said_by: &str, text: &str, at: &str) -> WriteOutcome;
    fn end_session(&mut self, person_id: &str, at: &str) -> WriteOutcome;
    fn forget(&mut self, person_id: &str, item_id: &str) -> WriteOutcome;

    /// Archive each row [`Self::recall`] returns for this person and query.
    ///
    /// Returns the ids whose outcome was [`WriteOutcome::Archived`]. Denied
    /// memory returns an empty list and does not recall. A twin already
    /// archived comes back [`WriteOutcome::Skipped`] and is not included.
    /// The recall cap bounds the list. Rows are archived, never deleted.
    fn forget_matching(&mut self, person_id: &str, query: &str) -> Vec<String> {
        if !self.remember_granted() {
            return Vec::new();
        }
        let hits = self.recall(person_id, query);
        let mut archived = Vec::new();
        for item in hits {
            if let WriteOutcome::Archived { id } = self.forget(person_id, &item.id) {
                archived.push(id);
            }
        }
        archived
    }
    fn recall(&mut self, person_id: &str, query: &str) -> Vec<MemoryItem>;
    fn tombstone(&self, item_id: &str) -> Option<Tombstone>;
}

/// Inner-life stand-in. Consent stays deny. Nothing is stored and the sink
/// is never called, including after [`PersonMemory::set_remember`]`(true)`.
#[derive(Default)]
pub struct DenyMemory {
    sink: Option<Box<dyn MemorySink + Send>>,
}

impl DenyMemory {
    pub fn new() -> Self {
        Self::default()
    }
}

impl PersonMemory for DenyMemory {
    fn remember_granted(&self) -> bool {
        false
    }

    fn set_remember(&mut self, _grant: bool) {}

    fn set_sink(&mut self, sink: Box<dyn MemorySink + Send>) {
        self.sink = Some(sink);
    }

    fn pin(&mut self, _person_id: &str, _text: &str, _at: &str) -> WriteOutcome {
        WriteOutcome::Refused
    }

    fn note_state(&mut self, _person_id: &str, _state: &PersonState, _at: &str) -> WriteOutcome {
        WriteOutcome::Refused
    }

    fn note_turn(
        &mut self,
        _person_id: &str,
        _said_by: &str,
        _text: &str,
        _at: &str,
    ) -> WriteOutcome {
        WriteOutcome::Refused
    }

    fn end_session(&mut self, _person_id: &str, _at: &str) -> WriteOutcome {
        WriteOutcome::Refused
    }

    fn forget(&mut self, _person_id: &str, _item_id: &str) -> WriteOutcome {
        WriteOutcome::Refused
    }

    fn recall(&mut self, _person_id: &str, _query: &str) -> Vec<MemoryItem> {
        Vec::new()
    }

    fn tombstone(&self, _item_id: &str) -> Option<Tombstone> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    struct Count {
        stored: Arc<AtomicUsize>,
        extracted: Arc<AtomicUsize>,
    }

    impl MemorySink for Count {
        fn on_stored(&mut self, _item: &MemoryItem) {
            self.stored.fetch_add(1, Ordering::SeqCst);
        }

        fn on_extracted(&mut self, _person_id: &str, _text: &str) {
            self.extracted.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn counter() -> (Count, Arc<AtomicUsize>, Arc<AtomicUsize>) {
        let stored = Arc::new(AtomicUsize::new(0));
        let extracted = Arc::new(AtomicUsize::new(0));
        (
            Count {
                stored: Arc::clone(&stored),
                extracted: Arc::clone(&extracted),
            },
            stored,
            extracted,
        )
    }

    #[test]
    fn deny_stand_in_never_writes_or_calls_back() {
        let (sink, stored, extracted) = counter();
        let mut memory = DenyMemory::new();
        memory.set_sink(Box::new(sink));
        memory.set_remember(true);
        assert!(!memory.remember_granted());
        let state = PersonState {
            names: vec![TrackedName {
                name: "Nia".into(),
                class_label: "they".into(),
            }],
            arousal: 0.3,
            valence: 0.25,
            dominance: 0.5,
            feeling: None,
        };
        assert_eq!(
            memory.pin("person-alpha", "my favourite colour is blue", "t000001"),
            WriteOutcome::Refused
        );
        assert_eq!(
            memory.note_state("person-alpha", &state, "t000001"),
            WriteOutcome::Refused
        );
        assert_eq!(
            memory.note_turn("person-alpha", "person-alpha", "hello", "t000001"),
            WriteOutcome::Refused
        );
        assert_eq!(
            memory.end_session("person-alpha", "t000001"),
            WriteOutcome::Refused
        );
        assert_eq!(
            memory.forget("person-alpha", "ram-1"),
            WriteOutcome::Refused
        );
        assert!(memory.recall("person-alpha", "favourite").is_empty());
        assert!(memory.tombstone("ram-1").is_none());
        assert_eq!(stored.load(Ordering::SeqCst), 0);
        assert_eq!(extracted.load(Ordering::SeqCst), 0);
    }
}
