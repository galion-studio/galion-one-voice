// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Phase 4 HORBM brain.
//!
//! A chat turn picks one stored clause from a [`Retriever`]. The committed
//! fixture is the in-memory stand-in. The first spoken piece is at most two
//! words when the fixture allows a cut, then the rest of that same clause.
//! A miss speaks the stored persona clause. Lookup and Action turns are
//! marked for Galion Research and are not posted.
//!
//! [`StoredKeys`] reads an `HKB1` file. Version 2 stores the string table
//! and a per-key int8 scale in the file. Version 1 still loads: there is
//! no string table, and every scale is `1`. The dot is `galion-tensor`'s,
//! so the scalar and SIMD scores stay the same bits.
//!
//! No weights, no network, no audio file, no `unsafe`.

#![forbid(unsafe_code)]

mod adapt;
mod fixture;
mod hkb;
mod pick;
mod retrieve;

pub use adapt::{KeyAdapter, RetrievalAdapter};
pub use fixture::Fixture;
pub use hkb::{
    dequant_score, HkbError, KeyPrefault, SimdLevel, StoredKeys, StoredPack, STORED_VERSION,
};
pub use pick::{classify, pick, split_clause, Answer, Heuristic, MarkKind, SidebandMark, HIT_CAP};
pub use retrieve::{MemoryRetriever, Retrieved, Retriever, StoredClause};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_turn_produces_the_fixture_clause() {
        let raw = include_str!("../fixtures/clauses.json");
        assert!(!raw.contains(".wav"));
        assert!(!raw.contains(".mp3"));
        assert!(!raw.contains("http://"));
        assert!(!raw.contains("https://"));
        let brain = Heuristic::committed();
        let out = brain.answer("where is the kettle");
        assert!(out.hit);
        assert_eq!(out.id, "kettle");
        assert_eq!(out.clause, "The kettle is on the hob.");
        assert_eq!(out.first, "The kettle");
        assert!(out.first.split_whitespace().count() <= 2);
        assert_eq!(out.rest, "is on the hob.");
        assert_eq!(format!("{} {}", out.first, out.rest), out.clause);
        match out.mark {
            Some(SidebandMark {
                kind: MarkKind::Lookup { query },
                posted: false,
            }) => assert_eq!(query, "where is the kettle"),
            other => panic!("lookup mark was not recorded: {other:?}"),
        }
    }

    #[test]
    fn a_miss_is_the_stored_persona_clause() {
        let brain = Heuristic::committed();
        assert_eq!(brain.miss_id(), "horbm-say-i-dont-know");
        assert_eq!(brain.miss_text(), "I don't know.");
        let out = brain.answer("who won the match");
        assert!(!out.hit);
        assert_eq!(out.clause, "I don't know.");
        assert_eq!(out.first, "I don't know.");
        assert!(out.rest.is_empty());
        assert!(matches!(
            out.mark,
            Some(SidebandMark {
                kind: MarkKind::Lookup { .. },
                posted: false,
            })
        ));
    }

    #[test]
    fn a_two_word_clause_stays_whole_and_an_action_is_only_marked() {
        let brain = Heuristic::committed();
        let here = brain.answer("are you here");
        assert_eq!(here.clause, "Right here.");
        assert_eq!(here.first, "Right here.");
        assert!(here.first.split_whitespace().count() <= 2);
        assert!(here.rest.is_empty());
        assert!(here.mark.is_none());

        let action = brain.answer("remind me about the kettle");
        assert!(!action.hit);
        assert_eq!(action.clause, "I don't know.");
        match action.mark {
            Some(SidebandMark {
                kind: MarkKind::Action { name },
                posted: false,
            }) => assert_eq!(name, "remind"),
            other => panic!("action mark missing: {other:?}"),
        }
    }

    #[test]
    fn a_tie_is_a_miss_and_the_adapters_pass_the_retriever_through() {
        let fixture = Fixture::parse(
            r#"{
                "persona_miss": {"id": "miss", "text": "I don't know.", "atomic": true},
                "clauses": [
                    {"id": "b", "text": "The gate sticks.", "atomic": false},
                    {"id": "a", "text": "The gate closes.", "atomic": false}
                ]
            }"#,
        )
        .unwrap();
        let brain = Heuristic::from_fixture(&fixture);
        let tied = brain.answer("the gate");
        assert!(!tied.hit);
        assert_eq!(tied.clause, "I don't know.");

        fn search(memory: &MemoryRetriever, query: &str, limit: usize) -> Vec<Retrieved> {
            memory.retrieve(query, limit)
        }
        let adapter = RetrievalAdapter {
            inner: MemoryRetriever::from_fixture(&Fixture::committed()),
            search,
        };
        let hits = adapter.retrieve("where is the kettle", HIT_CAP);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].text, "The kettle is on the hob.");

        fn nearest(memory: &MemoryRetriever, query: &str, limit: usize) -> Vec<Retrieved> {
            memory.retrieve(query, limit)
        }
        let keys = KeyAdapter {
            inner: MemoryRetriever::committed(),
            nearest,
        };
        assert_eq!(keys.retrieve("where is the kettle", 4)[0].id, "kettle");
    }

    #[test]
    fn blank_input_does_not_invent_a_clause() {
        let out = Heuristic::committed().answer("   ");
        assert!(!out.hit);
        assert!(out.clause.is_empty());
        assert!(out.mark.is_none());
    }
}
