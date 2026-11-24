// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! The moment core sits beside empathy. A swing there does not rewrite the mood.

use horbm::{Brain, CardHit, CardLookup, IntentMsg, EMOTION_FRAME_MS, FRAME_MS, UNKNOWN_REPLY};

struct NoCards;

impl CardLookup for NoCards {
    fn lookup(&self, _query: &str) -> Option<CardHit> {
        None
    }
}

#[test]
fn step_moment_leaves_empathy_and_the_baseline_alone() {
    assert_eq!(FRAME_MS, EMOTION_FRAME_MS);
    assert_eq!(FRAME_MS, 80);
    let mut brain = Brain::with_cards(NoCards, None);
    let mood = brain.mood_meta();
    let baseline = brain.emotion().baseline().identity_bits();
    for i in 0..40 {
        let mut msg = IntentMsg::steady(i * EMOTION_FRAME_MS);
        msg.fatigue = 0.9;
        brain.step_moment(&msg).unwrap();
    }
    assert_eq!(brain.mood_meta(), mood);
    assert_eq!(brain.emotion().baseline().identity_bits(), baseline);
    let cue = brain.conductor_cue();
    assert!(cue.fatigue > 0.8);
    assert!(cue.arousal() < brain.emotion().baseline().arousal());
    assert!(!cue.may_share);
    let out = brain.turn("where is the kettle");
    assert_eq!(out.reply, UNKNOWN_REPLY);
    assert_eq!(brain.emotion().baseline().identity_bits(), baseline);
}
