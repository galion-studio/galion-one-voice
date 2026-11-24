// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Knowledge cards, aligned with `v2/data/SCHEMA.md`.
//!
//! The stored `document` is the card JSON that was read, so every field is
//! kept, including ones this struct does not index. Unknown keys stay in
//! that JSON. Aliases from the research notes are still accepted: `text`
//! for `body`, `gist` for `summary`, `speaker` for `said_by`, and
//! `epistemic` (`said` → `fact`, `inferred` → `interpretation`).

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::error::{Error, Result};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Source {
    pub title: String,
    pub url: String,
    pub author: String,
    pub date: String,
    pub accessed: String,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ContestedView {
    pub label: String,
    pub summary: String,
    pub status: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<Source>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Card {
    pub id: String,
    pub title: String,
    pub section: String,
    pub category: String,
    pub body: String,
    pub summary: String,
    pub claim_type: String,
    pub sources: Vec<Source>,
    pub evidence_strength: String,
    pub said_by: String,
    pub date: String,
    pub source: String,
    pub status: String,
    pub tags: Vec<String>,
    pub contested_views: Vec<ContestedView>,
    pub related: Vec<String>,
    pub aliases: Vec<String>,
    pub read_time_min: Option<i64>,
    pub legacy: Option<Value>,
    /// Original card JSON. This is what a rebuild stores and what a
    /// revision archives. Indexed columns are a projection of it.
    pub document: String,
    pub content_hash: String,
}

impl Card {
    /// Body plus contested views, for the FTS document. The stored body stays as written.
    pub fn lexical_body(&self) -> String {
        let mut text = self.body.clone();
        for view in &self.contested_views {
            text.push('\n');
            text.push_str(&view.label);
            text.push('\n');
            text.push_str(&view.summary);
        }
        text
    }

    /// Text the index and the embedder see, including contested views.
    pub fn index_text(&self) -> String {
        let mut text = format!("{}\n{}\n{}", self.title, self.summary, self.body);
        for view in &self.contested_views {
            text.push('\n');
            text.push_str(&view.label);
            text.push('\n');
            text.push_str(&view.summary);
        }
        text
    }

    /// Short gist returned on a hit. The summary when it exists.
    pub fn gist(&self) -> String {
        if !self.summary.is_empty() {
            self.summary.clone()
        } else if self.body.chars().count() <= 480 {
            self.body.clone()
        } else {
            let truncated: String = self.body.chars().take(480).collect();
            format!("{truncated}…")
        }
    }
}

/// Parse one JSON value into zero or more cards.
///
/// A bare object, an array, or `{"cards":[...]}` are all accepted.
/// A JSON object that is not a card (no `id`) yields an empty list so
/// catalog files can sit beside cards. A card missing its text is an error.
pub fn parse_cards(value: &Value, path_hint: &str) -> Result<Vec<Card>> {
    match value {
        Value::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                if let Some(card) = parse_one(item, path_hint)? {
                    out.push(card);
                }
            }
            Ok(out)
        }
        Value::Object(map) => {
            if let Some(cards) = map.get("cards") {
                return parse_cards(cards, path_hint);
            }
            Ok(parse_one(value, path_hint)?.into_iter().collect())
        }
        _ => Err(Error::Invalid(format!(
            "{path_hint}: card JSON must be an object or array"
        ))),
    }
}

pub fn card_from_document(document: &str) -> Result<Card> {
    let value: Value = serde_json::from_str(document)?;
    parse_one(&value, "stored")?
        .ok_or_else(|| Error::Invalid("stored document is not a card".into()))
}

