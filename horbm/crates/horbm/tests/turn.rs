// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Full turn: mock cards, mock gateway, empathy mood on the reply.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use horbm::{
    is_deep_question, is_money_action, AskOutcome, Brain, CardHit, CardLookup, DialogueConsent,
    GapReason, Persona, ResearchAsk, ResearchFollowUp, RESEARCH_CAP, UNKNOWN_REPLY,
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
    caps: Arc<Mutex<Vec<Duration>>>,
    sleep_cap: bool,
    answer: AskOutcome,
}

impl ResearchAsk for Gateway {
    fn ask(&mut self, _question: &str, cap: Duration) -> AskOutcome {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.caps.lock().expect("caps").push(cap);
        if self.sleep_cap {
            thread::sleep(cap);
        }
        self.answer.clone()
    }
}

#[test]
fn miss_is_exact_and_mood_passes_through_a_card() {
    let calls = Arc::new(AtomicU32::new(0));
    let mut brain = Brain::with_cards(
        Cards,
        Some(Box::new(Gateway {
            calls: Arc::clone(&calls),
            caps: Arc::new(Mutex::new(Vec::new())),
            sleep_cap: false,
            answer: AskOutcome::Answer {
                text: "A filler line.".into(),
                source: "local".into(),
            },
        })),
    );
    let miss = brain.turn("I am so tired");
    assert_eq!(miss.reply, UNKNOWN_REPLY);
    assert_eq!(miss.reply, "I don't know.");
    assert_eq!(miss.sources, vec!["honest:unknown".to_string()]);
    assert!(miss.evidence.is_empty());
    assert!(miss.mood_meta.e > 0.0);
    assert!(miss.mood.valence > 0.0);
    assert_eq!(miss.mood.arousal, miss.mood_meta.arousal);
    assert_eq!(miss.mood.valence, miss.mood_meta.valence);
    assert_eq!(miss.mood.dominance, miss.mood_meta.dominance);
    assert!(miss.mood_meta.named.is_none());
    assert!(!miss.reply.to_ascii_lowercase().contains("sorry"));
    assert!(!miss.reply.to_ascii_lowercase().contains("filler"));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(brain.gaps()[0].reason, GapReason::NoCard);
    assert_eq!(brain.gaps()[0].advice, "ask_galion_research");

    let hit = brain.turn("I am so tired where is the kettle");
    assert_eq!(hit.reply, "The kettle is on the hob.");
    assert_eq!(hit.sources, vec!["card:kettle".to_string()]);
    assert!(hit.mood_meta.e > 0.0);
    assert!(hit.mood_meta.rate.is_finite());
    assert!(hit.mood_meta.energy.is_finite());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn gateway_timeout_at_three_seconds_falls_back() {
    let calls = Arc::new(AtomicU32::new(0));
    let caps = Arc::new(Mutex::new(Vec::new()));
    let mut brain = Brain::with_cards(
        Cards,
        Some(Box::new(Gateway {
            calls: Arc::clone(&calls),
            caps: Arc::clone(&caps),
            sleep_cap: false,
            answer: AskOutcome::Timeout,
        })),
    );
    let started = Instant::now();
    let out = brain.turn("why is the spare fuse missing from the drawer?");
    let elapsed = started.elapsed();
    eprintln!("MEASURE horbm_gateway_timeout_ms={}", elapsed.as_millis());
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "research ran inside the turn"
    );
    assert!(
        elapsed < Duration::from_millis(500),
        "the clause waited on research: {elapsed:?}"
    );
    assert_eq!(out.reply, "I don't know.");
    assert_eq!(out.first_clause, "I don't know.");
    assert_eq!(out.sources, vec!["honest:unknown".to_string()]);
    assert!(out.evidence.is_empty());
    assert!(!out.reply.to_ascii_lowercase().contains("ollama"));
    assert!(out.mood_meta.intent == "ask" || out.mood_meta.intent == "chat");
    assert_eq!(brain.gaps().last().unwrap().reason, GapReason::ResearchOff);
    drop(out);
    let follow = wait_follow(&mut brain);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(caps.lock().unwrap().as_slice(), &[RESEARCH_CAP]);
    assert_eq!(RESEARCH_CAP, Duration::from_secs(3));
    assert_eq!(follow.outcome, AskOutcome::Timeout);
    assert!(follow.spoken.is_none());
    assert_eq!(follow.gap.reason, GapReason::ResearchTimeout);
    assert_eq!(
        brain.gaps().last().unwrap().reason,
        GapReason::ResearchTimeout
    );
}

