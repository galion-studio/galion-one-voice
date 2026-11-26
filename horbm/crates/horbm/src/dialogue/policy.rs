// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Deterministic policy, plus an offline bandit that does not steer.
//!
//! The heuristic picks the move. The bandit may count that move when it is
//! switched on and learn consent is granted. Counting does not change the
//! next move. [`OfflineBandit::choose_steered`] is the optional count tie-in.
//! `Dialogue::decide` and `Dialogue::decide_held` do not call it.

use super::cues::{self, CheckInPhrase};
use super::filter::{self, Legal};
use super::intent::TurnIntent;
use super::moves::Move;
use crate::research_style::ResearchHit;
use crate::UNKNOWN_REPLY;

/// What the gathered evidence looks like, plus the ring facts the heuristic
/// is allowed to reuse.
pub struct Choice<'a> {
    pub heard: &'a str,
    pub intent: TurnIntent,
    pub reply: &'a str,
    pub sources: &'a [String],
    pub research: &'a [ResearchHit],
    pub current_topic: Option<&'a str>,
    pub ring_source: Option<&'a str>,
    pub ring_clause: Option<&'a str>,
    pub callback: Option<&'a str>,
    pub check_ins: bool,
    pub remember: bool,
}

/// Spoken text after the move is chosen. `Keep` leaves the gathered clause.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReplyPlan {
    Keep,
    Replace {
        reply: String,
        sources: Vec<String>,
        evidence: Vec<String>,
    },
}

impl ReplyPlan {
    fn line(reply: &str, source: &str) -> Self {
        Self::Replace {
            reply: reply.to_string(),
            sources: vec![source.to_string()],
            evidence: vec![source.to_string()],
        }
    }

    fn miss() -> Self {
        Self::line(UNKNOWN_REPLY, "honest:unknown")
    }
}

/// The ask miss. The reply is the existing unknown line. Evidence stays empty,
/// matching a gathered miss. Not a new sentence.
fn honest_unknown() -> ReplyPlan {
    ReplyPlan::Replace {
        reply: UNKNOWN_REPLY.to_string(),
        sources: vec!["honest:unknown".to_string()],
        evidence: Vec::new(),
    }
}

/// Recall speaks a person diary hit (`memory:` plus an id). A callback offer
/// is `memory:callback` and is not that hit. A card is not a memory.
fn recall_without_memory(choice: &Choice<'_>) -> bool {
    choice.intent == TurnIntent::Recall
        && !choice.sources.iter().any(|source| is_person_memory(source))
}

fn is_person_memory(source: &str) -> bool {
    match source.strip_prefix("memory:") {
        Some(id) => !id.is_empty() && id != "callback",
        None => false,
    }
}

