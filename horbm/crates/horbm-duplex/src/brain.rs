// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! `horbm::Brain` behind the conductor's clause seam.
//!
//! The conductor calls `answer` on the eager frame and only reads.
//! The loop writes the cell while the person is still speaking.
//! Both sides share one brain. The clock does not wait.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use galion_conductor::{ClauseBrain, PickedClause};
use horbm::{Brain, CardLookup, Pad, UNKNOWN_REPLY};

struct Stored {
    query: String,
    id: String,
    clause: String,
    lead: String,
    rest: String,
    hit: bool,
    mood: Pad,
    energy: f32,
    pitch_st: f32,
    pause_ms: f32,
}

struct Inner {
    brain: RefCell<Brain>,
    cell: RefCell<Option<Stored>>,
    writes: Cell<u32>,
    reads: Cell<u32>,
}

/// Shared with the conductor. One thread. The cell is the only hand-off.
#[derive(Clone)]
pub(crate) struct BrainHandle {
    inner: Rc<Inner>,
}

pub(crate) struct MouthLine {
    pub matched: bool,
    pub mood: Pad,
    pub energy: f32,
    pub pitch_st: f32,
    pub pause_ms: f32,
}

impl BrainHandle {
    pub(crate) fn with_cards(cards: impl CardLookup + Send + 'static) -> Self {
        Self {
            inner: Rc::new(Inner {
                brain: RefCell::new(Brain::with_cards(cards, None)),
                cell: RefCell::new(None),
                writes: Cell::new(0),
                reads: Cell::new(0),
            }),
        }
    }

    pub(crate) fn reads(&self) -> u32 {
        self.inner.reads.get()
    }

    pub(crate) fn writes(&self) -> u32 {
        self.inner.writes.get()
    }

    /// Store the clause for `query` if the cell does not already hold it.
    pub(crate) fn prefetch(&self, query: &str) {
        self.fill(query);
    }

    pub(crate) fn mouth_line(&self, clause: &str) -> MouthLine {
        let cell = self.inner.cell.borrow();
        if let Some(stored) = cell.as_ref() {
            if stored.clause == clause {
                return MouthLine {
                    matched: true,
                    mood: stored.mood,
                    energy: stored.energy,
                    pitch_st: stored.pitch_st,
                    pause_ms: stored.pause_ms,
                };
            }
        }
        MouthLine {
            matched: false,
            mood: Pad::RESTING,
            energy: 1.0,
            pitch_st: 0.0,
            pause_ms: 0.0,
        }
    }

    fn fill(&self, query: &str) {
        let query = query.trim();
        if query.is_empty() {
            return;
        }
        let same = self
            .inner
            .cell
            .borrow()
            .as_ref()
            .is_some_and(|stored| stored.query == query);
        if same {
            return;
        }
        let turn = self.inner.brain.borrow_mut().turn(query);
        let (id, hit) = card_id(&turn.sources);
        let stored = Stored {
            query: query.to_string(),
            id,
            clause: turn.reply,
            lead: turn.first_clause,
            rest: turn.rest,
            hit,
            mood: turn.mood,
            energy: turn.mood_meta.energy,
            pitch_st: turn.mood_meta.pitch_st,
            pause_ms: turn.mood_meta.pause_ms,
        };
        *self.inner.cell.borrow_mut() = Some(stored);
        self.inner
            .writes
            .set(self.inner.writes.get().saturating_add(1));
    }
}

impl ClauseBrain for BrainHandle {
    fn answer(&self, query: &str) -> PickedClause {
        let query = query.trim();
        self.fill(query);
        self.inner
            .reads
            .set(self.inner.reads.get().saturating_add(1));
        let cell = self.inner.cell.borrow();
        match cell.as_ref() {
            Some(stored) if stored.query == query => PickedClause {
                id: stored.id.clone(),
                clause: stored.clause.clone(),
                lead: stored.lead.clone(),
                rest: stored.rest.clone(),
                hit: stored.hit,
            },
            _ => PickedClause {
                id: String::new(),
                clause: String::new(),
                lead: String::new(),
                rest: String::new(),
                hit: false,
            },
        }
    }
}

fn card_id(sources: &[String]) -> (String, bool) {
    for source in sources {
        if let Some(id) = source.strip_prefix("card:") {
            if !id.is_empty() {
                return (id.to_string(), true);
            }
        }
    }
    (UNKNOWN_REPLY.to_string(), false)
}