#[test]
fn a_payment_is_not_armed_and_a_question_mark_keeps_the_deep_rule() {
    let calls = Arc::new(AtomicU32::new(0));
    let caps = Arc::new(Mutex::new(Vec::new()));
    let mut brain = Brain::with_cards(
        Cards,
        Some(Box::new(Gateway {
            calls: Arc::clone(&calls),
            caps: Arc::clone(&caps),
            sleep_cap: false,
            answer: AskOutcome::Miss,
        })),
    );

    for line in [
        "send money to pat today",
        "wire money to the account",
        "transfer money now",
        "make a payment for the invoice",
    ] {
        assert!(
            is_money_action(line),
            "phrase rule fired: one of the four conductor phrases: {line}"
        );
        let out = brain.turn(line);
        assert_eq!(out.reply, UNKNOWN_REPLY, "{line}");
        assert_eq!(out.first_clause, UNKNOWN_REPLY, "{line}");
        let spoken = out.reply.to_ascii_lowercase();
        assert!(!spoken.contains("sent"), "payment confirmation: {line}");
        assert!(!spoken.contains("paid"), "payment confirmation: {line}");
        assert!(!spoken.contains("payment"), "payment confirmation: {line}");
        assert!(!spoken.contains("transfer"), "payment confirmation: {line}");
        if let Some(mark) = &out.sideband {
            assert!(!mark.posted, "a payment was posted: {line}");
        }
        assert_eq!(
            brain.gaps().last().unwrap().reason,
            GapReason::NoCard,
            "{line} was treated as a research question"
        );
        drop(out);
    }
    thread::sleep(Duration::from_millis(40));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(brain.research_follow_up().is_none());

    // Question-mark rule, same as `is_money_action` in
    // `crates/galion-conductor/src/research.rs`: a `?` skips the four phrases.
    // The deep-question rule then runs. `send money?` is two words and has
    // no why, how, or explain, so research stays off.
    assert!(
        !is_money_action("send money?"),
        "question-mark rule fired: `?` skips the phrase list"
    );
    assert!(
        !is_deep_question("send money?"),
        "deep-question rule fired: two words, no why/how/explain"
    );
    let questioned = brain.turn("send money?");
    assert_eq!(questioned.reply, UNKNOWN_REPLY);
    assert_eq!(questioned.first_clause, UNKNOWN_REPLY);
    assert!(!questioned.reply.to_ascii_lowercase().contains("sent"));
    if let Some(mark) = &questioned.sideband {
        assert!(!mark.posted);
    }
    assert_eq!(brain.gaps().last().unwrap().reason, GapReason::NoCard);
    drop(questioned);
    thread::sleep(Duration::from_millis(40));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(brain.research_follow_up().is_none());

    let deep = brain.turn("why is the spare fuse missing from the drawer?");
    assert_eq!(deep.reply, UNKNOWN_REPLY);
    assert_eq!(deep.first_clause, UNKNOWN_REPLY);
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "research ran inside the turn"
    );
    assert_eq!(brain.gaps().last().unwrap().reason, GapReason::ResearchOff);
    drop(deep);
    let follow = wait_follow(&mut brain);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(caps.lock().unwrap().as_slice(), &[RESEARCH_CAP]);
    assert_eq!(RESEARCH_CAP, Duration::from_secs(3));
    assert!(follow.spoken.is_none());
    assert_eq!(follow.gap.reason, GapReason::ResearchMiss);

    // The question-mark rule leaves a deep question armed, including one
    // that contains a payment phrase. The conductor classifies this line
    // as a lookup, not a payment order.
    assert!(
        !is_money_action("how do I send money safely?"),
        "question-mark rule fired: `?` skips the phrase list"
    );
    assert!(
        is_deep_question("how do I send money safely?"),
        "deep-question rule fired: the word how"
    );
    let lookup = brain.turn("how do I send money safely?");
    assert_eq!(lookup.reply, UNKNOWN_REPLY);
    assert_eq!(lookup.first_clause, UNKNOWN_REPLY);
    assert!(!lookup.reply.to_ascii_lowercase().contains("sent"));
    assert!(!lookup.reply.to_ascii_lowercase().contains("payment"));
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "research ran inside the turn"
    );
    drop(lookup);
    let follow = wait_follow(&mut brain);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(
        caps.lock().unwrap().as_slice(),
        &[RESEARCH_CAP, RESEARCH_CAP]
    );
    assert_eq!(follow.heard, "how do I send money safely?");
    assert!(follow.spoken.is_none());
    assert_eq!(follow.gap.reason, GapReason::ResearchMiss);
}

