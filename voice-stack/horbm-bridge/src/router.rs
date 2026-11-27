// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! HORBM layer 5, the reasoning router.
//!
//! [`ReasoningRouter::route`] and [`ReplyEngine::turn`] share one order:
//!
//! 1. HORBM cards answer first. A hit is spoken and Research is not asked.
//! 2. There is no S0, no student, no LLM, and no Ollama between the card and
//!    Research. A miss is not a guess.
//! 3. A deep card miss may ask the Galion Research gateway, with
//!    [`galion_research_gateway::RESEARCH_CAP`] (3 seconds). A speakable
//!    answer that arrives inside that cap is the spoken line.
//! 4. Otherwise the spoken line is exactly [`crate::UNKNOWN_REPLY`].
//!
//! [`ReplyEngine::turn`] is one [`horbm::Brain::turn`]. Comfort does not open
//! Research. A deep miss drops the turn value, which starts the ask, and
//! then waits only until the cap. A line that arrives later is not spoken.
//! [`ReasoningRouter::research_follow_up`] can still read that late result.

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

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

/// Layer 5.
///
/// `route` and [`ReplyEngine::turn`] both speak HORBM, then Research inside
/// 3 seconds, otherwise exactly [`crate::UNKNOWN_REPLY`]. Neither path
/// invents a line.
///
/// The card loader and the research port are each one value. `route` and
/// the brain share them. `CardLoader` is `Send` and not `Sync`, so the share
/// is a mutex. Each lookup or ask holds that mutex only for the delegated
/// call.
pub struct ReasoningRouter {
    cards: Arc<Mutex<Box<dyn CardLoader>>>,
    research: Option<Arc<Mutex<Box<dyn ResearchPort>>>>,
    gaps: Vec<GapEntry>,
    mood: Pad,
    /// Built once in [`Self::new`]. A turn does not build another.
    brain: horbm::Brain,
}

