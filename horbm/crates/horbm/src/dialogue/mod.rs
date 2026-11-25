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
//! the count does not change the move. [`Dialogue::decide_held`] matches
//! [`Dialogue::decide`] when the hold flag is false. When the flag is true,
//! a selected topic offer becomes ask and the offer line is not built.
//! Check-ins are off until the person turns them on. A callback is stored
//! only when remember is granted. Nothing here writes a file, opens a
//! socket, or reads the diary.

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
        self.decide_held(input, false)
    }

    /// [`Self::decide`] when `hold_topic` is false. When it is true, a
    /// selected [`Move::OfferTopic`] becomes [`Move::Ask`] and the offer
    /// line is not built. The bandit still only counts the move that is
    /// returned. A research hit still cannot be an answer or a citation.
    pub fn decide_held(&mut self, input: DecideIn<'_>, hold_topic: bool) -> Decision {
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
        let choice = Choice {
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
        };
        // `hold_topic` false stays on `choose`, the same call `decide` used.
        let (chosen, plan, propensity) = if hold_topic {
            policy::choose_held(&choice, true)
        } else {
            policy::choose(&choice)
        };
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::research_style::ResearchHit;
    use crate::UNKNOWN_REPLY;

    struct Step {
        heard: &'static str,
        intent: &'static str,
        reply: &'static str,
        sources: &'static [&'static str],
    }

    fn input<'a>(
        heard: &'a str,
        intent: &'a str,
        reply: &'a str,
        sources: &'a [String],
        research: &'a [ResearchHit],
    ) -> DecideIn<'a> {
        DecideIn {
            heard,
            intent,
            reply,
            sources,
            research,
            arousal: 0.3,
            valence: 0.25,
            dominance: 0.5,
            e: 0.0,
        }
    }

    fn step_input<'a>(step: &Step, sources: &'a [String]) -> DecideIn<'a> {
        input(step.heard, step.intent, step.reply, sources, &[])
    }

    fn fresh(learn: bool) -> Dialogue {
        let mut dialogue = Dialogue::new();
        dialogue.set_bandit_enabled(true);
        dialogue.set_consent(DialogueConsent {
            remember: false,
            learn,
            check_ins: false,
        });
        dialogue
    }

    fn play(hold: Option<bool>, learn: bool) -> (Vec<Decision>, Dialogue) {
        let steps = [
            Step {
                heard: "where is the kettle",
                intent: "ask",
                reply: "The kettle is on the hob.",
                sources: &["card:kettle"],
            },
            Step {
                heard: "cite that",
                intent: "ask",
                reply: UNKNOWN_REPLY,
                sources: &["honest:unknown"],
            },
            Step {
                heard: "something else",
                intent: "chat",
                reply: UNKNOWN_REPLY,
                sources: &["honest:unknown"],
            },
            Step {
                heard: "I am so tired",
                intent: "vent",
                reply: UNKNOWN_REPLY,
                sources: &["honest:unknown"],
            },
            Step {
                heard: "goodbye",
                intent: "goodbye",
                reply: UNKNOWN_REPLY,
                sources: &["honest:unknown"],
            },
            Step {
                heard: "who won the match",
                intent: "ask",
                reply: UNKNOWN_REPLY,
                sources: &["honest:unknown"],
            },
            Step {
                heard: "  ",
                intent: "chat",
                reply: UNKNOWN_REPLY,
                sources: &["honest:unknown"],
            },
        ];
        let mut dialogue = fresh(learn);
        let mut decisions = Vec::new();
        for step in steps {
            let sources: Vec<String> = step
                .sources
                .iter()
                .map(|item| (*item).to_string())
                .collect();
            let decision = match hold {
                None => dialogue.decide(step_input(&step, &sources)),
                Some(flag) => dialogue.decide_held(step_input(&step, &sources), flag),
            };
            decisions.push(decision);
        }
        (decisions, dialogue)
    }

    fn assert_same_state(left: &Dialogue, right: &Dialogue) {
        assert_eq!(left.ring_len(), right.ring_len());
        for index in 0..left.ring_len() {
            assert_eq!(left.ring_get(index), right.ring_get(index));
        }
        assert_eq!(left.bandit_counts(), right.bandit_counts());
        assert_eq!(left.bandit_updates(), right.bandit_updates());
        assert_eq!(left.callbacks(), right.callbacks());
        assert_eq!(left.consent(), right.consent());
    }

    fn plan_mentions(plan: &ReplyPlan, needle: &str) -> bool {
        let needle = needle.to_ascii_lowercase();
        match plan {
            ReplyPlan::Keep => false,
            ReplyPlan::Replace {
                reply,
                sources,
                evidence,
            } => {
                reply.to_ascii_lowercase().contains(&needle)
                    || sources
                        .iter()
                        .any(|source| source.to_ascii_lowercase().contains(&needle))
                    || evidence
                        .iter()
                        .any(|source| source.to_ascii_lowercase().contains(&needle))
            }
        }
    }

    fn spoken<'a>(decision: &'a Decision, original: &'a str) -> &'a str {
        match &decision.plan {
            ReplyPlan::Keep => original.trim(),
            ReplyPlan::Replace { reply, .. } => reply,
        }
    }

    fn style_hit() -> ResearchHit {
        ResearchHit {
            id: "soft-pace".to_string(),
            text: "The spare kettle sits in the cupboard.".to_string(),
            licence: "in-memory".to_string(),
            dataset: "memory".to_string(),
            lang: "en".to_string(),
            score: 1.0,
            not_for: vec!["answer".to_string(), "cite".to_string()],
        }
    }

    #[test]
    fn hold_false_matches_decide_and_hold_true_blocks_offer_topic() {
        let (plain, plain_state) = play(None, true);
        let (held_off, held_off_state) = play(Some(false), true);
        assert_eq!(plain, held_off);
        assert_same_state(&plain_state, &held_off_state);
        let (plain_denied, denied_state) = play(None, false);
        let (held_denied, held_denied_state) = play(Some(false), false);
        assert_eq!(plain_denied, held_denied);
        assert_same_state(&denied_state, &held_denied_state);
        assert_eq!(denied_state.bandit_updates(), 0);

        const OFFER: usize = 2;
        assert_eq!(plain[OFFER].chosen, Some(Move::OfferTopic));
        assert_eq!(
            plain[OFFER].propensity,
            policy::propensity(Move::OfferTopic)
        );
        match &plain[OFFER].plan {
            ReplyPlan::Replace {
                reply,
                sources,
                evidence,
            } => {
                assert_eq!(reply, "Back to kettle?");
                assert_eq!(sources, &vec!["topic:kettle".to_string()]);
                assert_eq!(evidence, sources);
            }
            ReplyPlan::Keep => panic!("the offer kept the gathered miss"),
        }

        let (held_on, held_on_state) = play(Some(true), true);
        for index in 0..plain.len() {
            if index == OFFER {
                continue;
            }
            assert_eq!(plain[index], held_on[index]);
        }
        assert_eq!(held_on[OFFER].chosen, Some(Move::Ask));
        assert_ne!(held_on[OFFER].chosen, Some(Move::OfferTopic));
        assert_eq!(held_on[OFFER].propensity, policy::propensity(Move::Ask));
        assert!(matches!(held_on[OFFER].plan, ReplyPlan::Keep));
        assert!(!plan_mentions(&held_on[OFFER].plan, "Back to"));
        assert!(!plan_mentions(&held_on[OFFER].plan, "topic:"));
        assert_eq!(plain_state.ring_len(), held_on_state.ring_len());
        for index in 0..plain_state.ring_len() {
            if index == OFFER {
                continue;
            }
            assert_eq!(plain_state.ring_get(index), held_on_state.ring_get(index));
        }
        let offered = plain_state.ring_get(OFFER).expect("offer turn");
        let held = held_on_state.ring_get(OFFER).expect("held turn");
        assert_eq!(offered.chosen, Move::OfferTopic);
        assert_eq!(offered.clause, "Back to kettle?");
        assert_eq!(held.chosen, Move::Ask);
        assert_eq!(held.clause, UNKNOWN_REPLY);
        assert!(!held.clause.contains("Back to"));
        assert_eq!(held.topic.as_deref(), Some("kettle"));
        assert_eq!(offered.topic, held.topic);
        let mut counts = plain_state.bandit_counts();
        counts[Move::OfferTopic.index()] -= 1;
        counts[Move::Ask.index()] += 1;
        assert_eq!(held_on_state.bandit_counts(), counts);
        assert_eq!(held_on_state.bandit_updates(), plain_state.bandit_updates());
        assert_eq!(held_on_state.consent(), plain_state.consent());

        let unknown = vec!["honest:unknown".to_string()];
        let mut primed = fresh(true);
        let _ = primed.decide(input(
            "where is the kettle",
            "ask",
            UNKNOWN_REPLY,
            &unknown,
            &[],
        ));
        for _ in 0..30 {
            let _ = primed.decide(input("I am so tired", "vent", UNKNOWN_REPLY, &unknown, &[]));
        }
        assert!(primed.bandit_counts()[Move::Comfort.index()] >= 30);
        let mut as_decide = primed.clone();
        let mut as_off = primed.clone();
        let mut as_on = primed.clone();
        let vent_switch = |dialogue: &mut Dialogue, hold: Option<bool>| match hold {
            None => dialogue.decide(input(
                "I am so tired something else",
                "vent",
                UNKNOWN_REPLY,
                &unknown,
                &[],
            )),
            Some(flag) => dialogue.decide_held(
                input(
                    "I am so tired something else",
                    "vent",
                    UNKNOWN_REPLY,
                    &unknown,
                    &[],
                ),
                flag,
            ),
        };
        let flooded = vent_switch(&mut as_decide, None);
        let flooded_off = vent_switch(&mut as_off, Some(false));
        let flooded_on = vent_switch(&mut as_on, Some(true));
        assert_eq!(flooded, flooded_off);
        assert_eq!(flooded.chosen, Some(Move::OfferTopic));
        assert_eq!(spoken(&flooded, UNKNOWN_REPLY), "Back to kettle?");
        assert_eq!(as_decide.bandit_counts(), as_off.bandit_counts());
        assert_eq!(flooded_on.chosen, Some(Move::Ask));
        assert_ne!(flooded_on.chosen, Some(Move::Comfort));
        assert_eq!(spoken(&flooded_on, UNKNOWN_REPLY), UNKNOWN_REPLY);
        assert!(!plan_mentions(&flooded_on.plan, "Back to"));
        let flooded_snap = as_on.ring_get(as_on.ring_len() - 1).expect("held vent");
        assert_eq!(flooded_snap.chosen, Move::Ask);
        assert!(!flooded_snap.clause.contains("Back to"));

        let mut remembered = Dialogue::new();
        remembered.set_bandit_enabled(true);
        remembered.set_consent(DialogueConsent {
            remember: true,
            learn: true,
            check_ins: true,
        });
        assert!(remembered.note_callback("how did the interview go?"));
        let mut callback_decide = remembered.clone();
        let mut callback_off = remembered.clone();
        let mut callback_on = remembered.clone();
        let greet = |dialogue: &mut Dialogue, hold: Option<bool>| match hold {
            None => dialogue.decide(input("hello", "chat", UNKNOWN_REPLY, &unknown, &[])),
            Some(flag) => {
                dialogue.decide_held(input("hello", "chat", UNKNOWN_REPLY, &unknown, &[]), flag)
            }
        };
        let callback = greet(&mut callback_decide, None);
        let callback_same = greet(&mut callback_off, Some(false));
        let callback_held = greet(&mut callback_on, Some(true));
        assert_eq!(callback, callback_same);
        assert_eq!(callback.chosen, Some(Move::OfferTopic));
        assert_eq!(
            spoken(&callback, UNKNOWN_REPLY),
            "how did the interview go?"
        );
        assert_eq!(callback_held.chosen, Some(Move::Ask));
        assert_eq!(spoken(&callback_held, UNKNOWN_REPLY), UNKNOWN_REPLY);
        assert!(!plan_mentions(&callback_held.plan, "interview"));
        assert!(!plan_mentions(&callback_held.plan, "Back to"));
        assert_eq!(
            callback_on.callbacks(),
            &["how did the interview go?".to_string()]
        );

        let hit = style_hit();
        assert!(!hit.allows_answer());
        assert!(!hit.allows_cite());
        assert!(hit.tagged_not_for_answer_and_cite());
        let hits = [hit.clone()];
        let research_sources = vec!["card:soft-pace".to_string()];
        let mut researched = fresh(true);
        let _ = researched.decide(input(
            "where is the kettle",
            "ask",
            UNKNOWN_REPLY,
            &unknown,
            &[],
        ));
        let mut research_decide = researched.clone();
        let mut research_off = researched.clone();
        let mut research_on = researched.clone();
        let research_line = |dialogue: &mut Dialogue, hold: Option<bool>| match hold {
            None => dialogue.decide(input(
                "something else",
                "chat",
                hits[0].text.as_str(),
                &research_sources,
                &hits,
            )),
            Some(flag) => dialogue.decide_held(
                input(
                    "something else",
                    "chat",
                    hits[0].text.as_str(),
                    &research_sources,
                    &hits,
                ),
                flag,
            ),
        };
        let research_offer = research_line(&mut research_decide, None);
        let research_same = research_line(&mut research_off, Some(false));
        let research_held = research_line(&mut research_on, Some(true));
        assert_eq!(research_offer, research_same);
        assert_eq!(research_offer.chosen, Some(Move::OfferTopic));
        assert_ne!(research_offer.chosen, Some(Move::Answer));
        assert_ne!(research_offer.chosen, Some(Move::Cite));
        assert!(!spoken(&research_offer, hits[0].text.as_str())
            .to_ascii_lowercase()
            .contains("cupboard"));
        assert_eq!(research_held.chosen, Some(Move::Ask));
        assert_ne!(research_held.chosen, Some(Move::Answer));
        assert_ne!(research_held.chosen, Some(Move::Cite));
        assert_eq!(spoken(&research_held, hits[0].text.as_str()), UNKNOWN_REPLY);
        assert!(!plan_mentions(&research_held.plan, "cupboard"));
        assert!(!plan_mentions(&research_held.plan, "soft-pace"));
        assert!(!plan_mentions(&research_held.plan, "Back to"));
        let research_snap = research_on
            .ring_get(research_on.ring_len() - 1)
            .expect("held research turn");
        assert_eq!(research_snap.chosen, Move::Ask);
        assert!(!research_snap
            .clause
            .to_ascii_lowercase()
            .contains("cupboard"));
        assert!(!research_snap.clause.contains("Back to"));

        let mut cited = Dialogue::new();
        let cite_sources = vec!["card:soft-pace".to_string()];
        let cited_plain = cited.decide(input(
            "cite that",
            "ask",
            hits[0].text.as_str(),
            &cite_sources,
            &hits,
        ));
        let mut cited_off = Dialogue::new();
        let cited_same = cited_off.decide_held(
            input(
                "cite that",
                "ask",
                hits[0].text.as_str(),
                &cite_sources,
                &hits,
            ),
            false,
        );
        let mut cited_on = Dialogue::new();
        let cited_held = cited_on.decide_held(
            input(
                "cite that",
                "ask",
                hits[0].text.as_str(),
                &cite_sources,
                &hits,
            ),
            true,
        );
        assert_eq!(cited_plain, cited_same);
        assert_eq!(cited_plain, cited_held);
        assert_ne!(cited_held.chosen, Some(Move::Answer));
        assert_ne!(cited_held.chosen, Some(Move::Cite));
        assert_eq!(spoken(&cited_held, hits[0].text.as_str()), UNKNOWN_REPLY);
        assert!(!spoken(&cited_held, hits[0].text.as_str())
            .to_ascii_lowercase()
            .contains("cupboard"));

        let card = vec!["card:kettle".to_string()];
        let mut answered = Dialogue::new();
        let answer = answered.decide(input(
            "where is the kettle",
            "ask",
            "The kettle is on the hob.",
            &card,
            &hits,
        ));
        let mut answered_on = Dialogue::new();
        let answer_held = answered_on.decide_held(
            input(
                "where is the kettle",
                "ask",
                "The kettle is on the hob.",
                &card,
                &hits,
            ),
            true,
        );
        assert_eq!(answer, answer_held);
        assert_eq!(answer_held.chosen, Some(Move::Answer));
        assert_eq!(
            spoken(&answer_held, "The kettle is on the hob."),
            "The kettle is on the hob."
        );
    }
}