#[test]
fn text_only_kettle_clause_is_unchanged() {
    let mut brain = Brain::with_cards(Cards, None);
    let out = brain.turn("where is the kettle");
    assert_eq!(out.reply, "The kettle is on the hob.");
    assert_eq!(out.first_clause, "The kettle");
    let again = brain.turn_with_ear("where is the kettle", None);
    assert_eq!(again.reply, "The kettle is on the hob.");
    assert_eq!(again.first_clause, "The kettle");
}

#[test]
fn a_sad_ear_does_not_replace_a_card_clause() {
    let ear = horbm_inner::Heard::from_axes(
        horbm_inner::SAD_EAR_VALENCE,
        0.2,
        horbm_inner::EAR_CONFIDENCE,
    )
    .expect("the floor is a claim");
    assert!(ear.valence <= horbm_inner::SAD_EAR_VALENCE);
    assert!(ear.confidence >= horbm_inner::EAR_CONFIDENCE);
    let mut brain = Brain::with_cards(Cards, None);
    brain.set_ear_sense(true);
    assert!(brain.ear_sense());
    let out = brain.turn_with_ear("where is the kettle", Some(ear));
    assert_eq!(out.reply, "The kettle is on the hob.");
    assert_eq!(out.first_clause, "The kettle");
}

#[test]
fn ear_sense_from_flag_defaults_on() {
    assert!(Brain::ear_sense_from_flag(None));
    assert!(Brain::ear_sense_from_flag(Some("")));
    assert!(Brain::ear_sense_from_flag(Some("1")));
    assert!(Brain::ear_sense_from_flag(Some(" ON ")));
    assert!(Brain::ear_sense_from_flag(Some("allow")));
    assert!(!Brain::ear_sense_from_flag(Some("0")));
    assert!(!Brain::ear_sense_from_flag(Some("off")));
    assert!(!Brain::ear_sense_from_flag(Some("Deny")));
}

#[test]
fn text_turn_is_identical_with_ear_sense_on_and_off() {
    let mut on = Brain::with_cards(Cards, None);
    on.set_ear_sense(true);
    let mut off = Brain::with_cards(Cards, None);
    off.set_ear_sense(false);
    let heard = "where is the kettle";
    let with_ear = on.turn_with_ear(heard, None);
    let without = off.turn_with_ear(heard, None);
    assert_eq!(with_ear.reply, without.reply);
    assert_eq!(with_ear.first_clause, without.first_clause);
    assert_eq!(with_ear.reply, "The kettle is on the hob.");
    assert_eq!(with_ear.first_clause, "The kettle");
    same_mood(on.mood_meta(), off.mood_meta());
    same_mood(with_ear.mood_meta, without.mood_meta);
    same_mood(on.mood_meta(), with_ear.mood_meta);
}

