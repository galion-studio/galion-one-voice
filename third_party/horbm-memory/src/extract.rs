// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Deterministic fact extraction and session summaries.
//!
//! Galion Research's Rust port left L1 extraction and L2 summaries
//! unimplemented (`openmind-memory` returns `scheduler_notified: false`).
//! This crate does not call a model. The scores below are a documented
//! heuristic, in the spirit of `l1_records.priority`, not a copy of that
//! column's write path.
//!
//! | Line | Importance |
//! |---|---:|
//! | `remember this:` | 0.92 |
//! | `my name is` | 0.90 |
//! | birthday | 0.84 |
//! | `I am a …` / `I live in` | 0.80 |
//! | `my sister` / friend / partner / … | 0.78 |
//! | `I prefer` / `I like` / `I love` / `I hate` | 0.70 |
//! | `I decided` / `I plan` / `I will` | 0.66 |
//!
//! Jokes and lines that look like credentials produce no candidate. Name,
//! home, and birthday are single-value: a new object archives the previous
//! row (the text stays). Preferences and relationships stack. If a line
//! does not match, it stays in the transcript only.

use crate::text::index_tokens;

pub const SUMMARY_IMPORTANCE: f32 = 0.85;

#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    pub text: String,
    pub importance: f32,
    pub subject_key: String,
    pub object_value: String,
    pub object_display: String,
    pub relation: String,
}

impl Candidate {
    pub fn meaning(&self, who: &str) -> String {
        let object = if self.object_display.is_empty() {
            self.text.trim().trim_end_matches('.').to_string()
        } else {
            self.object_display.clone()
        };
        if self.relation.is_empty() {
            format!("{who} asked to keep {object}")
        } else {
            let relation = &self.relation;
            format!("{who} {relation} {object}")
        }
    }
}

pub fn is_single_value_slot(subject_key: &str) -> bool {
    matches!(subject_key, "name" | "live_in" | "birthday")
}

