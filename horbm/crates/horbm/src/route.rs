// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Cards, then a deep-question research ask, then an honest miss.
//!
//! The spoken miss is [`UNKNOWN_REPLY`] and nothing else. A gap is recorded.
//! It is not turned into a sentence.

use std::time::{Duration, Instant};

use galion_research_gateway::{clean_source, RESEARCH_CAP};

/// Spoken when the cards and, if asked, research have no answer.
pub const UNKNOWN_REPLY: &str = "I don't know.";

/// Gap advice from `horbm-memory` when no card covers the query.
pub const GAP_ADVICE: &str = "ask_galion_research";

/// What produced the spoken line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RouteKind {
    Card,
    Research,
    Unknown,
}

/// Why a card miss was queued.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GapReason {
    NoCard,
    CardUnspeakable,
    ResearchOff,
    ResearchMiss,
    ResearchTimeout,
    ResearchUnreachable,
    /// Cards missed. Research spoke. The card gap stays on the queue.
    ResearchAnswered,
}

/// One miss. The knowledge files are not modified.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gap {
    pub heard: String,
    pub reason: GapReason,
    pub advice: &'static str,
}

/// One card the router is allowed to consider.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CardHit {
    pub id: String,
    pub gist: String,
}

/// Read-only lookup. Implementors must not write the knowledge directory.
pub trait CardLookup {
    fn lookup(&self, query: &str) -> Option<CardHit>;
    /// How many clauses this lookup can return. The default is unknown.
    fn len(&self) -> usize {
        0
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Gateway result. Nothing in this enum is a model guess.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AskOutcome {
    Answer { text: String, source: String },
    Miss,
    Timeout,
    Unreachable,
}

/// The research port. A stand-in must not invent a line the test did not set.
/// `cap` is [`RESEARCH_CAP`] from the turn.
pub trait ResearchAsk {
    fn ask(&mut self, question: &str, cap: Duration) -> AskOutcome;
}

/// Turns a card or research line into the text that may be spoken.
/// `None` means the line is not spoken and the turn misses.
pub type Speakable = fn(&str) -> Option<String>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteOut {
    pub kind: RouteKind,
    pub spoken: String,
    pub sources: Vec<String>,
    pub evidence: Vec<String>,
    pub gap: Option<Gap>,
    pub lookup_us: u32,
    pub took_us: u32,
}

/// Layer 5 order. `heard` is already the lookup string.
pub fn route_heard(
    heard: &str,
    cards: &dyn CardLookup,
    mut research: Option<&mut dyn ResearchAsk>,
    speakable: Speakable,
) -> RouteOut {
    let started = Instant::now();
    let trimmed = heard.trim();
    if trimmed.is_empty() {
        return RouteOut {
            kind: RouteKind::Unknown,
            spoken: String::new(),
            sources: Vec::new(),
            evidence: Vec::new(),
            gap: None,
            lookup_us: 0,
            took_us: elapsed_us(started.elapsed()),
        };
    }
    let lookup_started = Instant::now();
    let hit = cards.lookup(trimmed);
    let lookup_us = elapsed_us(lookup_started.elapsed());
    if let Some(hit) = hit {
        if let Some(spoken) = speakable(&hit.gist) {
            let source = format!("card:{}", hit.id);
            return RouteOut {
                kind: RouteKind::Card,
                spoken,
                evidence: vec![source.clone()],
                sources: vec![source],
                gap: None,
                lookup_us,
                took_us: elapsed_us(started.elapsed()),
            };
        }
        return finish_miss(trimmed, GapReason::CardUnspeakable, lookup_us, started);
    }
    if !is_deep_question(trimmed) {
        return finish_miss(trimmed, GapReason::NoCard, lookup_us, started);
    }
    let reason = match research.as_mut() {
        None => GapReason::ResearchOff,
        Some(port) => match port.ask(trimmed, RESEARCH_CAP) {
            AskOutcome::Answer { text, source } => {
                if clean_source(&source).is_some() {
                    if let Some(spoken) = speakable(&text) {
                        let label = format!("research:{source}");
                        let gap = Gap {
                            heard: trimmed.to_string(),
                            reason: GapReason::ResearchAnswered,
                            advice: GAP_ADVICE,
                        };
                        return RouteOut {
                            kind: RouteKind::Research,
                            spoken,
                            sources: vec![label.clone()],
                            evidence: vec![label],
                            gap: Some(gap),
                            lookup_us,
                            took_us: elapsed_us(started.elapsed()),
                        };
                    }
                }
                GapReason::ResearchMiss
            }
            AskOutcome::Miss => GapReason::ResearchMiss,
            AskOutcome::Timeout => GapReason::ResearchTimeout,
            AskOutcome::Unreachable => GapReason::ResearchUnreachable,
        },
    };
    finish_miss(trimmed, reason, lookup_us, started)
}

fn finish_miss(heard: &str, reason: GapReason, lookup_us: u32, started: Instant) -> RouteOut {
    let gap = Gap {
        heard: heard.to_string(),
        reason,
        advice: GAP_ADVICE,
    };
    RouteOut {
        kind: RouteKind::Unknown,
        spoken: UNKNOWN_REPLY.to_string(),
        sources: vec!["honest:unknown".into()],
        evidence: Vec::new(),
        gap: Some(gap),
        lookup_us,
        took_us: elapsed_us(started.elapsed()),
    }
}

/// A question mark, and either eight words or the word why, how, or explain.
pub fn is_deep_question(text: &str) -> bool {
    let trimmed = text.trim();
    if !trimmed.contains('?') {
        return false;
    }
    if trimmed.split_whitespace().count() >= 8 {
        return true;
    }
    raw_words(trimmed)
        .iter()
        .any(|word| word == "why" || word == "how" || word == "explain")
}

fn raw_words(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in text.chars() {
        if ch.is_alphanumeric() {
            for c in ch.to_lowercase() {
                cur.push(c);
            }
        } else if !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn elapsed_us(elapsed: Duration) -> u32 {
    u32::try_from(elapsed.as_micros()).unwrap_or(u32::MAX)
}

/// Accept a non-empty line. The voice bridge replaces this with its own
/// speakable-chunk filter before a live turn.
pub fn accept_line(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    struct Cards {
        hit: Option<CardHit>,
    }

    impl CardLookup for Cards {
        fn lookup(&self, query: &str) -> Option<CardHit> {
            let hit = self.hit.clone()?;
            if query.to_ascii_lowercase().contains("kettle") {
                Some(hit)
            } else {
                None
            }
        }
    }

    struct Rec {
        calls: Arc<AtomicU32>,
        caps: Arc<std::sync::Mutex<Vec<Duration>>>,
        answer: AskOutcome,
    }

    impl ResearchAsk for Rec {
        fn ask(&mut self, _question: &str, cap: Duration) -> AskOutcome {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.caps.lock().expect("caps").push(cap);
            self.answer.clone()
        }
    }

    fn kettle() -> CardHit {
        CardHit {
            id: "kettle".into(),
            gist: "The kettle is on the hob.".into(),
        }
    }

    #[test]
    fn a_card_answers_before_research() {
        let calls = Arc::new(AtomicU32::new(0));
        let mut research = Rec {
            calls: Arc::clone(&calls),
            caps: Arc::new(std::sync::Mutex::new(Vec::new())),
            answer: AskOutcome::Answer {
                text: "A model guess.".into(),
                source: "local".into(),
            },
        };
        let out = route_heard(
            "why kettle?",
            &Cards {
                hit: Some(kettle()),
            },
            Some(&mut research),
            accept_line,
        );
        assert_eq!(out.kind, RouteKind::Card);
        assert_eq!(out.spoken, "The kettle is on the hob.");
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert!(out.gap.is_none());
    }

    #[test]
    fn a_shallow_miss_is_exact_and_does_not_ask() {
        let calls = Arc::new(AtomicU32::new(0));
        let mut research = Rec {
            calls: Arc::clone(&calls),
            caps: Arc::new(std::sync::Mutex::new(Vec::new())),
            answer: AskOutcome::Answer {
                text: "Maybe forty two.".into(),
                source: "local".into(),
            },
        };
        let out = route_heard(
            "where is the spare fuse?",
            &Cards { hit: None },
            Some(&mut research),
            accept_line,
        );
        assert_eq!(out.spoken, UNKNOWN_REPLY);
        assert_eq!(out.sources, vec!["honest:unknown".to_string()]);
        assert_eq!(out.gap.unwrap().reason, GapReason::NoCard);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert!(!out.spoken.to_ascii_lowercase().contains("forty"));
        assert!(!out.spoken.to_ascii_lowercase().contains("ollama"));
    }

    #[test]
    fn a_timeout_uses_the_three_second_cap_and_misses() {
        let caps = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut research = Rec {
            calls: Arc::new(AtomicU32::new(0)),
            caps: Arc::clone(&caps),
            answer: AskOutcome::Timeout,
        };
        let out = route_heard(
            "why is the spare fuse missing from the drawer?",
            &Cards { hit: None },
            Some(&mut research),
            accept_line,
        );
        assert_eq!(caps.lock().unwrap().as_slice(), &[RESEARCH_CAP]);
        assert_eq!(RESEARCH_CAP, Duration::from_secs(3));
        assert_eq!(out.spoken, "I don't know.");
        assert_eq!(out.gap.unwrap().reason, GapReason::ResearchTimeout);
        assert!(out.evidence.is_empty());
    }
}