fn same_mood(left: horbm::MoodMeta, right: horbm::MoodMeta) {
    assert_eq!(left.arousal.to_bits(), right.arousal.to_bits());
    assert_eq!(left.valence.to_bits(), right.valence.to_bits());
    assert_eq!(left.dominance.to_bits(), right.dominance.to_bits());
    assert_eq!(left.e.to_bits(), right.e.to_bits());
    assert_eq!(left.intent, right.intent);
    assert_eq!(left.rate.to_bits(), right.rate.to_bits(), "mood_meta.rate");
    assert_eq!(left.energy.to_bits(), right.energy.to_bits());
    assert_eq!(left.pitch_st.to_bits(), right.pitch_st.to_bits());
    assert_eq!(left.pause_ms.to_bits(), right.pause_ms.to_bits());
    assert_eq!(left.named, right.named);
}

#[test]
fn only_the_grant_word_opens_the_person_file_at_open() {
    for (index, flag) in [None, Some("yes"), Some("true")].into_iter().enumerate() {
        let dir = scratch(&format!("remember-deny-{index}"));
        let path = dir.join("person.sqlite");
        let mut brain =
            Brain::open_with_remember_flag(&path, &dir.join("knowledge"), &dir.join("cases"), flag);
        assert!(!path.exists(), "{flag:?}");
        let _ = brain.turn("hello");
        assert!(!path.exists(), "denied turn creates no file: {flag:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    let dir = scratch("remember-grant");
    let path = dir.join("person.sqlite");
    let mut brain = Brain::open_with_remember_flag(
        &path,
        &dir.join("knowledge"),
        &dir.join("cases"),
        Some("grant"),
    );
    assert!(!path.exists(), "the file waits for a write");
    let _ = brain.turn("hello");
    assert!(path.is_file(), "grant opens the person file on a turn");
    let _ = std::fs::remove_dir_all(&dir);
}

const NORTHWIND: &str = r#"
schema = "galion-horbm-persona/1"
version = 1
id = "northwind"
voice = "northwind"

[traits]
warmth = 0.61

[honesty]
never_fabricate = true
unknown = "I don't know."

[boundaries]
forbid = ["fake-sympathy", "unsourced-fact"]

[style]
brevity = 0.5
warmth = 0.62
max_sentences = 2

[baseline]
valence = 0.1
arousal = 0.31
dominance = 0.4
approach = 0.0
pitch_centre_hz = 216.0

[inner]
care = "heard"
want = "answer"
expect = "pause"
"#;

const LONG_CARD: &str = "The kettle is on the hob. The hob is hot. The lid is shut.";

struct LongCards;

impl CardLookup for LongCards {
    fn lookup(&self, query: &str) -> Option<CardHit> {
        if query.to_ascii_lowercase().contains("kettle") {
            Some(CardHit {
                id: "kettle".into(),
                gist: LONG_CARD.into(),
            })
        } else {
            None
        }
    }
}

/// A fixture persona with `max_sentences = 2`. An unpicked reply is cut.
/// A card clause is a picked answer and keeps every sentence.
#[test]
fn long_plain_reply_is_capped_and_card_text_is_not() {
    let persona = Persona::parse(NORTHWIND).expect("northwind persona");
    assert_eq!(persona.id(), "northwind");
    assert_eq!(persona.style().max_sentences, 2);
    let mut brain = Brain::with_persona(LongCards, None, persona);
    brain.set_dialogue_consent(DialogueConsent {
        remember: true,
        learn: false,
        check_ins: true,
    });
    assert!(brain.note_callback("Alpha stays. Bravo stays. Charlie goes."));

    let plain = brain.turn("hello");
    assert_eq!(plain.arm, "offer_topic");
    assert_eq!(plain.reply, "Alpha stays. Bravo stays.");
    assert!(!plain.reply.contains("Charlie"));
    assert_eq!(plain.sources, vec!["memory:callback".to_string()]);

    let missed = brain.turn("zzznomatchtoken");
    assert_eq!(missed.reply, UNKNOWN_REPLY);
    assert_eq!(missed.reply, "I don't know.");
    assert_eq!(missed.first_clause, UNKNOWN_REPLY);

    let card = brain.turn("where is the kettle");
    assert_eq!(card.reply, LONG_CARD);
    assert!(card.reply.contains("The lid is shut."));
    assert_eq!(card.sources, vec!["card:kettle".to_string()]);
    assert_eq!(card.first_clause, "The kettle");
}

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "horbm-turn-flag-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
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