enum Offer<'a> {
    Callback(&'a str),
    Topic(&'a str),
}

/// Heuristic scores, then the persona and truth mask. Higher wins.
/// Ties keep the earlier move in [`Move::ALL`].
pub fn choose(choice: &Choice<'_>) -> (Move, ReplyPlan, f32) {
    choose_held(choice, false)
}

/// [`choose`] when `hold_topic` is false. A hold that selected
/// [`Move::OfferTopic`] becomes [`Move::Ask`] before the offer line is built.
pub(super) fn choose_held(choice: &Choice<'_>, hold_topic: bool) -> (Move, ReplyPlan, f32) {
    // A correction is never an argument, and a recall with no person-memory
    // source is never a card. Both use the ask miss. The old labels do not
    // enter this branch.
    if choice.intent == TurnIntent::Correct || recall_without_memory(choice) {
        return (Move::Ask, honest_unknown(), propensity(Move::Ask));
    }
    let sourced = filter::is_sourced_truth(choice.reply, choice.sources, choice.research);
    let hits = choice.research;
    let stop = cues::is_stop(choice.heard, choice.intent);
    let cite = cues::is_cite(choice.heard);
    let switch = cues::is_switch(choice.heard);
    let greet = cues::is_greeting(choice.heard);
    let vent = choice.intent == TurnIntent::Vent;

    let (cite_source, cite_clause) = if sourced {
        (
            independent_source(choice.sources, hits),
            citable_clause(choice.reply, hits),
        )
    } else if cite {
        let source = choice.ring_source.filter(|source| {
            filter::is_real_source(source) && !filter::source_is_research(source, hits)
        });
        let clause = choice
            .ring_clause
            .and_then(|clause| citable_clause(clause, hits));
        (source, clause)
    } else {
        (None, None)
    };
    let cite_ok = cite && cite_source.is_some() && cite_clause.is_some();

    let callback = if choice.check_ins && choice.remember && greet && !sourced {
        choice.callback.filter(|text| {
            let text = text.trim();
            !text.is_empty() && !filter::is_scripted_comfort(text)
        })
    } else {
        None
    };
    let switch_topic = if switch && !sourced {
        choice.current_topic.filter(|topic| !topic.is_empty())
    } else {
        None
    };
    let offer = if let Some(text) = callback {
        Some(Offer::Callback(text))
    } else {
        switch_topic.map(Offer::Topic)
    };

    let mut scores = [i32::MIN; 6];
    if stop {
        scores[Move::Stop.index()] = 100;
    }
    if cite_ok {
        scores[Move::Cite.index()] = 80;
    }
    if offer.is_some() {
        scores[Move::OfferTopic.index()] = 55;
    }
    if sourced {
        scores[Move::Answer.index()] = 60;
    }
    if vent && !sourced {
        scores[Move::Comfort.index()] = 50;
    }
    scores[Move::Ask.index()] = 10;
    let scores = filter::mask(
        scores,
        &Legal {
            sourced,
            stop,
            cite_ok,
            offer: offer.is_some(),
            vent,
        },
    );
    let mut chosen = argmax(scores);
    // A hold does not open a new topic. The offer was already selected, so
    // this becomes ask. Comfort can sit just under the offer; do not drop
    // the offer and take that next score. The offer line is not built.
    if hold_topic && chosen == Move::OfferTopic {
        chosen = Move::Ask;
    }
    let plan = plan_for(chosen, choice, sourced, cite_source, cite_clause, offer);
    (chosen, plan, propensity(chosen))
}

fn plan_for(
    chosen: Move,
    choice: &Choice<'_>,
    sourced: bool,
    cite_source: Option<&str>,
    cite_clause: Option<&str>,
    offer: Option<Offer<'_>>,
) -> ReplyPlan {
    match chosen {
        Move::Stop => ReplyPlan::line("Talk soon.", "persona:talk-soon"),
        Move::Cite if sourced => ReplyPlan::Keep,
        Move::Cite => ReplyPlan::line(
            cite_clause.unwrap_or(UNKNOWN_REPLY),
            cite_source.unwrap_or("honest:unknown"),
        ),
        Move::OfferTopic => match offer {
            Some(Offer::Callback(text)) => ReplyPlan::line(text, "memory:callback"),
            Some(Offer::Topic(topic)) => {
                let reply = format!("Back to {topic}?");
                let source = format!("topic:{topic}");
                ReplyPlan::Replace {
                    reply,
                    sources: vec![source.clone()],
                    evidence: vec![source],
                }
            }
            None => ReplyPlan::miss(),
        },
        Move::Answer => ReplyPlan::Keep,
        Move::Comfort | Move::Ask => {
            if filter::research_blocks_speech(choice.reply, choice.sources, choice.research) {
                ReplyPlan::miss()
            } else {
                keep_honest(choice.reply)
            }
        }
    }
}

fn independent_source<'a>(sources: &'a [String], hits: &[ResearchHit]) -> Option<&'a str> {
    sources
        .iter()
        .find(|source| filter::is_real_source(source) && !filter::source_is_research(source, hits))
        .map(String::as_str)
}

