// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Read-only HORBM knowledge cards.
//!
//! `open` reads `.md` and `.json` files. It does not create the directory,
//! write a card, or open the memory database. A query that shares no word
//! with a card is [`UNKNOWN_REPLY`]. The layer 5 router is asked first, in
//! [`crate::research::route`], and a `None` from that call stays on this path.

use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::time::Instant;

use crate::chunks::contains_forbidden;
use crate::engine::{Reply, ReplyEngine};
use crate::pad::Pad;
use crate::research;

/// Spoken when no card matches. Not a guess and not a model completion.
pub const UNKNOWN_REPLY: &str = "I don't know.";

const MAX_CARD_BYTES: u64 = 64 * 1024;

#[derive(Clone, Debug)]
struct Card {
    id: String,
    text: String,
    blob: String,
}

/// Cards loaded once. The directory is not opened for writing.
#[derive(Clone, Debug, Default)]
pub struct CardBrain {
    cards: Vec<Card>,
}

impl CardBrain {
    /// Read `dir` if it is already a directory. A missing path is an empty
    /// set, not an error, and it is not created.
    pub fn open_readonly(dir: &Path) -> Self {
        let mut cards = Vec::new();
        let Ok(entries) = std::fs::read_dir(dir) else {
            return Self { cards };
        };
        let Ok(root) = dir.canonicalize() else {
            return Self { cards };
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if !meta.is_file() || meta.len() > MAX_CARD_BYTES {
                continue;
            }
            let Ok(canon) = path.canonicalize() else {
                continue;
            };
            if !canon.starts_with(&root) {
                continue;
            }
            let Some(id_raw) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let ext = path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            let Ok(text) = read_to_string(&path) else {
                continue;
            };
            let (id, body) = match ext.as_str() {
                "md" => (clean_id(id_raw), markdown_body(&text)),
                "json" => {
                    let id = json_string(&text, "id").unwrap_or_else(|| clean_id(id_raw));
                    let body = json_string(&text, "text")
                        .or_else(|| json_string(&text, "body"))
                        .unwrap_or_default();
                    (clean_id(&id), body)
                }
                _ => continue,
            };
            let body = body.trim().to_string();
            if body.is_empty() || id.is_empty() {
                continue;
            }
            let blob = format!("{id} {body}").to_ascii_lowercase();
            cards.push(Card {
                id,
                text: body,
                blob,
            });
        }
        cards.sort_by(|a, b| a.id.cmp(&b.id));
        Self { cards }
    }

    pub fn answer(&self, query: &str) -> Reply {
        let started = Instant::now();
        let tokens = tokens(query);
        let mut best: Option<(&Card, usize)> = None;
        for card in &self.cards {
            let score = tokens
                .iter()
                .filter(|t| card.blob.contains(t.as_str()))
                .count();
            if score == 0 {
                continue;
            }
            let replace = match best {
                None => true,
                Some((prev, prev_score)) => {
                    score > prev_score || (score == prev_score && card.id < prev.id)
                }
            };
            if replace {
                best = Some((card, score));
            }
        }
        let took_us = elapsed_us(started);
        match best {
            Some((card, _)) => Reply {
                text: card.text.clone(),
                mood: Pad::RESTING,
                sources: vec![format!("card:{}", card.id)],
                evidence: vec![format!("card:{}", card.id)],
                propensity: 0.0,
                violations: usize::from(contains_forbidden(&card.text)),
                lookup_us: took_us,
                took_us,
            },
            None => Reply {
                text: UNKNOWN_REPLY.to_string(),
                mood: Pad::RESTING,
                sources: vec!["horbm:none".into()],
                evidence: Vec::new(),
                propensity: 0.0,
                violations: 0,
                lookup_us: took_us,
                took_us,
            },
        }
    }
}

/// HORBM reply engine. Asks the layer 5 router, then the card bridge.
/// Memory and case paths are accepted so the `Brain::open` shape stays, and
/// they are not opened.
pub struct CardEngine {
    cards: CardBrain,
    pad: Pad,
}

impl CardEngine {
    pub fn open(memory_db: &Path, knowledge_dir: &Path, cases: &Path) -> Self {
        let _ = (memory_db, cases);
        Self {
            cards: CardBrain::open_readonly(knowledge_dir),
            pad: Pad::RESTING,
        }
    }
}

impl ReplyEngine for CardEngine {
    fn turn(&mut self, text: &str) -> Reply {
        if let Some(routed) = research::route(text) {
            self.pad = routed.mood;
            return routed;
        }
        let reply = self.cards.answer(text);
        self.pad = reply.mood;
        reply
    }

    fn tick(&mut self) -> Pad {
        self.pad
    }

    fn mood(&self) -> Pad {
        self.pad
    }
}

