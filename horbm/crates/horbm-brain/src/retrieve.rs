// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Retrieval seam. Swarm 2 (FTS5) and Swarm 3 (int8 keys) implement this.
//! The in-memory retriever is the phase 4 stand-in over the committed fixture.

use crate::fixture::Fixture;

/// One stored clause a retriever may return. The brain speaks `text` unchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Retrieved {
    pub id: String,
    pub text: String,
    /// `true` when the fixture says this clause is not cut into a two-word lead.
    pub atomic: bool,
    /// Higher is a better cover. Equal scores are a tie, not a guess.
    pub score: i32,
}

/// Read-only candidate list. Implementors must not open a socket or a file
/// on the call. The caller ranks. A limit of 4 matches the R4 cell cap.
pub trait Retriever {
    fn retrieve(&self, query: &str, limit: usize) -> Vec<Retrieved>;
}

/// One row of the committed fixture, before scoring.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredClause {
    pub id: String,
    pub text: String,
    pub atomic: bool,
}

/// In-memory clauses. A hit must cover every content token of the query.
#[derive(Clone, Debug, Default)]
pub struct MemoryRetriever {
    clauses: Vec<StoredClause>,
    miss_id: String,
}

impl MemoryRetriever {
    pub fn from_fixture(fixture: &Fixture) -> Self {
        Self {
            clauses: fixture.clauses.clone(),
            miss_id: fixture.miss_id.clone(),
        }
    }

    pub fn committed() -> Self {
        Self::from_fixture(&Fixture::committed())
    }

    pub fn clauses(&self) -> &[StoredClause] {
        &self.clauses
    }
}

impl Retriever for MemoryRetriever {
    fn retrieve(&self, query: &str, limit: usize) -> Vec<Retrieved> {
        if limit == 0 {
            return Vec::new();
        }
        let wanted = content_tokens(query);
        if wanted.is_empty() {
            return Vec::new();
        }
        let mut hits = Vec::new();
        for clause in &self.clauses {
            if clause.id == self.miss_id {
                continue;
            }
            let have = content_tokens(&clause.text);
            let score = wanted
                .iter()
                .filter(|token| have.iter().any(|h| h == *token))
                .count();
            if score == wanted.len() {
                hits.push(Retrieved {
                    id: clause.id.clone(),
                    text: clause.text.clone(),
                    atomic: clause.atomic,
                    score: i32::try_from(score).unwrap_or(i32::MAX),
                });
            }
        }
        hits.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then_with(|| word_count(&left.text).cmp(&word_count(&right.text)))
                .then_with(|| left.id.cmp(&right.id))
        });
        hits.truncate(limit);
        hits
    }
}

pub fn word_count(text: &str) -> usize {
    text.split_whitespace().count()
}

pub fn content_tokens(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in text.chars() {
        if ch.is_alphanumeric() || ch == '\'' {
            for lower in ch.to_lowercase() {
                cur.push(lower);
            }
        } else if !cur.is_empty() {
            push_token(&mut out, &mut cur);
        }
    }
    if !cur.is_empty() {
        push_token(&mut out, &mut cur);
    }
    out
}

fn push_token(out: &mut Vec<String>, cur: &mut String) {
    let token = std::mem::take(cur);
    let bare = token.trim_matches('\'').to_string();
    if bare.len() >= 2 && !is_stopword(&bare) && !out.iter().any(|have| have == &bare) {
        out.push(bare);
    }
}

fn is_stopword(token: &str) -> bool {
    matches!(
        token,
        "a" | "an"
            | "the"
            | "and"
            | "or"
            | "of"
            | "to"
            | "in"
            | "on"
            | "for"
            | "with"
            | "at"
            | "by"
            | "from"
            | "as"
            | "is"
            | "are"
            | "was"
            | "were"
            | "be"
            | "been"
            | "am"
            | "it"
            | "its"
            | "this"
            | "that"
            | "these"
            | "those"
            | "not"
            | "but"
            | "if"
            | "into"
            | "you"
            | "your"
            | "i"
            | "me"
            | "my"
            | "do"
            | "does"
            | "did"
            | "can"
            | "please"
            | "about"
            | "what"
            | "when"
            | "where"
            | "who"
            | "whom"
            | "why"
            | "how"
            | "which"
    )
}
