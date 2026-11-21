// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Read-only HORBM card loader.
//!
//! Cards stay on disk. This module opens them with `File::open` and `read_dir`.
//! It does not create a directory, write a card, or rebuild an index. Knowledge
//! base development is paused. Crate `horbm` calls this loader and does not
//! write it. There is no knowledge-ro server on 127.0.0.1:7900 in this
//! repository, and this loader does not bind that port.
//! Crate `horbm-memory` (`galion-knowledge` `v2/memory` at
//! `761a7f225304fc010379219e5b88c6669dd48f40`) is the index: it uses rusqlite,
//! `Stack::open` creates the knowledge directory, and a query returns at most
//! [`MAX_HITS`] covering hits. This loader does not open that crate, because
//! opening it writes `revisions.sqlite`. The cards themselves are the JSON
//! files under `v2/data/cards/horbm` (90) and `v2/data/cards/world` (1416) in
//! that revision. They are not copied into this repo.
//!
//! The match rule is `Stack::query` at that revision: every content token of
//! the query has to sit in the card. Stopwords match `v2/memory/src/text.rs`.
//! A tie between two equally specific cards is not a spoken match. The
//! knowledge crate's miss line is `I don't know`, and its gap advice string
//! when nothing covers the query is `ask_galion_research`.

use std::collections::BTreeSet;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use serde_json::Value;

/// `horbm-memory` returns at most this many hits from one query.
pub const MAX_HITS: usize = 4;

/// Gap advice string from `horbm-memory` `advice_for` when no card covers the query.
pub const GAP_ADVICE: &str = "ask_galion_research";

/// One card the router is allowed to speak.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CardMatch {
    pub id: String,
    pub gist: String,
}

/// A card kept in memory after a read. The loader does not write it back.
#[derive(Clone, Debug)]
pub struct LoadedCard {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub body: String,
    pub aliases: Vec<String>,
    haystack: BTreeSet<String>,
}

impl LoadedCard {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        summary: impl Into<String>,
        body: impl Into<String>,
        aliases: Vec<String>,
    ) -> Result<Self, CardError> {
        let id = id.into();
        if id.trim().is_empty() {
            return Err(CardError::Invalid("card id is empty".into()));
        }
        let title = title.into();
        let summary = summary.into();
        let body = body.into();
        if summary.trim().is_empty() && body.trim().is_empty() && title.trim().is_empty() {
            return Err(CardError::Invalid(format!("card {id} has no text")));
        }
        let mut card = Self {
            id,
            title,
            summary,
            body,
            aliases,
            haystack: BTreeSet::new(),
        };
        card.haystack = haystack_of(&card);
        Ok(card)
    }

    fn gist(&self) -> String {
        if !self.summary.trim().is_empty() {
            self.summary.trim().to_string()
        } else {
            self.body.trim().to_string()
        }
    }
}

/// Read-only lookup. Implementors must not write the knowledge directory.
pub trait CardLoader: Send + 'static {
    fn lookup(&self, query: &str) -> Option<CardMatch>;
}

/// Cards read from JSON files, or built in a test.
#[derive(Clone, Debug, Default)]
pub struct MemoryCards {
    cards: Vec<LoadedCard>,
}

impl MemoryCards {
    pub fn from_cards(cards: Vec<LoadedCard>) -> Self {
        Self { cards }
    }

    pub fn len(&self) -> usize {
        self.cards.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }

    /// Read `*.json` under `dir`. Missing path is an error and is not created.
    pub fn load_dir(dir: &Path) -> Result<Self, CardError> {
        if !dir.is_dir() {
            return Err(CardError::NotADirectory(dir.display().to_string()));
        }
        let mut files = Vec::new();
        collect_json(dir, &mut files)?;
        files.sort();
        let mut cards = Vec::new();
        for path in files {
            let mut file = File::open(&path).map_err(|err| CardError::Io(err.to_string()))?;
            let mut raw = String::new();
            file.read_to_string(&mut raw)
                .map_err(|err| CardError::Io(err.to_string()))?;
            let value: Value = serde_json::from_str(&raw)
                .map_err(|err| CardError::Invalid(format!("{}: {err}", path.display())))?;
            cards.extend(cards_from_value(&value, &path.display().to_string())?);
        }
        Ok(Self { cards })
    }
}