fn read_to_string(path: &Path) -> std::io::Result<String> {
    let mut file = File::open(path)?;
    let mut buf = String::new();
    file.read_to_string(&mut buf)?;
    Ok(buf)
}

fn markdown_body(text: &str) -> String {
    let text = text.trim_start_matches('\u{feff}');
    let mut lines = text.lines();
    let Some(first) = lines.next() else {
        return String::new();
    };
    if first.trim_start().starts_with("# ") {
        lines.collect::<Vec<_>>().join("\n")
    } else {
        text.to_string()
    }
}

fn json_string(src: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{key}\"");
    let start = src.find(&pattern)?;
    let rest = src[start + pattern.len()..].trim_start();
    let rest = rest.strip_prefix(':')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    let mut out = String::new();
    let mut chars = rest.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some(other) => out.push(other),
                None => break,
            }
        } else if c == '"' {
            break;
        } else {
            out.push(c);
        }
    }
    let out = out.trim().to_string();
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn clean_id(raw: &str) -> String {
    raw.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn tokens(query: &str) -> Vec<String> {
    query
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| w.len() >= 3 && !STOP.contains(&w.to_ascii_lowercase().as_str()))
        .map(|w| w.to_ascii_lowercase())
        .collect()
}

const STOP: &[&str] = &[
    "the", "and", "for", "are", "was", "not", "but", "with", "this", "that", "from", "have",
    "what", "when", "where", "who", "how", "why", "about", "you", "your", "does", "did",
];

fn elapsed_us(started: Instant) -> u32 {
    u32::try_from(started.elapsed().as_micros()).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_card(dir: &Path, name: &str, body: &str) {
        fs::write(dir.join(name), body).unwrap();
    }

    #[test]
    fn matching_card_is_quoted_and_a_miss_is_i_dont_know() {
        let dir = std::env::temp_dir().join(format!(
            "galion-cards-{}-{}",
            std::process::id(),
            elapsed_us(Instant::now())
        ));
        fs::create_dir_all(&dir).unwrap();
        write_card(&dir, "kettle.md", "# Kettle\nThe kettle is on the stove.\n");
        write_card(
            &dir,
            "gate.json",
            "{\"id\":\"gate\",\"text\":\"The gate sticks on the left.\"}\n",
        );
        let before = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect::<Vec<_>>();
        let brain = CardBrain::open_readonly(&dir);
        let hit = brain.answer("where is the kettle");
        assert_eq!(hit.text, "The kettle is on the stove.");
        assert_eq!(hit.sources, vec!["card:kettle".to_string()]);
        assert_eq!(hit.violations, 0);
        assert!(!hit.text.contains("don't know"));
        let other = brain.answer("what about the gate");
        assert_eq!(other.text, "The gate sticks on the left.");
        assert_eq!(other.sources, vec!["card:gate".to_string()]);
        let miss = brain.answer("who won the match");
        assert_eq!(miss.text, UNKNOWN_REPLY);
        assert_eq!(miss.sources, vec!["horbm:none".to_string()]);
        assert_eq!(miss.evidence.len(), 0);
        let after = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect::<Vec<_>>();
        assert_eq!(before.len(), after.len());
        let kettle = fs::read_to_string(dir.join("kettle.md")).unwrap();
        assert!(kettle.contains("The kettle is on the stove."));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn open_does_not_create_a_missing_directory() {
        let dir = std::env::temp_dir().join(format!(
            "galion-cards-missing-{}-{}",
            std::process::id(),
            elapsed_us(Instant::now())
        ));
        assert!(!dir.exists());
        let brain = CardBrain::open_readonly(&dir);
        let miss = brain.answer("hello there");
        assert_eq!(miss.text, UNKNOWN_REPLY);
        assert!(!dir.exists());
    }

    #[test]
    fn engine_asks_the_router_then_the_cards() {
        let dir = std::env::temp_dir().join(format!(
            "galion-engine-{}-{}",
            std::process::id(),
            elapsed_us(Instant::now())
        ));
        fs::create_dir_all(&dir).unwrap();
        write_card(&dir, "kettle.md", "The kettle is on the stove.\n");
        let memory = dir.join("memory.db");
        let cases = dir.join("cases");
        let mut engine = CardEngine::open(&memory, &dir, &cases);
        assert!(research::route("where is the kettle").is_none());
        let hit = engine.turn("where is the kettle");
        assert_eq!(hit.text, "The kettle is on the stove.");
        assert!(!memory.exists());
        assert!(!cases.exists());
        assert_eq!(
            engine.turn("unrelated question about mars").text,
            UNKNOWN_REPLY
        );
        fs::remove_dir_all(&dir).unwrap();
    }
}