fn citable_clause<'a>(clause: &'a str, hits: &[ResearchHit]) -> Option<&'a str> {
    let trimmed = clause.trim();
    if trimmed.is_empty() || trimmed == UNKNOWN_REPLY || filter::is_scripted_comfort(trimmed) {
        return None;
    }
    if hits
        .iter()
        .any(|hit| filter::reply_uses_example(clause, hit))
    {
        return None;
    }
    Some(clause)
}

fn keep_honest(reply: &str) -> ReplyPlan {
    let trimmed = reply.trim();
    if trimmed.is_empty() || trimmed == UNKNOWN_REPLY {
        ReplyPlan::Keep
    } else if filter::is_scripted_comfort(trimmed) {
        ReplyPlan::miss()
    } else {
        ReplyPlan::Keep
    }
}

pub fn argmax(scores: [i32; 6]) -> Move {
    let mut best = Move::Ask;
    let mut best_score = scores[Move::Ask.index()];
    for move_ in Move::ALL {
        let score = scores[move_.index()];
        if score > best_score {
            best = move_;
            best_score = score;
        }
    }
    best
}

pub fn propensity(move_: Move) -> f32 {
    match move_ {
        Move::Answer => 0.8,
        Move::Cite => 0.7,
        Move::OfferTopic => 0.6,
        Move::Comfort => 0.5,
        Move::Ask => 0.4,
        Move::Stop => 0.9,
    }
}

/// Same scores with zero counts match [`argmax`]. A count never revives an
/// illegal arm (`i32::MIN`).
pub fn steered(scores: [i32; 6], counts: [u32; 6]) -> Move {
    let mut adjusted = scores;
    for (score, count) in adjusted.iter_mut().zip(counts) {
        if *score == i32::MIN {
            continue;
        }
        let bump = i32::try_from(count).unwrap_or(i32::MAX);
        *score = score.saturating_add(bump);
    }
    argmax(adjusted)
}

/// Optional counter. Off by default. `observe` writes a count only when
/// enabled and `learn` is granted. It does not pick the move.
#[derive(Clone, Debug)]
pub struct OfflineBandit {
    enabled: bool,
    counts: [u32; 6],
    updates: u32,
}

impl Default for OfflineBandit {
    fn default() -> Self {
        Self::new()
    }
}

