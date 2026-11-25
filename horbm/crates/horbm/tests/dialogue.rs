// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Layer 4. Each move in its scenario, and the consent gates.

use horbm::{
    flag_granted, Brain, CardHit, CardLookup, DialogueConsent, PronounClass, UNKNOWN_REPLY,
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

fn open_brain() -> Brain {
    Brain::with_cards(Cards, None)
}

type Spoken = (
    String,
    String,
    Vec<String>,
    Vec<String>,
    String,
    String,
    f32,
);

fn fields(out: &horbm::TurnOut) -> Spoken {
    (
        out.reply.clone(),
        out.arm.clone(),
        out.sources.clone(),
        out.evidence.clone(),
        out.first_clause.clone(),
        out.rest.clone(),
        out.propensity,
    )
}

#[test]
fn answer_is_the_sourced_clause() {
    let mut brain = open_brain();
    let out = brain.turn("where is the kettle");
    assert_eq!(out.arm, "answer");
    assert_eq!(out.reply, "The kettle is on the hob.");
    assert_eq!(out.first_clause, "The kettle");
    assert_eq!(out.rest, "is on the hob.");
    assert_eq!(out.sources, vec!["card:kettle".to_string()]);
    assert_eq!(out.propensity, 0.8);
    assert_eq!(brain.ring_len(), 1);
    assert_eq!(
        brain.ring_get(0).and_then(|snap| snap.topic.as_deref()),
        Some("kettle")
    );
}

#[test]
fn cite_repeats_the_sourced_clause_and_a_cold_cite_does_not_invent_one() {
    let mut brain = open_brain();
    let cold = brain.turn("cite that");
    assert_eq!(cold.arm, "ask");
    assert_eq!(cold.reply, UNKNOWN_REPLY);
    assert_eq!(cold.sources, vec!["honest:unknown".to_string()]);
    assert!(cold.evidence.is_empty());
    assert_ne!(cold.arm, "answer");
    assert_ne!(cold.arm, "cite");

    let mut brain = open_brain();
    let hit = brain.turn("where is the kettle");
    assert_eq!(hit.arm, "answer");
    let cited = brain.turn("cite that");
    assert_eq!(cited.arm, "cite");
    assert_eq!(cited.reply, "The kettle is on the hob.");
    assert_eq!(cited.first_clause, "The kettle");
    assert_eq!(cited.rest, "is on the hob.");
    assert_eq!(cited.sources, vec!["card:kettle".to_string()]);
    assert_eq!(cited.propensity, 0.7);
}

#[test]
fn ask_is_the_miss_and_answer_never_speaks_without_a_card() {
    let mut brain = open_brain();
    let out = brain.turn("who won the match");
    assert_eq!(out.arm, "ask");
    assert_eq!(out.reply, "I don't know.");
    assert_eq!(out.reply, UNKNOWN_REPLY);
    assert_eq!(out.sources, vec!["honest:unknown".to_string()]);
    assert!(out.evidence.is_empty());
    assert_ne!(out.arm, "answer");
    assert!(!out.reply.to_ascii_lowercase().contains("match"));
}

#[test]
fn comfort_keeps_the_honest_miss_and_does_not_script_a_feeling() {
    let mut brain = open_brain();
    let out = brain.turn("I am so tired");
    assert_eq!(out.arm, "comfort");
    assert_eq!(out.reply, "I don't know.");
    assert_eq!(out.sources, vec!["honest:unknown".to_string()]);
    assert!(out.evidence.is_empty());
    assert!(out.mood_meta.e > 0.0);
    let lower = out.reply.to_ascii_lowercase();
    assert!(!lower.contains("tired"));
    assert!(!lower.contains("sorry"));
    assert!(!lower.contains("hear you"));
    assert_eq!(out.propensity, 0.5);
}

#[test]
fn a_vent_with_a_card_still_answers() {
    let mut brain = open_brain();
    let out = brain.turn("I am so tired where is the kettle");
    assert_eq!(out.arm, "answer");
    assert_eq!(out.reply, "The kettle is on the hob.");
    assert!(!out.reply.to_ascii_lowercase().contains("sorry"));
}

#[test]
fn offer_topic_points_at_a_topic_already_in_the_ring() {
    let mut brain = open_brain();
    let nowhere = brain.turn("something else");
    assert_eq!(nowhere.arm, "ask");
    assert_eq!(nowhere.reply, UNKNOWN_REPLY);
    assert!(!nowhere.reply.to_ascii_lowercase().contains("interview"));

    let mut brain = open_brain();
    let _ = brain.turn("where is the kettle");
    let out = brain.turn("something else");
    assert_eq!(out.arm, "offer_topic");
    assert_eq!(out.reply, "Back to kettle?");
    assert_eq!(out.sources, vec!["topic:kettle".to_string()]);
    assert_eq!(out.propensity, 0.6);
    assert!(!out.reply.to_ascii_lowercase().contains("interview"));
}

#[test]
fn stop_uses_the_persona_goodbye_and_does_not_keep_talking() {
    let mut brain = open_brain();
    let out = brain.turn("goodbye");
    assert_eq!(out.arm, "stop");
    assert_eq!(out.reply, "Talk soon.");
    assert_eq!(out.sources, vec!["persona:talk-soon".to_string()]);
    assert_eq!(out.propensity, 0.9);
    assert!(!out.reply.contains('?'));

    let quiet = brain.turn("please stop");
    assert_eq!(quiet.arm, "stop");
    assert_eq!(quiet.reply, "Talk soon.");
}

#[test]
fn a_bus_stop_is_not_the_stop_move() {
    let mut brain = open_brain();
    let out = brain.turn("where is the bus stop");
    assert_eq!(out.arm, "ask");
    assert_eq!(out.reply, UNKNOWN_REPLY);
}

#[test]
fn consent_denied_stores_no_callback_and_writes_no_count() {
    let dir = std::env::temp_dir().join(format!(
        "horbm-dialogue-deny-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let before = names(&dir);

    let mut brain = open_brain();
    assert!(!brain.dialogue_consent().remember);
    assert!(!brain.dialogue_consent().learn);
    assert!(!brain.check_ins());
    assert!(!brain.note_callback("how did the interview go?"));
    assert!(brain.callbacks().is_empty());
    brain.set_bandit_enabled(true);
    let out = brain.turn("hello");
    assert_eq!(out.arm, "ask");
    assert_eq!(out.reply, "I don't know.");
    assert!(!out.reply.to_ascii_lowercase().contains("interview"));
    assert_eq!(brain.bandit_updates(), 0);
    assert_eq!(brain.bandit_counts(), [0; 6]);
    assert_eq!(names(&dir), before);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn remember_without_check_ins_keeps_the_callback_unspoken() {
    let mut brain = open_brain();
    brain.set_dialogue_consent(DialogueConsent {
        remember: true,
        learn: false,
        check_ins: false,
    });
    assert!(brain.note_callback("how did the interview go?"));
    assert_eq!(
        brain.callbacks(),
        &["how did the interview go?".to_string()]
    );
    assert!(!brain.check_ins());
    let out = brain.turn("hello");
    assert_eq!(out.reply, "I don't know.");
    assert_eq!(out.arm, "ask");
    assert!(!out.reply.to_ascii_lowercase().contains("interview"));
}

#[test]
fn a_scripted_comfort_line_is_not_stored_as_a_callback() {
    let mut brain = open_brain();
    brain.set_dialogue_consent(DialogueConsent {
        remember: true,
        learn: false,
        check_ins: true,
    });
    assert!(!brain.note_callback("I hear you."));
    assert!(brain.callbacks().is_empty());
    let out = brain.turn("hello");
    assert_eq!(out.reply, UNKNOWN_REPLY);
}

#[test]
fn check_ins_are_off_until_the_person_turns_them_on() {
    let mut brain = open_brain();
    brain.set_dialogue_consent(DialogueConsent {
        remember: true,
        learn: false,
        check_ins: false,
    });
    assert!(brain.note_callback("how did the interview go?"));
    let enabled = brain.turn("check ins on");
    assert!(brain.check_ins());
    assert_eq!(enabled.arm, "ask");
    assert_eq!(enabled.reply, UNKNOWN_REPLY);
    assert!(!enabled.reply.to_ascii_lowercase().contains("interview"));

    let out = brain.turn("hello");
    assert_eq!(out.arm, "offer_topic");
    assert_eq!(out.reply, "how did the interview go?");
    assert_eq!(out.sources, vec!["memory:callback".to_string()]);

    let off = brain.turn("check ins off");
    assert!(!brain.check_ins());
    assert_ne!(off.reply, "how did the interview go?");
    let hello = brain.turn("hello");
    assert_eq!(hello.reply, UNKNOWN_REPLY);
    assert_eq!(hello.arm, "ask");
}

#[test]
fn withdrawing_remember_drops_the_callback() {
    let mut brain = open_brain();
    brain.set_dialogue_consent(DialogueConsent {
        remember: true,
        learn: false,
        check_ins: true,
    });
    assert!(brain.note_callback("how did the interview go?"));
    brain.set_dialogue_consent(DialogueConsent {
        remember: false,
        learn: false,
        check_ins: true,
    });
    assert!(brain.callbacks().is_empty());
    let out = brain.turn("hello");
    assert_eq!(out.reply, UNKNOWN_REPLY);
    assert!(!out.reply.to_ascii_lowercase().contains("interview"));
}

#[test]
fn the_bandit_counts_only_when_it_is_on_and_learn_is_granted() {
    let mut denied = open_brain();
    denied.set_bandit_enabled(true);
    denied.set_dialogue_consent(DialogueConsent::from_flags(Some("yes"), Some("true")));
    let _ = denied.turn("where is the kettle");
    assert_eq!(denied.bandit_updates(), 0);

    let mut off = open_brain();
    off.set_dialogue_consent(DialogueConsent::from_flags(None, Some("grant")));
    let _ = off.turn("where is the kettle");
    assert!(off.dialogue_consent().learn);
    assert_eq!(off.bandit_updates(), 0);

    let mut granted = open_brain();
    granted.set_bandit_enabled(true);
    granted.set_dialogue_consent(DialogueConsent::from_flags(Some("grant"), Some(" grant ")));
    let first = granted.turn("where is the kettle");
    let second = granted.turn("where is the kettle");
    assert_eq!(first.arm, "answer");
    assert_eq!(second.arm, first.arm);
    assert_eq!(second.reply, first.reply);
    assert_eq!(granted.bandit_updates(), 2);
    assert_eq!(granted.bandit_counts()[horbm::Move::Answer.index()], 2);
}

#[test]
fn only_grant_opens_remember_or_learn_and_check_ins_stay_off() {
    assert!(!flag_granted(None));
    assert!(!flag_granted(Some("yes")));
    assert!(!flag_granted(Some("true")));
    assert!(!flag_granted(Some("")));
    assert!(flag_granted(Some("grant")));
    assert!(flag_granted(Some(" grant ")));
    let consent = DialogueConsent::from_flags(Some("grant"), Some("yes"));
    assert!(consent.remember);
    assert!(!consent.learn);
    assert!(!consent.check_ins);
    assert!(!DialogueConsent::default().check_ins);
}

#[test]
fn the_same_lines_produce_the_same_move_and_the_same_words() {
    let once = script();
    let twice = script();
    assert_eq!(once, twice);
    let arms: Vec<&str> = once.iter().map(|row| row.1.as_str()).collect();
    assert_eq!(
        arms,
        vec!["answer", "comfort", "offer_topic", "cite", "stop", "ask"]
    );
}

fn script() -> Vec<Spoken> {
    let mut brain = open_brain();
    brain.note_name("Maciej", PronounClass::He);
    let lines = [
        "where is the kettle",
        "I am so tired",
        "something else",
        "cite that",
        "goodbye",
        "who won the match",
    ];
    lines
        .into_iter()
        .map(|line| fields(&brain.turn(line)))
        .collect()
}

#[test]
fn the_ring_keeps_mood_and_the_focus_name_and_drops_the_oldest_at_32() {
    let mut brain = open_brain();
    brain.note_name("Maciej", PronounClass::He);
    let out = brain.turn("where is the kettle");
    let snap = brain.ring_get(0).expect("first turn");
    assert_eq!(snap.name.as_deref(), Some("Maciej"));
    assert_eq!(snap.arousal, out.mood.arousal);
    assert_eq!(snap.valence, out.mood.valence);
    assert_eq!(snap.dominance, out.mood.dominance);
    assert_eq!(snap.e, out.mood_meta.e);
    assert_eq!(snap.chosen, horbm::Move::Answer);
    for i in 0..32 {
        let _ = brain.turn(&format!("who won match {i}"));
    }
    assert_eq!(brain.ring_len(), 32);
    assert!(brain
        .ring_get(0)
        .and_then(|snap| snap.topic.as_deref())
        .is_some_and(|topic| topic != "kettle"));
    assert!(brain.ring_get(31).is_some());
    assert_eq!(
        brain.ring_get(31).and_then(|snap| snap.name.as_deref()),
        Some("Maciej")
    );
}

fn names(dir: &std::path::Path) -> Vec<String> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        found.push(entry.file_name().to_string_lossy().into_owned());
    }
    found.sort();
    found
}
