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

#[test]
fn inner_life_surprise_nudges_the_moment_and_does_not_learn() {
    let mut brain = Brain::with_cards(NoCards, None);
    let mood = brain.mood_meta();
    let baseline = brain.emotion().baseline().identity_bits();
    let kept = brain.inner().kept_expect();
    let before_care = brain.inner().cares()[0].strength;

    let calm = brain.inner_turn("speaking").unwrap();
    assert_eq!(calm.verb_str(), "hold");
    assert_eq!(brain.emotion().baseline().identity_bits(), baseline);

    let surprise = brain.inner_turn("refused").unwrap();
    assert_eq!(surprise.verb_str(), "surprise");
    assert_eq!(surprise.source, horbm::IntentSource::Own);
    let to_mne = surprise.encode().unwrap();
    assert_eq!(horbm::IntentMsg::decode(&to_mne).unwrap(), surprise);

    let mut nudge = surprise;
    for i in 2..40u32 {
        nudge.t_ms = i * EMOTION_FRAME_MS;
        brain.step_moment(&nudge).unwrap();
    }
    assert!(brain.conductor_cue().arousal() > brain.emotion().baseline().arousal());
    assert_eq!(brain.emotion().baseline().identity_bits(), baseline);
    assert_eq!(brain.mood_meta(), mood);
    assert!(brain.inner().cares()[0].strength < before_care);
    assert!(!brain.learn_inner());
    assert_eq!(brain.inner().kept_expect().cue_str(), kept.cue_str());
    assert_eq!(brain.inner().kept_cares()[0].strength, before_care);

    let mut inbound = horbm::IntentMsg::steady(40 * EMOTION_FRAME_MS);
    inbound.source = horbm::IntentSource::Inbound;
    inbound.set_verb("ease").unwrap();
    inbound.strength = 0.4;
    inbound.set_pull(0, -0.2).unwrap();
    let to_life = inbound.encode().unwrap();
    let decoded = horbm::IntentMsg::decode(&to_life).unwrap();
    assert_eq!(decoded.encode().unwrap(), to_life);
    let valence = brain.inner().expect().point[0];
    brain.take_mne_intent(&decoded).unwrap();
    assert!(brain.inner().expect().point[0] < valence);
    assert_eq!(
        brain.inner().last_inbound().unwrap().source,
        horbm::IntentSource::Inbound
    );
    assert_eq!(brain.emotion().baseline().identity_bits(), baseline);
    assert_eq!(brain.mood_meta(), mood);

    let out = brain.turn("where is the kettle");
    assert_eq!(out.reply, UNKNOWN_REPLY);
    assert_eq!(brain.emotion().baseline().identity_bits(), baseline);
}
