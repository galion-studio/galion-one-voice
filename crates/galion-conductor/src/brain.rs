// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! Clause seam. Phase 4 (`horbm-brain` on PR #252) is not on `main`.
//!
//! `lead` is that crate's `Answer.first`. `rest` is `Answer.rest`.
//! `clause` is the line a later mouth would speak. This trait does not
//! open a socket and it does not start a retrieval thread. A slow brain
//! fills a cell between frames. `answer` only reads what is already there.

/// One pick. Owned so the conductor can hold it after `answer` returns.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PickedClause {
    pub id: String,
    pub clause: String,
    pub lead: String,
    pub rest: String,
    pub hit: bool,
}

/// Synchronous pick. Must not block the caller.
pub trait ClauseBrain {
    fn answer(&self, query: &str) -> PickedClause;
}

/// Fixture brain used by the simulator. Not `horbm-brain`.
///
/// A query that contains `kettle` returns the phase 4 fixture clause.
/// Any other non-empty query returns the persona miss `I don't know.`
/// An empty query returns an empty clause and does not speak the miss.
#[derive(Clone, Copy, Debug, Default)]
pub struct FakeBrain;

impl FakeBrain {
    pub const fn new() -> Self {
        Self
    }
}

impl ClauseBrain for FakeBrain {
    fn answer(&self, query: &str) -> PickedClause {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return PickedClause {
                id: String::new(),
                clause: String::new(),
                lead: String::new(),
                rest: String::new(),
                hit: false,
            };
        }
        if trimmed.to_ascii_lowercase().contains("kettle") {
            return PickedClause {
                id: "kettle".to_string(),
                clause: "The kettle is on the hob.".to_string(),
                lead: "The kettle".to_string(),
                rest: "is on the hob.".to_string(),
                hit: true,
            };
        }
        PickedClause {
            id: "horbm-say-i-dont-know".to_string(),
            clause: "I don't know.".to_string(),
            lead: "I don't know.".to_string(),
            rest: String::new(),
            hit: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_clause_and_miss() {
        let brain = FakeBrain::new();
        let hit = brain.answer("where is the kettle");
        assert_eq!(hit.clause, "The kettle is on the hob.");
        assert_eq!(hit.lead, "The kettle");
        assert_eq!(hit.rest, "is on the hob.");
        assert!(hit.hit);
        assert_eq!(hit.lead.split_whitespace().count(), 2);

        let miss = brain.answer("what is the weather");
        assert_eq!(miss.id, "horbm-say-i-dont-know");
        assert_eq!(miss.clause, "I don't know.");
        assert!(miss.rest.is_empty());
        assert!(!miss.hit);

        let blank = brain.answer("  ");
        assert!(blank.clause.is_empty());
        assert!(!blank.hit);
    }
}