fn parse_one(value: &Value, path_hint: &str) -> Result<Option<Card>> {
    let Some(obj) = value.as_object() else {
        return Err(Error::Invalid(format!(
            "{path_hint}: expected a card object"
        )));
    };
    let id = first_str(obj, &["id", "card_id"]);
    let Some(id) = id.filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    let title = first_str(obj, &["title"]).unwrap_or_else(|| id.clone());
    let body = first_str(obj, &["body", "text"]).unwrap_or_default();
    let summary = first_str(obj, &["summary", "gist"]).unwrap_or_default();
    if title.trim().is_empty() && body.trim().is_empty() && summary.trim().is_empty() {
        return Err(Error::Invalid(format!(
            "{path_hint}: card {id} has no text"
        )));
    }
    let section = first_str(obj, &["section", "scope"])
        .filter(|s| !s.is_empty())
        .or_else(|| section_from_path(path_hint))
        .ok_or_else(|| {
            Error::Invalid(format!(
                "{path_hint}: card {id} needs section world, horbm, or diary"
            ))
        })?;
    let section = canonical_section(&section, path_hint).ok_or_else(|| {
        Error::Invalid(format!(
            "{path_hint}: card {id} has section '{section}', expected world, horbm, or diary"
        ))
    })?;
    let sources = parse_sources(obj.get("sources"));
    let said_by = first_str(obj, &["said_by", "speaker", "author"])
        .filter(|s| !s.is_empty())
        .or_else(|| sources.iter().find_map(|s| nonempty(&s.author)))
        .unwrap_or_else(|| "unknown".to_string());
    let date = first_str(obj, &["as_of_date", "date", "said_at"])
        .filter(|s| !s.is_empty())
        .or_else(|| sources.iter().find_map(|s| nonempty(&s.date)))
        .unwrap_or_else(|| "unknown".to_string());
    let source = source_label(obj, &sources);
    let claim_type = map_claim(first_str(obj, &["claim_type", "epistemic"]).unwrap_or_default());
    let tags = string_list(obj.get("tags"));
    let contested_views = parse_views(obj.get("contested_views"));
    let document = serde_json::to_string(value)?;
    Ok(Some(Card {
        id,
        title,
        section,
        category: first_str(obj, &["category"]).unwrap_or_default(),
        body,
        summary,
        claim_type,
        sources,
        evidence_strength: first_str(obj, &["evidence_strength", "evidence"]).unwrap_or_default(),
        said_by,
        date,
        source,
        status: first_str(obj, &["status"]).unwrap_or_default(),
        tags,
        contested_views,
        related: string_list(obj.get("related")),
        aliases: string_list(obj.get("aliases")),
        read_time_min: obj.get("read_time_min").and_then(|v| v.as_i64()),
        legacy: obj.get("legacy").cloned().filter(|v| !v.is_null()),
        content_hash: sha256_hex(document.as_bytes()),
        document,
    }))
}

/// Sources and contested views carried on a hit, read back from the stored
/// card JSON. A memory row has neither.
pub(crate) fn provenance_from_document(document: &str) -> (Vec<Source>, Vec<ContestedView>) {
    let Ok(value) = serde_json::from_str::<Value>(document) else {
        return (Vec::new(), Vec::new());
    };
    let Some(obj) = value.as_object() else {
        return (Vec::new(), Vec::new());
    };
    (
        parse_sources(obj.get("sources")),
        parse_views(obj.get("contested_views")),
    )
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0xf) as usize] as char);
    }
    out
}

const HEX: &[u8; 16] = b"0123456789abcdef";

fn source_label(obj: &serde_json::Map<String, Value>, sources: &[Source]) -> String {
    if let Some(src) = sources.first() {
        let label = match (src.title.is_empty(), src.url.is_empty()) {
            (false, false) => format!("{} {}", src.title, src.url),
            (false, true) => src.title.clone(),
            (true, false) => src.url.clone(),
            (true, true) => String::new(),
        };
        if !label.is_empty() {
            return label;
        }
    }
    if let Some(Value::String(s)) = obj.get("source").or_else(|| obj.get("src")) {
        if !s.trim().is_empty() {
            return s.trim().to_string();
        }
    }
    "unknown".to_string()
}

fn parse_sources(value: Option<&Value>) -> Vec<Source> {
    let Some(Value::Array(items)) = value else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let obj = item.as_object()?;
            Some(Source {
                title: first_str(obj, &["title"]).unwrap_or_default(),
                url: first_str(obj, &["url", "href"]).unwrap_or_default(),
                author: first_str(obj, &["author"]).unwrap_or_default(),
                date: first_str(obj, &["date"]).unwrap_or_default(),
                accessed: first_str(obj, &["accessed"]).unwrap_or_default(),
                note: first_str(obj, &["note"]).unwrap_or_default(),
            })
        })
        .collect()
}

fn parse_views(value: Option<&Value>) -> Vec<ContestedView> {
    let Some(Value::Array(items)) = value else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let obj = item.as_object()?;
            let label = first_str(obj, &["label", "position"]).unwrap_or_default();
            let summary = first_str(obj, &["summary", "position"]).unwrap_or_default();
            if label.is_empty() && summary.is_empty() {
                return None;
            }
            let label = if label.is_empty() {
                summary.clone()
            } else {
                label
            };
            let summary = if summary.is_empty() {
                label.clone()
            } else {
                summary
            };
            Some(ContestedView {
                label,
                summary,
                status: first_str(obj, &["status"]).unwrap_or_default(),
                sources: parse_sources(obj.get("sources")),
            })
        })
        .collect()
}