pub trait FactExtractor {
    fn extract(&self, text: &str, said_by: &str, date: &str, source: &str) -> Vec<Candidate>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct BuiltinExtractor;

impl FactExtractor for BuiltinExtractor {
    fn extract(&self, text: &str, _said_by: &str, _date: &str, _source: &str) -> Vec<Candidate> {
        extract_line(text)
    }
}

pub fn extract_line(text: &str) -> Vec<Candidate> {
    let trimmed = text.trim();
    if trimmed.is_empty() || is_joke(trimmed) || looks_like_secret(trimmed) {
        return Vec::new();
    }
    if let Some(rest) = strip_remember(trimmed) {
        let rest = rest.trim().trim_start_matches(['-', ':']).trim();
        if rest.is_empty() || is_joke(rest) || looks_like_secret(rest) {
            return Vec::new();
        }
        let mut found = extract_patterns(rest);
        if found.is_empty() {
            found.push(Candidate {
                text: rest.to_string(),
                importance: 0.92,
                subject_key: String::new(),
                object_value: String::new(),
                object_display: String::new(),
                relation: String::new(),
            });
        } else {
            for candidate in &mut found {
                candidate.importance = candidate.importance.max(0.92);
                candidate.text = rest.to_string();
            }
        }
        return found;
    }
    extract_patterns(trimmed)
}

pub fn is_joke(text: &str) -> bool {
    let lower = text.to_lowercase();
    [
        "haha",
        "hahaha",
        "lol",
        "lmao",
        "rofl",
        "just kidding",
        "just a joke",
        "this is a joke",
        "that was a joke",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

pub fn looks_like_secret(text: &str) -> bool {
    let lower = text.to_lowercase();
    [
        "password",
        "passphrase",
        "api key",
        "api_key",
        "apikey",
        "secret key",
        "access token",
        "private key",
        "credential",
        "social security",
        "credit card",
        "card number",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn extract_patterns(text: &str) -> Vec<Candidate> {
    let mut out = Vec::new();
    if let Some(name) = capture(text, &["my name is ", "i am called ", "i'm called "]) {
        if useful_object(&name.value) {
            out.push(make(text, 0.90, "name", &name, "is named"));
        }
    }
    if let Some(birthday) = capture(text, &["my birthday is ", "i was born on "]) {
        if useful_object(&birthday.value) {
            out.push(make(text, 0.84, "birthday", &birthday, "was born on"));
        }
    }
    if let Some(home) = capture(
        text,
        &[
            "i live in ",
            "i'm based in ",
            "i am based in ",
            "my home is in ",
        ],
    ) {
        if useful_object(&home.value) {
            out.push(make(text, 0.80, "live_in", &home, "lives in"));
        }
    }
    if let Some(role) = capture(text, &["i am a ", "i am an ", "i'm a ", "i'm an "]) {
        if useful_object(&role.value) {
            out.push(make(text, 0.80, "role", &role, "is a"));
        }
    }
    if let Some(pref) = capture(
        text,
        &[
            "i prefer ",
            "i like ",
            "i love ",
            "i hate ",
            "i always ",
            "i never ",
        ],
    ) {
        if useful_object(&pref.value) {
            let relation = if text.to_lowercase().contains("i hate ") {
                "hates"
            } else if text.to_lowercase().contains("i love ") {
                "loves"
            } else if text.to_lowercase().contains("i like ") {
                "likes"
            } else if text.to_lowercase().contains("i never ") {
                "avoids"
            } else if text.to_lowercase().contains("i always ") {
                "always wants"
            } else {
                "prefers"
            };
            out.push(make(text, 0.70, "prefer", &pref, relation));
        }
    }
    if let Some(plan) = capture(text, &["i decided to ", "i plan to ", "i will "]) {
        if useful_object(&plan.value) {
            let relation = if text.to_lowercase().contains("i plan to ") {
                "plans to"
            } else if text.to_lowercase().contains("i will ") {
                "will"
            } else {
                "decided to"
            };
            out.push(make(text, 0.66, "decide", &plan, relation));
        }
    }
    if let Some(rel) = capture_relation(text) {
        out.push(rel);
    }
    out
}

struct Captured {
    value: String,
    display: String,
}

fn make(
    text: &str,
    importance: f32,
    subject: &str,
    captured: &Captured,
    relation: &str,
) -> Candidate {
    Candidate {
        text: text.trim().to_string(),
        importance,
        subject_key: subject.to_string(),
        object_value: captured.value.to_lowercase(),
        object_display: captured.display.clone(),
        relation: relation.to_string(),
    }
}

fn capture(original: &str, prefixes: &[&str]) -> Option<Captured> {
    let lower = original.to_lowercase();
    for prefix in prefixes {
        let Some(idx) = lower.find(prefix) else {
            continue;
        };
        let start_chars = lower[..idx + prefix.len()].chars().count();
        let rest: String = original.chars().skip(start_chars).collect();
        let display = clean_phrase(&rest);
        if display.is_empty() {
            continue;
        }
        return Some(Captured {
            value: display.to_lowercase(),
            display,
        });
    }
    None
}

fn capture_relation(text: &str) -> Option<Candidate> {
    if index_tokens(text).len() < 3 {
        return None;
    }
    let lower = text.to_lowercase();
    const RELS: &[&str] = &[
        "sister", "brother", "mother", "father", "parent", "friend", "partner", "wife", "husband",
        "son", "daughter",
    ];
    for rel in RELS {
        let marker = format!("my {rel}");
        if lower.contains(&marker) {
            return Some(Candidate {
                text: text.trim().to_string(),
                importance: 0.78,
                subject_key: format!("rel:{rel}"),
                object_value: rel.to_string(),
                object_display: (*rel).to_string(),
                relation: "mentioned their".to_string(),
            });
        }
    }
    None
}

fn clean_phrase(rest: &str) -> String {
    let cut = rest
        .split(|c: char| matches!(c, '.' | '?' | '!' | '\n' | ';' | ','))
        .next()
        .unwrap_or(rest);
    let mut words = Vec::new();
    for word in cut.split_whitespace().take(8) {
        let trimmed = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '-' && c != '\'');
        if !trimmed.is_empty() {
            words.push(trimmed.to_string());
        }
    }
    words.join(" ")
}

fn useful_object(value: &str) -> bool {
    value.chars().count() >= 2 && !index_tokens(value).is_empty()
}

fn strip_remember(text: &str) -> Option<&str> {
    let lower = text.to_lowercase();
    const PREFIX: &str = "remember this";
    if !lower.starts_with(PREFIX) {
        return None;
    }
    Some(&text[PREFIX.len()..])
}

#[derive(Clone, Debug)]
pub struct SummaryTurn {
    pub said_by: String,
    pub date: String,
    pub text: String,
}

/// A short reading of the call: current beliefs, the arc, and a count of
/// asides left in the transcript. It is not a copy of the lines.
pub fn summarise_session(session_id: &str, turns: &[SummaryTurn]) -> String {
    let mut facts: Vec<Candidate> = Vec::new();
    let mut asides = 0usize;
    let mut speakers: Vec<String> = Vec::new();
    let mut topics: Vec<String> = Vec::new();
    let mut first_date = String::new();
    let mut last_date = String::new();
    for turn in turns {
        if first_date.is_empty() && !turn.date.is_empty() {
            first_date.clone_from(&turn.date);
        }
        if !turn.date.is_empty() {
            last_date.clone_from(&turn.date);
        }
        if !turn.said_by.trim().is_empty() && !speakers.iter().any(|name| name == &turn.said_by) {
            speakers.push(turn.said_by.clone());
        }
        if looks_like_secret(&turn.text) || is_joke(&turn.text) {
            asides += 1;
            continue;
        }
        let extracted = extract_line(&turn.text);
        if extracted.is_empty() {
            if index_tokens(&turn.text).len() < 3 {
                asides += 1;
            }
            for token in index_tokens(&turn.text) {
                if topics.len() == 6 {
                    break;
                }
                if !topics.iter().any(|have| have == &token) {
                    topics.push(token);
                }
            }
            continue;
        }
        for candidate in extracted {
            if is_single_value_slot(&candidate.subject_key) {
                facts.retain(|fact| fact.subject_key != candidate.subject_key);
            }
            facts.push(candidate);
        }
    }
    let who = if speakers.len() == 1 {
        speakers[0].clone()
    } else if speakers.is_empty() {
        "the speaker".to_string()
    } else {
        speakers.join(" and ")
    };
    let when = if first_date.is_empty() {
        "undated".to_string()
    } else if first_date == last_date || last_date.is_empty() {
        first_date
    } else {
        format!("{first_date} to {last_date}")
    };
    let mut sentence = format!("Session {session_id} ({when}).");
    if facts.is_empty() {
        sentence.push_str(
            " Nothing here is a preference, identity, decision, or plan worth promoting.",
        );
        if !topics.is_empty() {
            sentence.push_str(&format!(" The call touched {}.", topics.join(", ")));
        }
    } else {
        let meanings: Vec<String> = facts.iter().map(|fact| fact.meaning(&who)).collect();
        sentence.push_str(" What this call means: ");
        sentence.push_str(&meanings.join(". "));
        sentence.push('.');
        if !topics.is_empty() {
            sentence.push_str(&format!(
                " The call also moved through {}.",
                topics.join(", ")
            ));
        }
    }
    if asides > 0 {
        if asides == 1 {
            sentence.push_str(" 1 aside stayed in the transcript and was not promoted.");
        } else {
            sentence.push_str(&format!(
                " {asides} asides stayed in the transcript and were not promoted."
            ));
        }
    }
    sentence
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remember_this_and_a_joke_and_a_secret() {
        let kept = extract_line("remember this: I prefer celadon tea");
        assert_eq!(kept.len(), 1);
        assert!(kept[0].importance >= 0.92);
        assert_eq!(kept[0].subject_key, "prefer");
        assert!(extract_line("haha quokkajoke was just a joke").is_empty());
        assert!(extract_line("my password is hunter2").is_empty());
        assert!(extract_line("ok").is_empty());
    }

    #[test]
    fn summary_keeps_the_later_name_and_drops_the_joke() {
        let text = summarise_session(
            "default",
            &[
                SummaryTurn {
                    said_by: "Ada".into(),
                    date: "2026-11-18".into(),
                    text: "my name is Ada Lovelace".into(),
                },
                SummaryTurn {
                    said_by: "Ada".into(),
                    date: "2026-11-18".into(),
                    text: "I prefer celadon tea".into(),
                },
                SummaryTurn {
                    said_by: "Ada".into(),
                    date: "2026-11-18".into(),
                    text: "haha quokkajoke was just a joke".into(),
                },
                SummaryTurn {
                    said_by: "Ada".into(),
                    date: "2026-11-18".into(),
                    text: "my name is Grace Hopper".into(),
                },
            ],
        );
        assert!(text.contains("Grace Hopper"), "{text}");
        assert!(!text.contains("Ada Lovelace"), "{text}");
        assert!(text.to_lowercase().contains("celadon"), "{text}");
        assert!(!text.to_lowercase().contains("quokkajoke"), "{text}");
        assert!(text.contains("aside"), "{text}");
        assert!(!text.contains("haha"), "{text}");
    }
}
