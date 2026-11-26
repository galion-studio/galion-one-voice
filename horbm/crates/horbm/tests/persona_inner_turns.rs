// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! The dialogue path reads the installed inner lean. The spoken clause stays
//! the card or the honest miss. Comfort does not open research and does not
//! invent a line.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use horbm::{
    AskOutcome, Brain, CardHit, CardLookup, GapReason, Heard, ResearchAsk, ResearchFollowUp, Verb,
    CHECKIN_MS, TICK_MS, UNKNOWN_REPLY, WAIT_MS,
};

struct Cards;

impl CardLookup for Cards {
    fn lookup(&self, query: &str) -> Option<CardHit> {
        if query.to_ascii_lowercase().contains("kettle") {
            Some(CardHit {
                id: "kettle".into(),
                gist: "The kettle is on the hob.".into(),
            })
        } else {
            None
        }
    }
}

struct Gateway {
    calls: Arc<AtomicU32>,
}

impl ResearchAsk for Gateway {
    fn ask(&mut self, _question: &str, cap: Duration) -> AskOutcome {
        assert_eq!(cap, horbm::RESEARCH_CAP);
        self.calls.fetch_add(1, Ordering::SeqCst);
        AskOutcome::Miss
    }
}

#[test]
fn a_sad_line_comforts_without_a_script_or_a_lookup() {
    let calls = Arc::new(AtomicU32::new(0));
    let mut brain = Brain::with_cards(
        Cards,
        Some(Box::new(Gateway {
            calls: Arc::clone(&calls),
        })),
    );
    let out = brain.turn("why am I so sad?");
    assert_eq!(out.reply, UNKNOWN_REPLY);
    assert_eq!(brain.dialogue_lean().verb, Verb::Comfort);
    assert!(brain.persona_inner().care() > brain.persona_inner().home().care);
    assert!(out.mood_meta.rate < 1.0);
    assert!(out.mood_meta.pause_ms >= 120.0);
    assert!(!brain.dialogue_lean().hold_topic);
    for banned in ["i hear you", "that sounds", "i'm here for you", "sorry"] {
        assert!(!out.reply.to_ascii_lowercase().contains(banned));
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(brain.gaps().last().unwrap().reason, GapReason::ResearchOff);
    assert!(brain.emotion().persisted_baseline().is_none());
    assert!(!brain.persona_inner().keeps_user_facts());
}

#[test]
fn a_question_still_speaks_the_card_and_expects_an_answer() {
    let mut brain = Brain::with_cards(Cards, None);
    let out = brain.turn("where is the kettle");
    assert_eq!(out.reply, "The kettle is on the hob.");
    assert_eq!(out.first_clause, "The kettle");
    assert_eq!(brain.dialogue_lean().verb, Verb::Answer);
    assert!(brain.persona_inner().expect() > brain.persona_inner().home().expect);
    assert!(brain.may_open_topic());
}

#[test]
fn a_plain_deep_question_asks_research_after_the_clause() {
    let calls = Arc::new(AtomicU32::new(0));
    let mut brain = Brain::with_cards(
        Cards,
        Some(Box::new(Gateway {
            calls: Arc::clone(&calls),
        })),
    );
    let out = brain.turn("why is the spare fuse missing from the drawer?");
    assert_eq!(out.reply, UNKNOWN_REPLY);
    assert_eq!(out.first_clause, UNKNOWN_REPLY);
    assert_eq!(brain.dialogue_lean().verb, Verb::Answer);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(brain.gaps().last().unwrap().reason, GapReason::ResearchOff);
    drop(out);
    let follow = wait_follow(&mut brain);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(follow.outcome, AskOutcome::Miss);
    assert_eq!(follow.gap.reason, GapReason::ResearchMiss);
    assert_eq!(brain.gaps().last().unwrap().reason, GapReason::ResearchMiss);
}

#[test]
fn the_moment_warms_on_her_note_and_the_baseline_stays() {
    let mut brain = Brain::with_cards(Cards, None);
    let bits = brain.emotion().baseline().identity_bits();
    let pitch = brain.emotion().baseline().pitch_centre_hz.to_bits();
    let start_v = brain.conductor_cue().valence();
    let start_a = brain.conductor_cue().arousal();
    for _ in 0..24 {
        let note = brain
            .persona_nudge("I feel really sad", None, false)
            .expect("nudge");
        assert_eq!(note.verb, Verb::Comfort);
    }
    assert!(brain.conductor_cue().valence() > start_v);
    assert!(brain.conductor_cue().arousal() < start_a);
    assert_eq!(brain.emotion().baseline().identity_bits(), bits);
    assert_eq!(brain.emotion().baseline().pitch_centre_hz.to_bits(), pitch);
    assert!(brain.emotion().persisted_baseline().is_none());
    assert_eq!(brain.emotion().consent() & horbm_inner_remember_mask(), 0);
}

fn wait_follow(brain: &mut Brain) -> ResearchFollowUp {
    let started = Instant::now();
    loop {
        if let Some(follow) = brain.research_follow_up() {
            return follow;
        }
        if started.elapsed() > Duration::from_secs(2) {
            panic!("research follow-up did not arrive");
        }
        thread::sleep(Duration::from_millis(5));
    }
}

fn horbm_inner_remember_mask() -> u8 {
    // EmotionWire remember bit. The nudge must not set it.
    1 << 2
}

#[test]
fn sense_off_still_hears_the_words_and_drops_the_ear() {
    let mut brain = Brain::with_cards(Cards, None);
    let ignored = brain
        .persona_nudge(
            "hello",
            Some(Heard {
                valence: -0.6,
                arousal: 0.3,
                confidence: 0.95,
            }),
            false,
        )
        .expect("words");
    assert_eq!(ignored.verb, Verb::With);
    assert_eq!(
        brain.persona_inner().care().to_bits(),
        brain.persona_inner().home().care.to_bits()
    );

    let mut brain = Brain::with_cards(Cards, None);
    let words = brain
        .persona_nudge("I feel sad", None, false)
        .expect("sad words");
    assert_eq!(words.verb, Verb::Comfort);
    assert!(brain.persona_inner().care() > brain.persona_inner().home().care);
}

#[test]
fn persona_nudge_answers_from_the_installed_persona() {
    let mut brain = Brain::with_cards(Cards, None);
    let forwarded = brain
        .persona_nudge("where is the kettle", None, true)
        .expect("nudge");
    assert_eq!(forwarded.verb, Verb::Answer);
    let _ = brain.persona_inner();
}

#[test]
fn a_long_quiet_waits_and_does_not_open_a_topic() {
    let mut brain = Brain::with_cards(Cards, None);
    assert!(brain.may_open_topic());
    for _ in 0..(WAIT_MS / TICK_MS) {
        brain.tick_frame();
    }
    assert_eq!(brain.dialogue_lean().verb, Verb::Wait);
    assert!(!brain.may_open_topic());
    let frames = CHECKIN_MS / TICK_MS;
    for _ in (WAIT_MS / TICK_MS)..frames {
        brain.tick_frame();
    }
    assert_eq!(brain.dialogue_lean().verb, Verb::CheckIn);
    assert!(!brain.may_open_topic());
    brain.tick_frame();
    assert_eq!(brain.dialogue_lean().verb, Verb::Wait);
    assert_eq!(
        brain.emotion().frames(),
        0,
        "tick_frame does not step the moment"
    );
}
