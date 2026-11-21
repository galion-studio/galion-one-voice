// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! HORBM layer 5, the reasoning router.
//!
//! Order, fixed by the owner on 21 Nov 2026:
//!
//! 1. HORBM cards answer first.
//! 2. When no card matches there is no gap filler. No S0, no LLM, no Ollama.
//! 3. The spoken line is [`UNKNOWN_REPLY`] and the miss joins the gap queue.
//! 4. A deep question may then go to the Galion Research gateway, with
//!    [`galion_research_gateway::RESEARCH_CAP`] (3 seconds). On timeout the
//!    spoken line is still [`UNKNOWN_REPLY`].
//!
//! The 7881 UI should call [`ReasoningRouter::turn`] or [`ReasoningRouter::route`].
//! It should not call the gateway before the cards.

use std::time::{Duration, Instant};

use galion_research_gateway::{ClientReply, ResearchClient, RESEARCH_CAP};

use crate::chunks::{contains_forbidden, speakable_chunks};
use crate::engine::{Reply, ReplyEngine};
use crate::index::{CardLoader, MemoryCards};
use crate::pad::Pad;

/// Spoken when the cards and, if asked, research have no answer.
pub const UNKNOWN_REPLY: &str = "I don't know.";

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
///
/// `advice` is the `horbm-memory` string for a knowledge miss. It is
/// [`crate::index::GAP_ADVICE`] (`ask_galion_research`). The queue is in memory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GapEntry {
    pub heard: String,
    pub reason: GapReason,
    pub advice: &'static str,
}

/// Gateway result the router understands. Tests implement this. The HTTP
/// client does too. Nothing in this enum is a model guess.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResearchAnswer {
    Answer { text: String, source: String },
    Miss,
    Timeout,
    Unreachable,
}

/// The research port. A stand-in must not invent a line the test did not set.
pub trait ResearchPort: Send + 'static {
    fn ask(&mut self, question: &str, cap: Duration) -> ResearchAnswer;
}

