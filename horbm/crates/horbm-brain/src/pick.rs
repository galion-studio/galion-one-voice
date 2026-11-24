// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Heuristic pick of one stored clause.
//!
//! The spoken line is a clause that was already stored. A miss is the persona
//! clause from the fixture (`I don't know.`). Lookup and Action turns are
//! marked for the Galion Research side band. Phase 4 does not post that mark.
//! Phase 6 posts `ToHermes`. Nothing here opens a socket.

use crate::retrieve::{word_count, MemoryRetriever, Retrieved, Retriever};

/// Cap on candidates, the same handful R4 keeps in the memory cell.
pub const HIT_CAP: usize = 4;

/// Galion Research side-band intent. Same two arms as `galion_sideband::Intent`,
/// without a dependency on that crate. `posted` stays false until phase 6.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SidebandMark {
    pub kind: MarkKind,
    pub posted: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MarkKind {
    Lookup { query: String },
    Action { name: String },
}

/// What one chat turn may say.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Answer {
    pub id: String,
    pub clause: String,
    pub first: String,
    pub rest: String,
    pub hit: bool,
    pub mark: Option<SidebandMark>,
}

/// Fixture clauses plus the stored miss line.
#[derive(Clone, Debug)]
pub struct Heuristic {
    memory: MemoryRetriever,
    miss_id: String,
    miss_text: String,
}

impl Heuristic {
    pub fn committed() -> Self {
        Self::from_fixture(&crate::fixture::Fixture::committed())
    }

    pub fn from_fixture(fixture: &crate::fixture::Fixture) -> Self {
        Self {
            memory: MemoryRetriever::from_fixture(fixture),
            miss_id: fixture.miss_id.clone(),
            miss_text: fixture.miss_text.clone(),
        }
    }

    pub fn miss_text(&self) -> &str {
        &self.miss_text
    }

    pub fn miss_id(&self) -> &str {
        &self.miss_id
    }

    pub fn answer(&self, query: &str) -> Answer {
        pick(query, &self.memory, &self.miss_text, &self.miss_id)
    }
}

impl Retriever for Heuristic {
    fn retrieve(&self, query: &str, limit: usize) -> Vec<Retrieved> {
        self.memory.retrieve(query, limit)
    }
}

/// Choose one clause, or the stored miss. `miss_text` is spoken whole.
pub fn pick(query: &str, retriever: &dyn Retriever, miss_text: &str, miss_id: &str) -> Answer {
    let query = query.trim();
    let mark = classify(query).map(|kind| SidebandMark {
        kind,
        posted: false,
    });
    if query.is_empty() {
        return Answer {
            id: String::new(),
            clause: String::new(),
            first: String::new(),
            rest: String::new(),
            hit: false,
            mark: None,
        };
    }
    let mut hits = retriever.retrieve(query, HIT_CAP);
    hits.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| word_count(&left.text).cmp(&word_count(&right.text)))
            .then_with(|| left.id.cmp(&right.id))
    });
    let tied = hits.len() >= 2
        && hits[0].score == hits[1].score
        && word_count(&hits[0].text) == word_count(&hits[1].text);
    let chosen = if hits.is_empty() || tied {
        None
    } else {
        Some(hits.remove(0))
    };
    match chosen {
        Some(hit) => {
            let (first, rest) = split_clause(&hit.text, hit.atomic || hit.text == miss_text);
            Answer {
                id: hit.id,
                clause: hit.text,
                first,
                rest,
                hit: true,
                mark,
            }
        }
        None => {
            let clause = miss_text.to_string();
            let (first, rest) = split_clause(&clause, true);
            Answer {
                id: miss_id.to_string(),
                clause,
                first,
                rest,
                hit: false,
                mark,
            }
        }
    }
}

/// First spoken piece, then the remainder.
///
/// A clause the fixture marks atomic, including the persona miss, stays one
/// piece. Any other clause yields at most two words first, then the rest.
pub fn split_clause(text: &str, atomic: bool) -> (String, String) {
    let text = text.trim();
    if text.is_empty() {
        return (String::new(), String::new());
    }
    if atomic || word_count(text) <= 2 {
        return (text.to_string(), String::new());
    }
    let mut index = 0usize;
    let mut rest_at = None;
    for (seen, word) in text.split_whitespace().enumerate() {
        let pos = text[index..]
            .find(word)
            .map(|off| index + off)
            .unwrap_or(index);
        if seen == 2 {
            rest_at = Some(pos);
            break;
        }
        index = pos + word.len();
    }
    match rest_at {
        Some(at) => (text[..at].trim_end().to_string(), text[at..].to_string()),
        None => (text.to_string(), String::new()),
    }
}

/// Lookup or Action for the side band. Chat is `None`. No model.
pub fn classify(query: &str) -> Option<MarkKind> {
    let query = query.trim();
    if query.is_empty() {
        return None;
    }
    let first = first_word(query);
    if is_action(&first) {
        return Some(MarkKind::Action { name: first });
    }
    if query.contains('?') || is_interrogative(&first) {
        return Some(MarkKind::Lookup {
            query: query.to_string(),
        });
    }
    None
}

fn first_word(text: &str) -> String {
    let mut cur = String::new();
    for ch in text.chars() {
        if ch.is_alphanumeric() {
            for lower in ch.to_lowercase() {
                cur.push(lower);
            }
        } else if !cur.is_empty() {
            break;
        }
    }
    cur
}

fn is_interrogative(word: &str) -> bool {
    matches!(
        word,
        "who" | "what" | "where" | "when" | "why" | "how" | "which"
    )
}

fn is_action(word: &str) -> bool {
    matches!(
        word,
        "remind"
            | "schedule"
            | "send"
            | "set"
            | "start"
            | "stop"
            | "text"
            | "call"
            | "open"
            | "play"
            | "book"
            | "cancel"
    )
}