impl ReasoningRouter {
    pub fn new(cards: impl CardLoader, research: Option<Box<dyn ResearchPort>>) -> Self {
        let cards = Arc::new(Mutex::new(Box::new(cards) as Box<dyn CardLoader>));
        let research = research.map(|port| Arc::new(Mutex::new(port)));
        let brain = build_brain(&cards, &research);
        Self {
            cards,
            research,
            gaps: Vec::new(),
            mood: Pad::RESTING,
            brain,
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

    /// Gateway result the last turn did not speak.
    ///
    /// Does not block. A research line already returned by
    /// [`ReplyEngine::turn`] is consumed there. A body that arrives after the
    /// 3 second cap stays here and is not that reply. `None` means nothing
    /// is waiting.
    pub fn research_follow_up(&mut self) -> Option<horbm::ResearchFollowUp> {
        self.brain.research_follow_up()
    }

    /// Orchestrator path. HORBM cards, then a deep-question Research ask
    /// inside [`galion_research_gateway::RESEARCH_CAP`], otherwise exactly
    /// [`crate::UNKNOWN_REPLY`]. No S0 guess.
    pub fn route(&mut self, heard: &str) -> RouteOutcome {
        let cards = SharedCards {
            cards: Arc::clone(&self.cards),
        };
        let routed = match &self.research {
            Some(port) => {
                let mut ask = SharedAsk {
                    port: Arc::clone(port),
                };
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
        let gaps_before = self.brain.gaps().len();
        let out = self.brain.turn(text);
        let mut reply = reply_of(self.mood, &out);
        let wait = self.miss_asks_research(&out.reply, gaps_before);
        // Dropping the turn value is what lets a deep-miss ask start.
        // The follow hook is not public, so the whole value is dropped.
        drop(out);
        if !wait {
            return reply;
        }
        let waited = Instant::now();
        let cap = galion_research_gateway::RESEARCH_CAP;
        if let Some(follow) = self.wait_research(cap) {
            if let Some(spoken) = research_line(&follow) {
                return Reply {
                    text: spoken,
                    mood: self.mood,
                    sources: follow.sources,
                    evidence: follow.evidence,
                    propensity: 0.0,
                    violations: 0,
                    lookup_us: reply.lookup_us,
                    took_us: add_us(reply.took_us, waited.elapsed()),
                };
            }
        }
        reply.text = crate::UNKNOWN_REPLY.to_string();
        reply.sources = vec!["honest:unknown".to_string()];
        reply.evidence.clear();
        reply.took_us = add_us(reply.took_us, waited.elapsed());
        reply
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

struct SharedCards {
    cards: Arc<Mutex<Box<dyn CardLoader>>>,
}

impl horbm::CardLookup for SharedCards {
    fn lookup(&self, query: &str) -> Option<horbm::CardHit> {
        let hit = {
            let guard = self.cards.lock().expect("cards");
            CardLoader::lookup(guard.as_ref(), query)
        };
        hit.map(|hit| horbm::CardHit {
            id: hit.id,
            gist: hit.gist,
        })
    }
}

struct SharedAsk {
    port: Arc<Mutex<Box<dyn ResearchPort>>>,
}

impl horbm::ResearchAsk for SharedAsk {
    fn ask(&mut self, question: &str, cap: Duration) -> horbm::AskOutcome {
        let answer = {
            let mut guard = self.port.lock().expect("research");
            guard.ask(question, cap)
        };
        match answer {
            ResearchAnswer::Answer { text, source } => horbm::AskOutcome::Answer { text, source },
            ResearchAnswer::Miss => horbm::AskOutcome::Miss,
            ResearchAnswer::Timeout => horbm::AskOutcome::Timeout,
            ResearchAnswer::Unreachable => horbm::AskOutcome::Unreachable,
        }
    }
}

/// One brain for the life of the router. The adapters hold the same arcs
/// `route` locks. Persona configuration is whatever [`horbm::Brain::with_cards`]
/// loads. This function does not name one.
fn build_brain(
    cards: &Arc<Mutex<Box<dyn CardLoader>>>,
    research: &Option<Arc<Mutex<Box<dyn ResearchPort>>>>,
) -> horbm::Brain {
    let lookup = SharedCards {
        cards: Arc::clone(cards),
    };
    let ask = research.as_ref().map(|port| {
        Box::new(SharedAsk {
            port: Arc::clone(port),
        }) as Box<dyn horbm::ResearchAsk + Send>
    });
    let mut brain = horbm::Brain::with_cards(lookup, ask);
    brain.set_speakable(speakable_line);
    brain
}

fn reply_of(mood: Pad, out: &horbm::TurnOut) -> Reply {
    Reply {
        text: out.reply.clone(),
        mood,
        sources: out.sources.clone(),
        evidence: out.evidence.clone(),
        propensity: 0.0,
        violations: 0,
        lookup_us: out.lookup_us,
        took_us: out.took_us,
    }
}

impl ReasoningRouter {
    /// A deep card miss armed Research. Comfort, a card hit, a shallow miss,
    /// and a missing port do not.
    fn miss_asks_research(&self, reply: &str, gaps_before: usize) -> bool {
        if reply != crate::UNKNOWN_REPLY || self.research.is_none() {
            return false;
        }
        if self.brain.dialogue_lean().verb == horbm::Verb::Comfort {
            return false;
        }
        let gaps = self.brain.gaps();
        gaps.len() > gaps_before
            && gaps
                .last()
                .is_some_and(|gap| gap.reason == horbm::GapReason::ResearchOff)
    }

    /// Block until the worker speaks, or until `cap`. A late body stays on
    /// the follow-up slot and is not returned here.
    fn wait_research(&mut self, cap: Duration) -> Option<horbm::ResearchFollowUp> {
        let started = Instant::now();
        loop {
            if let Some(follow) = self.research_follow_up() {
                return Some(follow);
            }
            if started.elapsed() >= cap {
                return None;
            }
            let left = cap.saturating_sub(started.elapsed());
            thread::sleep(Duration::from_millis(5).min(left));
        }
    }
}

fn research_line(follow: &horbm::ResearchFollowUp) -> Option<String> {
    let line = follow.spoken.as_ref()?.trim();
    if line.is_empty() {
        None
    } else {
        Some(line.to_string())
    }
}

fn add_us(base: u32, extra: Duration) -> u32 {
    let extra_us = u32::try_from(extra.as_micros()).unwrap_or(u32::MAX);
    base.saturating_add(extra_us)
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

    fn assert_no_student_guess(text: &str, sources: &[String]) {
        let blob = format!("{text} {}", sources.join(" ")).to_ascii_lowercase();
        assert!(!blob.contains("s0"), "{blob}");
        assert!(!blob.contains("ollama"), "{blob}");
        assert!(!blob.contains("llm"), "{blob}");
    }

    #[test]
    fn turn_card_hit_is_the_horbm_line_and_does_not_ask() {
        let (mut router, calls, caps) = router(ResearchAnswer::Answer {
            text: "A model guess.".into(),
            source: "local".into(),
        });
        let reply = router.turn("why kettle?");
        assert_eq!(reply.text, "The kettle is on the hob.");
        assert_eq!(reply.sources, vec!["card:kettle".to_string()]);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert!(caps.lock().unwrap().is_empty());
        assert_no_student_guess(&reply.text, &reply.sources);
        assert!(!reply.text.to_ascii_lowercase().contains("guess"));
    }

    #[test]
    fn turn_speaks_research_when_a_card_miss_answers_within_the_cap() {
        let (mut router, calls, caps) = router(ResearchAnswer::Answer {
            text: "The hob is warm.".into(),
            source: "local".into(),
        });
        let heard = "why is the spare fuse warm?";
        let started = std::time::Instant::now();
        let reply = router.turn(heard);
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "a ready research answer waited out the cap: {:?}",
            started.elapsed()
        );
        assert_eq!(reply.text, "The hob is warm.");
        assert_eq!(reply.sources, vec!["research:local".to_string()]);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(caps.lock().unwrap().as_slice(), &[RESEARCH_CAP]);
        assert_eq!(RESEARCH_CAP, Duration::from_secs(3));
        assert_no_student_guess(&reply.text, &reply.sources);
        let same = router.route(heard);
        assert_eq!(same.kind, RouteKind::Research);
        assert_eq!(same.spoken, reply.text);
    }

    #[test]
    fn turn_says_i_dont_know_when_neither_card_nor_research_answers() {
        for answer in [
            ResearchAnswer::Miss,
            ResearchAnswer::Timeout,
            ResearchAnswer::Unreachable,
        ] {
            let (mut router, calls, _) = router(answer);
            let reply = router.turn("why is the spare fuse missing from the drawer?");
            assert_eq!(reply.text, "I don't know.");
            assert_eq!(reply.text, UNKNOWN_REPLY);
            assert_eq!(reply.sources, vec!["honest:unknown".to_string()]);
            assert!(reply.evidence.is_empty());
            assert_eq!(calls.load(Ordering::SeqCst), 1);
            assert_no_student_guess(&reply.text, &reply.sources);
        }

        let (mut quiet, calls, _) = router(ResearchAnswer::Answer {
            text: "Maybe forty two.".into(),
            source: "local".into(),
        });
        let shallow = quiet.turn("where is the spare fuse?");
        assert_eq!(shallow.text, "I don't know.");
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_no_student_guess(&shallow.text, &shallow.sources);

        let mut bare = ReasoningRouter::new(MemoryCards::from_cards(vec![kettle()]), None);
        let off = bare.turn("why is the spare fuse missing from the drawer?");
        assert_eq!(off.text, "I don't know.");
        assert_eq!(off.sources, vec!["honest:unknown".to_string()]);
    }

    #[test]
    fn comfort_does_not_ask_research_or_invent_a_line() {
        let (mut router, calls, _) = router(ResearchAnswer::Answer {
            text: "A model guess.".into(),
            source: "local".into(),
        });
        let started = std::time::Instant::now();
        let reply = router.turn("why am I so sad?");
        assert!(
            started.elapsed() < Duration::from_millis(500),
            "comfort waited on research: {:?}",
            started.elapsed()
        );
        assert_eq!(reply.text, "I don't know.");
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_no_student_guess(&reply.text, &reply.sources);
        for banned in ["i hear you", "that sounds", "sorry", "guess"] {
            assert!(!reply.text.to_ascii_lowercase().contains(banned));
        }
    }

    /// The port ignores the cap on purpose. The turn still stops at 3 seconds
    /// and does not speak the late body.
    #[test]
    fn a_research_line_after_the_cap_is_not_spoken() {
        struct Hold {
            calls: Arc<AtomicU32>,
            caps: Arc<std::sync::Mutex<Vec<Duration>>>,
            release: std::sync::mpsc::Receiver<()>,
        }
        impl ResearchPort for Hold {
            fn ask(&mut self, _question: &str, cap: Duration) -> ResearchAnswer {
                self.caps.lock().expect("caps").push(cap);
                self.calls.fetch_add(1, Ordering::SeqCst);
                let _ = self.release.recv();
                ResearchAnswer::Answer {
                    text: "Too late to speak.".into(),
                    source: "local".into(),
                }
            }
        }
        let calls = Arc::new(AtomicU32::new(0));
        let caps = Arc::new(std::sync::Mutex::new(Vec::new()));
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let mut router = ReasoningRouter::new(
            MemoryCards::from_cards(vec![kettle()]),
            Some(Box::new(Hold {
                calls: Arc::clone(&calls),
                caps: Arc::clone(&caps),
                release: release_rx,
            })),
        );
        let started = std::time::Instant::now();
        let reply = router.turn("why is the spare fuse missing from the drawer?");
        let elapsed = started.elapsed();
        assert_eq!(reply.text, "I don't know.");
        assert_ne!(reply.text, "Too late to speak.");
        assert_eq!(reply.sources, vec!["honest:unknown".to_string()]);
        assert!(
            elapsed >= Duration::from_millis(2_900),
            "the turn did not keep the 3 second cap: {elapsed:?}"
        );
        assert!(
            elapsed < Duration::from_secs(5),
            "the turn waited past the cap: {elapsed:?}"
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(caps.lock().unwrap().as_slice(), &[RESEARCH_CAP]);
        release_tx.send(()).expect("release");
        let late = std::time::Instant::now();
        let follow = loop {
            if let Some(follow) = router.research_follow_up() {
                break follow;
            }
            if late.elapsed() > Duration::from_secs(2) {
                panic!("the late research body did not arrive");
            }
            thread::sleep(Duration::from_millis(5));
        };
        assert_eq!(follow.spoken.as_deref(), Some("Too late to speak."));
        assert_eq!(reply.text, "I don't know.");
    }
}
