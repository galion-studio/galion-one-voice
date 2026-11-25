// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! HORBM layer 4. Decide the next move and keep the talk alive.
//!
//! Empathy has already stepped the mood. Retrieval, cards, and the fixture
//! have already produced a clause or an honest miss. Galion Research has
//! not been asked. This layer picks one move, then the brain splits that
//! clause. A deep miss may ask research only after that clause is returned.
//!
//! The heuristic is the policy. The offline bandit is off. It counts a move
//! only when it is enabled and [`DialogueConsent::learn`] is granted, and
//! the count does not change the move. Check-ins are off until the person
//! turns them on. A callback is stored only when remember is granted.
//! Nothing here writes a file, opens a socket, or reads the diary.

mod cues;
mod filter;
mod moves;
mod policy;
mod topic;

use galion_sam_empathy::{NameBook, PronounClass};

use crate::research_style::ResearchHit;

pub use moves::{flag_granted, DialogueConsent, Move, TurnSnap, RING_CAP};
pub use policy::{OfflineBandit, ReplyPlan};

use moves::TurnRing;
use policy::Choice;
use topic::TopicManager;

/// One turn's evidence, already gathered. `reply` is the clause that would
/// be spoken, including a picked card whose `Spoke.reply` is still empty.
pub struct DecideIn<'a> {
    pub heard: &'a str,
    pub intent: &'a str,
    pub reply: &'a str,
    pub sources: &'a [String],
    /// Style examples already retrieved. They are not answer content.
    pub research: &'a [ResearchHit],
    pub arousal: f32,
    pub valence: f32,
    pub dominance: f32,
    pub e: f32,
}

/// The move, and whether the gathered clause stands.
#[derive(Clone, Debug, PartialEq)]
pub struct Decision {
    pub chosen: Option<Move>,
    pub plan: ReplyPlan,
    pub propensity: f32,
}

impl Decision {
    pub fn pass() -> Self {
        Self {
            chosen: None,
            plan: ReplyPlan::Keep,
            propensity: 0.0,
        }
    }

    pub fn plan(&self) -> &ReplyPlan {
        &self.plan
    }
}

/// Layer 4 state. The name book is the same type empathy already uses.
/// The ring is the 32-turn tracker.
#[derive(Clone, Debug)]
pub struct Dialogue {
    consent: DialogueConsent,
    names: NameBook,
    ring: TurnRing,
    topics: TopicManager,
    bandit: OfflineBandit,
}

impl Default for Dialogue {
    fn default() -> Self {
        Self::new()
    }
}

impl Dialogue {
    pub fn new() -> Self {
        Self {
            consent: DialogueConsent::default(),
            names: NameBook::new(),
            ring: TurnRing::default(),
            topics: TopicManager::default(),
            bandit: OfflineBandit::new(),
        }
    }

    pub fn consent(&self) -> DialogueConsent {
        self.consent
    }

    pub fn set_consent(&mut self, consent: DialogueConsent) {
        let withdraw = self.consent.remember && !consent.remember;
        if !consent.remember {
            self.topics.clear_callbacks();
        }
        if withdraw {
            self.ring.clear();
        }
        self.consent = consent;
    }

    pub fn note_name(&mut self, name: &str, class: PronounClass) {
        self.names.note(name, class);
    }

    /// Store a callback. False when remember is denied, and the list is
    /// left unchanged. No file is written either way.
    pub fn note_callback(&mut self, text: &str) -> bool {
        if !self.consent.remember {
            return false;
        }
        self.topics.push_callback(text)
    }

    pub fn callbacks(&self) -> &[String] {
        self.topics.callbacks()
    }

    pub fn check_ins(&self) -> bool {
        self.consent.check_ins
    }

    pub fn set_bandit_enabled(&mut self, enabled: bool) {
        self.bandit.set_enabled(enabled);
    }

    pub fn bandit_updates(&self) -> u32 {
        self.bandit.updates()
    }

    pub fn bandit_counts(&self) -> [u32; 6] {
        self.bandit.counts()
    }

    pub fn ring_len(&self) -> usize {
        self.ring.len()
    }

    pub fn ring_get(&self, index: usize) -> Option<&TurnSnap> {
        self.ring.get(index)
    }

    pub fn decide(&mut self, input: DecideIn<'_>) -> Decision {
        if input.heard.trim().is_empty() {
            return Decision::pass();
        }
        match policy::check_in_phrase(input.heard) {
            cues::CheckInPhrase::On => self.consent.check_ins = true,
            cues::CheckInPhrase::Off => self.consent.check_ins = false,
            cues::CheckInPhrase::None => {}
        }
        let prior = self.ring.last_sourced();
        let callback = self.topics.first_callback().map(str::to_string);
        let current = self.topics.current().map(str::to_string);
        let (chosen, plan, propensity) = policy::choose(&Choice {
            heard: input.heard,
            intent: input.intent,
            reply: input.reply,
            sources: input.sources,
            research: input.research,
            current_topic: current.as_deref(),
            ring_source: prior.as_ref().map(|item| item.source.as_str()),
            ring_clause: prior.as_ref().map(|item| item.clause.as_str()),
            callback: callback.as_deref(),
            check_ins: self.consent.check_ins,
            remember: self.consent.remember,
        });
        if !matches!(chosen, Move::Stop) {
            if let Some(word) = cues::topic_word(input.heard) {
                self.topics.note(word);
            }
        }
        let clause = match &plan {
            ReplyPlan::Keep => input.reply.trim().to_string(),
            ReplyPlan::Replace { reply, .. } => reply.clone(),
        };
        let source = match &plan {
            ReplyPlan::Keep => filter::real_source(input.sources).map(str::to_string),
            ReplyPlan::Replace { sources, .. } => filter::real_source(sources).map(str::to_string),
        };
        self.ring.push(TurnSnap {
            arousal: input.arousal,
            valence: input.valence,
            dominance: input.dominance,
            e: input.e,
            name: self.names.focus_name().map(str::to_string),
            topic: self.topics.current().map(str::to_string),
            chosen,
            source,
            clause,
        });
        self.bandit.observe(chosen, self.consent.learn);
        Decision {
            chosen: Some(chosen),
            plan,
            propensity,
        }
    }
}