impl OfflineBandit {
    pub const fn new() -> Self {
        Self {
            enabled: false,
            counts: [0; 6],
            updates: 0,
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn updates(&self) -> u32 {
        self.updates
    }

    pub fn counts(&self) -> [u32; 6] {
        self.counts
    }

    pub fn observe(&mut self, chosen: Move, learn: bool) {
        if !self.enabled || !learn {
            return;
        }
        let index = chosen.index();
        self.counts[index] = self.counts[index].saturating_add(1);
        self.updates = self.updates.saturating_add(1);
    }

    pub fn choose_steered(&self, scores: [i32; 6]) -> Move {
        steered(scores, self.counts)
    }
}

pub fn check_in_phrase(text: &str) -> CheckInPhrase {
    cues::check_in_phrase(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn choice<'a>(
        heard: &'a str,
        intent: &'a str,
        reply: &'a str,
        sources: &'a [String],
    ) -> Choice<'a> {
        Choice {
            heard,
            intent: super::TurnIntent::read(heard, intent),
            reply,
            sources,
            research: &[],
            current_topic: None,
            ring_source: None,
            ring_clause: None,
            callback: None,
            check_ins: false,
            remember: false,
        }
    }

    #[test]
    fn each_move_wins_its_own_case_and_an_unsourced_answer_cannot() {
        let card = vec!["card:kettle".to_string()];
        let unknown = vec!["honest:unknown".to_string()];
        let (answer, _, _) = choose(&choice(
            "where is the kettle",
            "ask",
            "The kettle is on the hob.",
            &card,
        ));
        assert_eq!(answer, Move::Answer);
        let (ask, plan, _) = choose(&choice("who won the match", "ask", UNKNOWN_REPLY, &unknown));
        assert_eq!(ask, Move::Ask);
        assert!(matches!(plan, ReplyPlan::Keep));
        let (comfort, _, _) = choose(&choice("I am so tired", "vent", UNKNOWN_REPLY, &unknown));
        assert_eq!(comfort, Move::Comfort);
        let (stop, _, _) = choose(&choice("goodbye", "goodbye", UNKNOWN_REPLY, &unknown));
        assert_eq!(stop, Move::Stop);
        let mut cite = choice("cite that", "ask", UNKNOWN_REPLY, &unknown);
        cite.ring_source = Some("card:kettle");
        cite.ring_clause = Some("The kettle is on the hob.");
        let (cited, plan, _) = choose(&cite);
        assert_eq!(cited, Move::Cite);
        match plan {
            ReplyPlan::Replace { reply, sources, .. } => {
                assert_eq!(reply, "The kettle is on the hob.");
                assert_eq!(sources, vec!["card:kettle".to_string()]);
            }
            ReplyPlan::Keep => panic!("cite had no current card"),
        }
        let mut offer = choice("something else", "chat", UNKNOWN_REPLY, &unknown);
        offer.current_topic = Some("kettle");
        let (offered, _, _) = choose(&offer);
        assert_eq!(offered, Move::OfferTopic);
    }

    #[test]
    fn zero_counts_match_the_heuristic_and_an_illegal_arm_stays_down() {
        let scores = [i32::MIN, i32::MIN, i32::MIN, 60, i32::MIN, 10];
        assert_eq!(argmax(scores), Move::Answer);
        assert_eq!(steered(scores, [0; 6]), Move::Answer);
        assert_eq!(steered(scores, [0; 6]), steered(scores, [0; 6]));
        let mut flooded = [9_000u32; 6];
        flooded[Move::Answer.index()] = 0;
        flooded[Move::Ask.index()] = 0;
        assert_eq!(steered(scores, flooded), Move::Answer);
        let bandit = OfflineBandit::new();
        assert!(!bandit.enabled());
        assert_eq!(bandit.choose_steered(scores), Move::Answer);
    }

    #[test]
    fn a_research_hit_is_not_an_answer_and_not_a_citation() {
        let hit = ResearchHit {
            id: "soft-pace".to_string(),
            text: "The spare kettle sits in the cupboard.".to_string(),
            licence: "in-memory".to_string(),
            dataset: "memory".to_string(),
            lang: "en".to_string(),
            score: 1.0,
            not_for: vec!["answer".to_string(), "cite".to_string()],
        };
        assert!(!hit.allows_answer());
        assert!(!hit.allows_cite());
        assert!(hit.tagged_not_for_answer_and_cite());
        let hits = [hit.clone()];
        let card = vec!["card:soft-pace".to_string()];
        let mut asked = choice(
            "where is the spare kettle",
            "ask",
            "The spare kettle sits in the cupboard.",
            &card,
        );
        asked.research = &hits;
        let (chosen, plan, _) = choose(&asked);
        assert_ne!(chosen, Move::Answer);
        assert_ne!(chosen, Move::Cite);
        assert_eq!(spoken(&plan, asked.reply), UNKNOWN_REPLY);
        assert!(!plan_mentions(&plan, "cupboard"));
        assert!(!plan_mentions(&plan, "soft-pace"));

        let unknown = vec!["honest:unknown".to_string()];
        let mut cited = choice("cite that", "ask", UNKNOWN_REPLY, &unknown);
        cited.research = &hits;
        cited.ring_source = Some("retrieve:soft-pace");
        cited.ring_clause = Some(hit.text.as_str());
        let (chosen, plan, _) = choose(&cited);
        assert_ne!(chosen, Move::Cite);
        assert_ne!(spoken(&plan, cited.reply), hit.text.as_str());
        assert!(!spoken(&plan, cited.reply)
            .to_ascii_lowercase()
            .contains("cupboard"));

        let truth = vec!["card:kettle".to_string()];
        let mut still = choice(
            "where is the kettle",
            "ask",
            "The kettle is on the hob.",
            &truth,
        );
        still.research = &hits;
        let (chosen, _, _) = choose(&still);
        assert_eq!(chosen, Move::Answer);
    }

    fn spoken<'a>(plan: &'a ReplyPlan, original: &'a str) -> &'a str {
        match plan {
            ReplyPlan::Keep => original,
            ReplyPlan::Replace { reply, .. } => reply,
        }
    }

    fn plan_mentions(plan: &ReplyPlan, needle: &str) -> bool {
        match plan {
            ReplyPlan::Keep => false,
            ReplyPlan::Replace { reply, sources, .. } => {
                reply.to_ascii_lowercase().contains(needle)
                    || sources.iter().any(|source| source.contains(needle))
            }
        }
    }

    #[test]
    fn recall_keeps_a_person_source_and_a_card_or_a_correction_is_unknown() {
        let card = vec!["card:kettle".to_string()];
        let (asked, plan, score) = choose(&choice(
            "do you remember where the kettle is",
            "ask",
            "The kettle is on the hob.",
            &card,
        ));
        assert_eq!(asked, Move::Ask);
        assert_eq!(score, propensity(Move::Ask));
        assert_eq!(spoken(&plan, "The kettle is on the hob."), UNKNOWN_REPLY);
        assert!(!spoken(&plan, "The kettle is on the hob.").contains("hob"));
        match plan {
            ReplyPlan::Replace {
                sources, evidence, ..
            } => {
                assert_eq!(sources, vec!["honest:unknown".to_string()]);
                assert!(evidence.is_empty());
            }
            ReplyPlan::Keep => panic!("a card was kept as a memory"),
        }

        let callback = vec!["memory:callback".to_string()];
        let (asked, plan, _) = choose(&choice(
            "do you remember",
            "chat",
            "how did the interview go?",
            &callback,
        ));
        assert_eq!(asked, Move::Ask);
        assert_eq!(spoken(&plan, "how did the interview go?"), UNKNOWN_REPLY);

        let memory = vec!["memory:pin-1".to_string()];
        let (asked, plan, _) = choose(&choice(
            "do you remember the blue bowl",
            "ask",
            "you remember the blue bowl is mine",
            &memory,
        ));
        assert_eq!(asked, Move::Ask);
        assert_eq!(
            spoken(&plan, "you remember the blue bowl is mine"),
            "you remember the blue bowl is mine"
        );
        assert!(matches!(plan, ReplyPlan::Keep));

        let mut wrong = choice(
            "no, that's wrong",
            "ask",
            "The kettle is on the hob.",
            &card,
        );
        wrong.ring_source = Some("card:kettle");
        wrong.ring_clause = Some("The kettle is on the hob.");
        let (asked, plan, _) = choose(&wrong);
        assert_eq!(asked, Move::Ask);
        assert_eq!(spoken(&plan, "The kettle is on the hob."), UNKNOWN_REPLY);
        assert_ne!(
            spoken(&plan, "The kettle is on the hob."),
            wrong.ring_clause.unwrap()
        );
    }

    #[test]
    fn observe_is_a_no_op_until_the_bandit_is_on_and_learn_is_granted() {
        let mut bandit = OfflineBandit::new();
        bandit.observe(Move::Ask, true);
        assert_eq!(bandit.updates(), 0);
        bandit.set_enabled(true);
        bandit.observe(Move::Ask, false);
        assert_eq!(bandit.updates(), 0);
        bandit.observe(Move::Ask, true);
        assert_eq!(bandit.updates(), 1);
        assert_eq!(bandit.counts()[Move::Ask.index()], 1);
    }
}