impl ResearchPort for ResearchClient {
    fn ask(&mut self, question: &str, cap: Duration) -> ResearchAnswer {
        match self.ask_within(question, cap) {
            ClientReply::Answer { text, source } => ResearchAnswer::Answer { text, source },
            ClientReply::NotFound => ResearchAnswer::Miss,
            ClientReply::Timeout => ResearchAnswer::Timeout,
            ClientReply::Unreachable => ResearchAnswer::Unreachable,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteOutcome {
    pub kind: RouteKind,
    pub spoken: String,
    pub sources: Vec<String>,
    pub evidence: Vec<String>,
    pub gap: Option<GapEntry>,
    pub lookup_us: u32,
    pub took_us: u32,
}

/// Layer 5. Cards, then maybe research, then an honest miss.
pub struct ReasoningRouter {
    cards: Box<dyn CardLoader>,
    research: Option<Box<dyn ResearchPort>>,
    gaps: Vec<GapEntry>,
    mood: Pad,
}

impl ReasoningRouter {
    pub fn new(cards: impl CardLoader, research: Option<Box<dyn ResearchPort>>) -> Self {
        Self {
            cards: Box::new(cards),
            research,
            gaps: Vec::new(),
            mood: Pad::RESTING,
        }
    }

    /// Read the knowledge directory. Does not create it.
    pub fn open(
        knowledge_dir: &std::path::Path,
        research: Option<Box<dyn ResearchPort>>,
    ) -> Result<Self, crate::index::CardError> {
        let cards = MemoryCards::load_dir(knowledge_dir)?;
        Ok(Self::new(cards, research))
    }

    /// `GALION_HORBM_KNOWLEDGE` is the card directory. Unset is an error.
    /// `GALION_RESEARCH_BIND` unset leaves research off. A set value must be
    /// `127.0.0.1` and a port. Nothing is created on disk.
    pub fn from_env() -> Result<Self, String> {
        let knowledge = std::env::var("GALION_HORBM_KNOWLEDGE")
            .map_err(|_| "GALION_HORBM_KNOWLEDGE is unset".to_string())?;
        let bind = std::env::var("GALION_RESEARCH_BIND").ok();
        Self::from_env_values(Some(knowledge.as_str()), bind.as_deref())
    }

    pub fn from_env_values(
        knowledge_dir: Option<&str>,
        research_bind: Option<&str>,
    ) -> Result<Self, String> {
        let Some(dir) = knowledge_dir.filter(|s| !s.is_empty()) else {
            return Err("GALION_HORBM_KNOWLEDGE is unset".into());
        };
        let research = match research_bind.filter(|s| !s.is_empty()) {
            None => None,
            Some(bind) => Some(Box::new(
                ResearchClient::connect(bind).map_err(|err| err.to_string())?,
            ) as Box<dyn ResearchPort>),
        };
        Self::open(std::path::Path::new(dir), research).map_err(|err| err.to_string())
    }

    pub fn gaps(&self) -> &[GapEntry] {
        &self.gaps
    }

    pub fn route(&mut self, heard: &str) -> RouteOutcome {
        let started = Instant::now();
        let trimmed = heard.trim();
        if trimmed.is_empty() {
            return RouteOutcome {
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
        let hit = self.cards.lookup(trimmed);
        let lookup_us = elapsed_us(lookup_started.elapsed());
        if let Some(hit) = hit {
            if let Some(spoken) = speakable_line(&hit.gist) {
                let source = format!("card:{}", hit.id);
                return RouteOutcome {
                    kind: RouteKind::Card,
                    spoken,
                    evidence: vec![source.clone()],
                    sources: vec![source],
                    gap: None,
                    lookup_us,
                    took_us: elapsed_us(started.elapsed()),
                };
            }
            return self.finish_miss(trimmed, GapReason::CardUnspeakable, lookup_us, started);
        }
        if !is_deep_question(trimmed) {
            return self.finish_miss(trimmed, GapReason::NoCard, lookup_us, started);
        }
        let reason = match self.research.as_mut() {
            None => GapReason::ResearchOff,
            Some(port) => match port.ask(trimmed, RESEARCH_CAP) {
                ResearchAnswer::Answer { text, source } => {
                    if galion_research_gateway::clean_source(&source).is_some() {
                        if let Some(spoken) = speakable_line(&text) {
                            let label = format!("research:{source}");
                            let gap = GapEntry {
                                heard: trimmed.to_string(),
                                reason: GapReason::ResearchAnswered,
                                advice: crate::index::GAP_ADVICE,
                            };
                            self.gaps.push(gap.clone());
                            return RouteOutcome {
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
                ResearchAnswer::Miss => GapReason::ResearchMiss,
                ResearchAnswer::Timeout => GapReason::ResearchTimeout,
                ResearchAnswer::Unreachable => GapReason::ResearchUnreachable,
            },
        };
        self.finish_miss(trimmed, reason, lookup_us, started)
    }

    fn finish_miss(
        &mut self,
        heard: &str,
        reason: GapReason,
        lookup_us: u32,
        started: Instant,
    ) -> RouteOutcome {
        let gap = GapEntry {
            heard: heard.to_string(),
            reason,
            advice: crate::index::GAP_ADVICE,
        };
        self.gaps.push(gap.clone());
        RouteOutcome {
            kind: RouteKind::Unknown,
            spoken: UNKNOWN_REPLY.to_string(),
            sources: vec!["honest:unknown".into()],
            evidence: Vec::new(),
            gap: Some(gap),
            lookup_us,
            took_us: elapsed_us(started.elapsed()),
        }
    }
}

impl ReplyEngine for ReasoningRouter {
    fn turn(&mut self, text: &str) -> Reply {
        let outcome = self.route(text);
        Reply {
            text: outcome.spoken,
            mood: self.mood,
            sources: outcome.sources,
            evidence: outcome.evidence,
            propensity: 0.0,
            violations: 0,
            lookup_us: outcome.lookup_us,
            took_us: outcome.took_us,
        }
    }

    fn tick(&mut self) -> Pad {
        self.mood
    }

    fn mood(&self) -> Pad {
        self.mood
    }
}

/// A question mark, and either eight words or the word why, how, or explain.
/// Lexical. No model.
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

fn speakable_line(text: &str) -> Option<String> {
    if contains_forbidden(text) {
        return None;
    }
    let chunks = speakable_chunks(text);
    if chunks.is_empty() {
        return None;
    }
    Some(
        chunks
            .into_iter()
            .map(|chunk| chunk.text)
            .collect::<Vec<_>>()
            .join(" "),
    )
}

fn elapsed_us(elapsed: Duration) -> u32 {
    u32::try_from(elapsed.as_micros()).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::{LoadedCard, MemoryCards};
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    fn kettle() -> LoadedCard {
        LoadedCard::new(
            "kettle",
            "The kettle",
            "The kettle is on the hob.",
            "Why the kettle is on the hob.",
            Vec::new(),
        )
        .unwrap()
    }

    struct Rec {
        calls: Arc<AtomicU32>,
        answer: ResearchAnswer,
        caps: Arc<std::sync::Mutex<Vec<Duration>>>,
    }

    impl ResearchPort for Rec {
        fn ask(&mut self, _question: &str, cap: Duration) -> ResearchAnswer {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.caps.lock().expect("caps").push(cap);
            self.answer.clone()
        }
    }

    fn router(
        answer: ResearchAnswer,
    ) -> (
        ReasoningRouter,
        Arc<AtomicU32>,
        Arc<std::sync::Mutex<Vec<Duration>>>,
    ) {
        let calls = Arc::new(AtomicU32::new(0));
        let caps = Arc::new(std::sync::Mutex::new(Vec::new()));
        let research = Rec {
            calls: Arc::clone(&calls),
            caps: Arc::clone(&caps),
            answer,
        };
        let router = ReasoningRouter::new(
            MemoryCards::from_cards(vec![kettle()]),
            Some(Box::new(research)),
        );
        (router, calls, caps)
    }

    #[test]
    fn cards_answer_before_research_even_when_the_question_is_deep() {
        let (mut router, calls, _) = router(ResearchAnswer::Answer {
            text: "A model guess.".into(),
            source: "local".into(),
        });
        let outcome = router.route("why kettle?");
        assert_eq!(outcome.kind, RouteKind::Card);
        assert_eq!(outcome.spoken, "The kettle is on the hob.");
        assert_eq!(outcome.sources, vec!["card:kettle".to_string()]);
        assert!(outcome.gap.is_none());
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert!(router.gaps().is_empty());
    }

    #[test]
    fn a_shallow_miss_does_not_guess_and_joins_the_gap_queue() {
        let (mut router, calls, _) = router(ResearchAnswer::Answer {
            text: "Maybe forty two.".into(),
            source: "local".into(),
        });
        let outcome = router.route("where is the spare fuse?");
        assert_eq!(outcome.kind, RouteKind::Unknown);
        assert_eq!(outcome.spoken, UNKNOWN_REPLY);
        assert_eq!(outcome.sources, vec!["honest:unknown".to_string()]);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(router.gaps().len(), 1);
        assert_eq!(router.gaps()[0].reason, GapReason::NoCard);
        assert_eq!(router.gaps()[0].advice, crate::index::GAP_ADVICE);
        assert!(!outcome.spoken.to_lowercase().contains("forty"));
        assert!(!outcome.sources.iter().any(|s| {
            let low = s.to_lowercase();
            low.contains("ollama") || low.contains("s0") || low.contains("llm")
        }));
    }

    #[test]
    fn a_deep_miss_asks_research_with_the_three_second_cap() {
        let (mut router, calls, caps) = router(ResearchAnswer::Timeout);
        let outcome = router.route("why is the spare fuse missing from the drawer?");
        assert!(is_deep_question(
            "why is the spare fuse missing from the drawer?"
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(caps.lock().unwrap().as_slice(), &[RESEARCH_CAP]);
        assert_eq!(outcome.kind, RouteKind::Unknown);
        assert_eq!(outcome.spoken, "I don't know.");
        assert_eq!(outcome.gap.unwrap().reason, GapReason::ResearchTimeout);
    }

    #[test]
    fn research_text_is_spoken_and_the_card_miss_stays_queued() {
        let (mut router, _, _) = router(ResearchAnswer::Answer {
            text: "The hob is warm.".into(),
            source: "local".into(),
        });
        let outcome = router.route("why is the spare fuse warm?");
        assert_eq!(outcome.kind, RouteKind::Research);
        assert_eq!(outcome.spoken, "The hob is warm.");
        assert_eq!(outcome.sources, vec!["research:local".to_string()]);
        assert_eq!(router.gaps()[0].reason, GapReason::ResearchAnswered);
    }

    #[test]
    fn an_unspeakable_card_does_not_fall_through_to_a_guess() {
        let card = LoadedCard::new(
            "kettle",
            "The kettle",
            "I hear you.",
            "Why the kettle.",
            Vec::new(),
        )
        .unwrap();
        let calls = Arc::new(AtomicU32::new(0));
        let research = Rec {
            calls: Arc::clone(&calls),
            caps: Arc::new(std::sync::Mutex::new(Vec::new())),
            answer: ResearchAnswer::Answer {
                text: "A model guess.".into(),
                source: "local".into(),
            },
        };
        let mut router = ReasoningRouter::new(
            MemoryCards::from_cards(vec![card]),
            Some(Box::new(research)),
        );
        let outcome = router.route("why kettle?");
        assert_eq!(outcome.spoken, UNKNOWN_REPLY);
        assert_eq!(outcome.gap.unwrap().reason, GapReason::CardUnspeakable);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn unknown_line_is_one_speakable_sentence() {
        let chunks = speakable_chunks(UNKNOWN_REPLY);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].text, UNKNOWN_REPLY);
        assert!(!contains_forbidden(UNKNOWN_REPLY));
    }

    #[test]
    fn unset_knowledge_dir_is_not_created() {
        let missing = std::env::temp_dir().join(format!(
            "galion-router-missing-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let err = match ReasoningRouter::from_env_values(Some(missing.to_str().unwrap()), None) {
            Ok(_) => panic!("missing knowledge dir must fail"),
            Err(err) => err,
        };
        assert!(err.contains("not a directory") || err.contains("directory"));
        assert!(!missing.exists());
        let unset = match ReasoningRouter::from_env_values(None, Some("0.0.0.0:7911")) {
            Ok(_) => panic!("unset knowledge dir must fail"),
            Err(err) => err,
        };
        assert!(unset.contains("unset"));
        let refused = match ReasoningRouter::from_env_values(
            Some(missing.to_str().unwrap()),
            Some("0.0.0.0:7911"),
        ) {
            Ok(_) => panic!("non-loopback bind must fail"),
            Err(err) => err,
        };
        assert!(refused.contains("127.0.0.1"));
        assert!(!missing.exists());
    }

    #[test]
    fn depth_is_lexical() {
        assert!(!is_deep_question("the kettle"));
        assert!(!is_deep_question("where is the spare fuse?"));
        assert!(is_deep_question("why kettle?"));
        assert!(is_deep_question("how does it sit?"));
        assert!(is_deep_question(
            "what sits on the left side of the old garden gate?"
        ));
    }
}