impl MemoryCards {
    fn covering_cards(&self, query: &str) -> Vec<&LoadedCard> {
        let terms = index_tokens(query);
        if terms.is_empty() {
            return Vec::new();
        }
        let mut hits: Vec<&LoadedCard> = self
            .cards
            .iter()
            .filter(|card| terms.iter().all(|term| card.haystack.contains(term)))
            .filter(|card| !card.gist().is_empty())
            .collect();
        hits.sort_by(|a, b| {
            a.haystack
                .len()
                .cmp(&b.haystack.len())
                .then_with(|| a.id.cmp(&b.id))
        });
        hits
    }

    /// At most [`MAX_HITS`] covering cards, most specific first.
    /// This does not pick a winner. A tie stays a tie.
    pub fn covering(&self, query: &str) -> Vec<CardMatch> {
        self.covering_cards(query)
            .into_iter()
            .take(MAX_HITS)
            .map(|card| CardMatch {
                id: card.id.clone(),
                gist: card.gist(),
            })
            .collect()
    }
}

impl CardLoader for MemoryCards {
    fn lookup(&self, query: &str) -> Option<CardMatch> {
        let hits = self.covering_cards(query);
        if hits.is_empty() {
            return None;
        }
        if hits.len() >= 2 && hits[0].haystack.len() == hits[1].haystack.len() {
            return None;
        }
        let card = hits[0];
        Some(CardMatch {
            id: card.id.clone(),
            gist: card.gist(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CardError {
    NotADirectory(String),
    Io(String),
    Invalid(String),
}

impl std::fmt::Display for CardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CardError::NotADirectory(path) => write!(f, "knowledge dir is not a directory: {path}"),
            CardError::Io(msg) => write!(f, "card read: {msg}"),
            CardError::Invalid(msg) => write!(f, "card: {msg}"),
        }
    }
}

impl std::error::Error for CardError {}

fn cards_from_value(value: &Value, path_hint: &str) -> Result<Vec<LoadedCard>, CardError> {
    match value {
        Value::Array(items) => {
            let mut out = Vec::new();
            for item in items {
                if let Some(card) = one_card(item, path_hint)? {
                    out.push(card);
                }
            }
            Ok(out)
        }
        Value::Object(map) => {
            if let Some(cards) = map.get("cards") {
                return cards_from_value(cards, path_hint);
            }
            Ok(one_card(value, path_hint)?.into_iter().collect())
        }
        _ => Err(CardError::Invalid(format!(
            "{path_hint}: card JSON must be an object or array"
        ))),
    }
}

fn one_card(value: &Value, path_hint: &str) -> Result<Option<LoadedCard>, CardError> {
    let Some(obj) = value.as_object() else {
        return Err(CardError::Invalid(format!(
            "{path_hint}: expected a card object"
        )));
    };
    let Some(id) = first_str(obj, &["id", "card_id"]).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    let title = first_str(obj, &["title"]).unwrap_or_else(|| id.clone());
    let summary = first_str(obj, &["summary", "gist"]).unwrap_or_default();
    let body = first_str(obj, &["body", "text"]).unwrap_or_default();
    if title.trim().is_empty() && summary.trim().is_empty() && body.trim().is_empty() {
        return Err(CardError::Invalid(format!(
            "{path_hint}: card {id} has no text"
        )));
    }
    let aliases = match obj.get("aliases") {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.trim().to_string()))
            .filter(|s| !s.is_empty())
            .collect(),
        _ => Vec::new(),
    };
    Ok(Some(LoadedCard::new(id, title, summary, body, aliases)?))
}

fn first_str(obj: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(Value::String(s)) = obj.get(*key) {
            return Some(s.trim().to_string());
        }
    }
    None
}

fn haystack_of(card: &LoadedCard) -> BTreeSet<String> {
    let mut text = format!(
        "{}\n{}\n{}\n{}",
        card.id, card.title, card.summary, card.body
    );
    for alias in &card.aliases {
        text.push('\n');
        text.push_str(alias);
    }
    index_tokens(&text).into_iter().collect()
}

fn collect_json(dir: &Path, out: &mut Vec<std::path::PathBuf>) -> Result<(), CardError> {
    let entries = std::fs::read_dir(dir).map_err(|err| CardError::Io(err.to_string()))?;
    for entry in entries {
        let path = entry.map_err(|err| CardError::Io(err.to_string()))?.path();
        if path.is_dir() {
            collect_json(&path, out)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
            out.push(path);
        }
    }
    Ok(())
}

/// Lowercase alphanumeric tokens, stopwords removed. Same shape as
/// `horbm-memory` `index_tokens`.
pub fn index_tokens(raw: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in raw.chars() {
        if ch.is_alphanumeric() {
            for c in ch.to_lowercase() {
                cur.push(c);
            }
        } else if !cur.is_empty() {
            push_token(&mut out, &mut cur);
        }
    }
    if !cur.is_empty() {
        push_token(&mut out, &mut cur);
    }
    out.into_iter().filter(|t| !is_stopword(t)).collect()
}