fn string_list(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.trim().to_string()))
            .filter(|s| !s.is_empty())
            .collect(),
        _ => Vec::new(),
    }
}

fn map_claim(raw: String) -> String {
    match raw.as_str() {
        "said" => "fact".to_string(),
        "inferred" => "interpretation".to_string(),
        "opinion" => "opinion".to_string(),
        other => other.to_string(),
    }
}

/// A folder named world, horbm, or diary wins when the card used some other
/// word. The site files those drafts under the folder and keeps the card.
fn canonical_section(section: &str, path: &str) -> Option<String> {
    if matches!(section, "world" | "horbm" | "diary") {
        return Some(section.to_string());
    }
    section_from_path(path)
}

fn section_from_path(path: &str) -> Option<String> {
    for part in path.split(['/', '\\']) {
        if matches!(part, "world" | "horbm" | "diary") {
            return Some(part.to_string());
        }
    }
    None
}

fn first_str(obj: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(Value::String(s)) = obj.get(*key) {
            return Some(s.trim().to_string());
        }
    }
    None
}

fn nonempty(s: &str) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_s1_shape_and_fills_unknowns() {
        let raw = r#"{
            "id": "stivers-2009",
            "title": "Turn-taking gap",
            "section": "world",
            "category": "science",
            "summary": "Mean answer offset is about 208 ms.",
            "body": "Across ten languages the mean offset is +208 ms.",
            "claim_type": "fact",
            "sources": [{"title": "Stivers et al., PNAS 2009", "url": "https://doi.org/10.1073/pnas.0903616106", "author": "Tanya Stivers and colleagues", "date": "2009", "accessed": "2026-11-18", "note": "primary paper"}],
            "evidence_strength": "strong",
            "said_by": "",
            "as_of_date": "",
            "status": "draft",
            "contested_views": [],
            "related": [],
            "extra_future_field": true
        }"#;
        let value: Value = serde_json::from_str(raw).unwrap();
        let cards = parse_cards(&value, "v2/data/cards/world/stivers-2009.json").unwrap();
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].said_by, "Tanya Stivers and colleagues");
        assert_eq!(cards[0].date, "2009");
        assert!(cards[0].source.contains("pnas.0903616106"));
        assert_eq!(cards[0].sources[0].accessed, "2026-11-18");
        assert_eq!(cards[0].sources[0].note, "primary paper");
        assert!(cards[0].document.contains("extra_future_field"));
        assert_eq!(cards[0].content_hash.len(), 64);
    }

    #[test]
    fn keeps_contested_views_and_legacy() {
        let raw = r#"{
            "id": "world.free-will-disputed",
            "title": "Free will",
            "section": "world",
            "category": "philosophy",
            "summary": "The disagreement is the point.",
            "body": "Libertarian, hard-determinist, and compatibilist positions.",
            "claim_type": "contested",
            "sources": [{"title": "Free Will", "url": "https://plato.stanford.edu/entries/freewill/", "author": "Timothy O'Connor and Christopher Franklin", "date": "2002-01-07", "accessed": "2026-11-18"}],
            "evidence_strength": "moderate",
            "said_by": "Timothy O'Connor and Christopher Franklin",
            "as_of_date": "2002-01-07",
            "status": "sourced - needs review",
            "contested_views": [
                {"label": "Libertarian free will", "summary": "Some actions are not fixed by prior events.", "status": "kept"}
            ],
            "related": ["world.libet-1983"],
            "legacy": {"v1_slug": "free-will", "note": "not evidence"}
        }"#;
        let value: Value = serde_json::from_str(raw).unwrap();
        let card = parse_cards(&value, "world/world.free-will-disputed.json")
            .unwrap()
            .pop()
            .unwrap();
        assert_eq!(card.claim_type, "contested");
        assert_eq!(card.contested_views.len(), 1);
        assert_eq!(card.contested_views[0].label, "Libertarian free will");
        assert_eq!(card.related, vec!["world.libet-1983".to_string()]);
        assert!(card.legacy.unwrap().get("v1_slug").is_some());
        let again = card_from_document(&card.document).unwrap();
        assert_eq!(again.content_hash, card.content_hash);
        assert_eq!(again.contested_views, card.contested_views);
    }

    #[test]
    fn skips_a_catalog_object() {
        let value: Value = serde_json::from_str(r#"{"sections":["world"]}"#).unwrap();
        let cards = parse_cards(&value, "v2/data/catalog.json").unwrap();
        assert!(cards.is_empty());
    }
}
