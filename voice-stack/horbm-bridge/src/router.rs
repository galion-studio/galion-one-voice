// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! HORBM layer 5, the reasoning router.
//!
//! Order, fixed by the owner on 21 Nov 2026:
//!
//! 1. HORBM cards answer first.
//! 2. When no card matches there is no gap filler. No S0, no LLM, no Ollama.
//! 3. The spoken line is [`crate::UNKNOWN_REPLY`] and the miss joins the gap queue.
//! 4. A deep question may then go to the Galion Research gateway, with
//!    [`galion_research_gateway::RESEARCH_CAP`] (3 seconds). On timeout the
//!    spoken line is still [`crate::UNKNOWN_REPLY`].
//!
//! The 7881 UI calls `horbm::Brain::turn`, which uses this same order after
//! the empathy rewrite. It does not call the gateway before the cards.

use std::time::Duration;

use galion_research_gateway::{ClientReply, ResearchClient};

use crate::chunks::{contains_forbidden, speakable_chunks};
use crate::engine::{Reply, ReplyEngine};
use crate::index::CardLoader;
use crate::pad::Pad;

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

    /// Read the knowledge directory. Does not create it. Prints the card
    /// count and load time when the directory is a pack.
    pub fn open(
        knowledge_dir: &std::path::Path,
        research: Option<Box<dyn ResearchPort>>,
    ) -> Result<Self, crate::index::CardError> {
        let loaded = crate::index::load_knowledge(knowledge_dir)?;
        eprintln!("{}", loaded.report.log_line());
        Ok(Self::new(loaded.cards, research))
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
        let cards = LookupAdapt(self.cards.as_ref());
        let routed = match self.research.as_mut() {
            Some(port) => {
                let mut ask = AskAdapt(port.as_mut());
                horbm::route_heard(heard, &cards, Some(&mut ask), speakable_line)
            }
            None => horbm::route_heard(heard, &cards, None, speakable_line),
        };
        let outcome = map_route(routed);
        if let Some(gap) = outcome.gap.clone() {
            self.gaps.push(gap);
        }
        outcome
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
    horbm::is_deep_question(text)
}

pub(crate) fn speakable_line(text: &str) -> Option<String> {
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

struct LookupAdapt<'a>(&'a dyn CardLoader);

impl horbm::CardLookup for LookupAdapt<'_> {
    fn lookup(&self, query: &str) -> Option<horbm::CardHit> {
        CardLoader::lookup(self.0, query).map(|hit| horbm::CardHit {
            id: hit.id,
            gist: hit.gist,
        })
    }
}

struct AskAdapt<'a>(&'a mut dyn ResearchPort);

impl horbm::ResearchAsk for AskAdapt<'_> {
    fn ask(&mut self, question: &str, cap: Duration) -> horbm::AskOutcome {
        match self.0.ask(question, cap) {
            ResearchAnswer::Answer { text, source } => horbm::AskOutcome::Answer { text, source },
            ResearchAnswer::Miss => horbm::AskOutcome::Miss,
            ResearchAnswer::Timeout => horbm::AskOutcome::Timeout,
            ResearchAnswer::Unreachable => horbm::AskOutcome::Unreachable,
        }
    }
}

fn map_route(out: horbm::RouteOut) -> RouteOutcome {
    RouteOutcome {
        kind: map_kind(out.kind),
        spoken: out.spoken,
        sources: out.sources,
        evidence: out.evidence,
        gap: out.gap.map(map_gap),
        lookup_us: out.lookup_us,
        took_us: out.took_us,
    }
}

fn map_kind(kind: horbm::RouteKind) -> RouteKind {
    match kind {
        horbm::RouteKind::Card => RouteKind::Card,
        horbm::RouteKind::Research => RouteKind::Research,
        horbm::RouteKind::Unknown => RouteKind::Unknown,
    }
}

fn map_gap(gap: horbm::Gap) -> GapEntry {
    GapEntry {
        heard: gap.heard,
        reason: map_reason(gap.reason),
        advice: gap.advice,
    }
}

fn map_reason(reason: horbm::GapReason) -> GapReason {
    match reason {
        horbm::GapReason::NoCard => GapReason::NoCard,
        horbm::GapReason::CardUnspeakable => GapReason::CardUnspeakable,
        horbm::GapReason::ResearchOff => GapReason::ResearchOff,
        horbm::GapReason::ResearchMiss => GapReason::ResearchMiss,
        horbm::GapReason::ResearchTimeout => GapReason::ResearchTimeout,
        horbm::GapReason::ResearchUnreachable => GapReason::ResearchUnreachable,
        horbm::GapReason::ResearchAnswered => GapReason::ResearchAnswered,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunks::SliceGrain;
    use crate::index::{LoadedCard, MemoryCards};
    use crate::UNKNOWN_REPLY;
    use galion_research_gateway::RESEARCH_CAP;
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
        assert_eq!(chunks[0].grain, SliceGrain::Sentence);
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