fn push_token(out: &mut Vec<String>, cur: &mut String) {
    if cur.len() <= 64 {
        out.push(std::mem::take(cur));
    } else {
        cur.clear();
    }
}

fn is_stopword(term: &str) -> bool {
    matches!(
        term,
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
            | "being"
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
            | "than"
            | "then"
            | "so"
            | "such"
            | "no"
            | "nor"
            | "too"
            | "very"
            | "can"
            | "will"
            | "just"
            | "about"
            | "over"
            | "after"
            | "before"
            | "between"
            | "out"
            | "up"
            | "down"
            | "off"
            | "own"
            | "same"
            | "other"
            | "which"
            | "who"
            | "whom"
            | "what"
            | "when"
            | "where"
            | "how"
            | "all"
            | "any"
            | "both"
            | "each"
            | "few"
            | "more"
            | "most"
            | "some"
            | "only"
            | "do"
            | "does"
            | "did"
            | "doing"
            | "have"
            | "has"
            | "had"
            | "having"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn kettle() -> LoadedCard {
        LoadedCard::new(
            "kettle",
            "The kettle",
            "The kettle is on the hob.",
            "The kettle is on the hob.",
            Vec::new(),
        )
        .unwrap()
    }

    #[test]
    fn a_card_answers_only_when_every_content_token_is_in_it() {
        let cards = MemoryCards::from_cards(vec![kettle()]);
        let hit = cards.lookup("the kettle").unwrap();
        assert_eq!(hit.id, "kettle");
        assert_eq!(hit.gist, "The kettle is on the hob.");
        assert!(cards.lookup("the kettle fuse").is_none());
    }

    #[test]
    fn a_tie_is_not_a_guess() {
        // Ids must not be stopwords. "a" is a stopword and "b" is not, so
        // those two haystacks are different sizes and the smaller one would win.
        let a = LoadedCard::new(
            "alpha-card",
            "Alpha",
            "Alpha marker.",
            "Alpha marker.",
            vec![],
        )
        .unwrap();
        let b =
            LoadedCard::new("beta-card", "Beta", "Beta marker.", "Beta marker.", vec![]).unwrap();
        let cards = MemoryCards::from_cards(vec![a, b]);
        assert!(cards.lookup("marker").is_none());
        assert_eq!(cards.covering("marker").len(), 2);
    }

    #[test]
    fn covering_keeps_at_most_four_and_a_unique_smallest_is_spoken() {
        assert_eq!(MAX_HITS, 4);
        assert_eq!(GAP_ADVICE, "ask_galion_research");
        let mut cards = Vec::new();
        for n in 0..5 {
            let extra = (0..n)
                .map(|i| format!("extra{i}"))
                .collect::<Vec<_>>()
                .join(" ");
            cards.push(
                LoadedCard::new(
                    format!("card-{n}"),
                    "Title",
                    format!("Summary shared {extra}."),
                    "Body shared.",
                    vec![],
                )
                .unwrap(),
            );
        }
        let loaded = MemoryCards::from_cards(cards);
        let hits = loaded.covering("shared");
        assert_eq!(hits.len(), MAX_HITS);
        assert_eq!(hits[0].id, "card-0");
        assert_eq!(loaded.lookup("shared").unwrap().id, "card-0");
    }

    #[test]
    fn load_dir_reads_json_and_does_not_create_a_missing_dir() {
        let root = std::env::temp_dir().join(format!(
            "galion-cards-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let missing = root.join("missing");
        let err = MemoryCards::load_dir(&missing).unwrap_err();
        assert!(matches!(err, CardError::NotADirectory(_)));
        assert!(!missing.exists());

        std::fs::create_dir_all(root.join("horbm")).unwrap();
        let path = root.join("horbm").join("kettle.json");
        let mut file = File::create(&path).unwrap();
        file.write_all(
            br#"{"id":"kettle","title":"The kettle","section":"horbm","summary":"The kettle is on the hob.","body":"The kettle is on the hob."}"#,
        )
        .unwrap();
        let before: Vec<_> = std::fs::read_dir(root.join("horbm"))
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        let loaded = MemoryCards::load_dir(&root).unwrap();
        let after: Vec<_> = std::fs::read_dir(root.join("horbm"))
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(before, after);
        assert_eq!(loaded.len(), 1);
        assert_eq!(
            loaded.lookup("kettle").unwrap().gist,
            "The kettle is on the hob."
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
