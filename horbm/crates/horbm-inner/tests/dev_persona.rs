// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Standalone inner goals. No audio, no user store, no named persona.

use std::time::Instant;

use galion_emotion::consent_bits::{ADAPT, REMEMBER, SENSE, SHARE};
use galion_emotion::persona_mood::configured;
use galion_emotion_core::{EmotionCore, IntentMsg, IntentSource, AROUSAL, DOMINANCE, VALENCE};
use horbm_inner::{
    remember_denied, Heard, InnerMsg, MsgDir, Perceived, PersonaInner, Verb, CARE_HOME, CARE_SAD,
    CHECKIN_GAP_MS, CHECKIN_MS, CURIOSITY_HOME, DECAY_KEEP_DEN, DECAY_KEEP_NUM, EAR_CONFIDENCE,
    EXPECT_HOME, REMEMBER_LOCK, TICK_MS, WAIT_MS,
};

fn seen(words: &str, ear: Option<Heard>, sense: bool) -> Perceived<'_> {
    Perceived {
        t_ms: 0,
        words,
        ear,
        sense,
    }
}

fn sad_ear() -> Heard {
    Heard {
        valence: -0.55,
        arousal: 0.28,
        confidence: 0.9,
    }
}

#[test]
fn standalone_home_matches_the_old_rest_and_names_nobody() {
    assert_eq!(REMEMBER_LOCK, "deny");
    assert!(remember_denied());
    let rest = std::hint::black_box(development_profile().dynamics.rest.valence);
    assert!(rest > 0.0, "the rest valence is warm");
    assert!((CARE_HOME - (0.30 + rest)).abs() < 1.0e-6);
    let half_life = std::hint::black_box(development_profile().dynamics.half_life_ms);
    assert!((half_life - 20_000.0).abs() < 1.0);
    let ideal = 1.0 - std::f32::consts::LN_2 * (TICK_MS as f32) / half_life;
    let keep = DECAY_KEEP_NUM as f32 / DECAY_KEEP_DEN as f32;
    assert!(
        (keep - ideal).abs() < 5.0e-4,
        "decay keep {keep} vs half-life {ideal}"
    );
    assert_eq!(TICK_MS, 80);
    let life = PersonaInner::new();
    assert_eq!(life.persona_id(), "");
    assert!(!life.keeps_user_facts());
    assert_eq!(life.care().to_bits(), CARE_HOME.to_bits());
    assert_eq!(life.curiosity().to_bits(), CURIOSITY_HOME.to_bits());
    assert_eq!(life.expect().to_bits(), EXPECT_HOME.to_bits());
}

#[test]
fn a_sad_listener_raises_care_and_warms_the_moment() {
    let mut life = PersonaInner::new();
    let msg = life.turn(seen("I feel really sad", None, false));
    assert!(
        life.care() > CARE_HOME + 0.1,
        "care {} home {CARE_HOME}",
        life.care()
    );
    assert!(life.care() < CARE_SAD);
    assert!(life.help() > horbm_inner::HELP_HOME);
    assert_eq!(msg.verb, Verb::Comfort);
    assert!(msg.valence > 0.0, "comfort pull should be warmer");
    assert!(msg.arousal < 0.0, "comfort pull should be softer");
    assert!(msg.dominance < 0.0);
    assert!(msg.strength > 0.6);
    assert!(!msg.sense);

    let gemi = msg.to_gemi().expect("gemi");
    assert_eq!(&gemi.encode().expect("bytes")[0..4], b"GEMI");
    assert_eq!(gemi.encode().expect("bytes").len(), 72);
    assert_eq!(gemi.verb_str(), "comfort");
    assert_eq!(gemi.source, IntentSource::Own);
    assert_eq!(gemi.factor_mask, 0);
    assert_eq!(gemi.consent & (REMEMBER | SHARE | ADAPT), 0);
    assert_eq!(gemi.consent, 0);

    let mut core = EmotionCore::at_rest();
    let mut hold = EmotionCore::at_rest();
    let bits = core.baseline().identity_bits();
    let pitch = core.baseline().pitch_centre_hz.to_bits();
    for i in 0..24 {
        let stepped = msg.to_gemi_at(i * TICK_MS).expect("stamp");
        core.step(&stepped).expect("step");
        hold.step(&IntentMsg::hold(i * TICK_MS)).expect("hold");
    }
    let warm = core.moment().pos[VALENCE] - hold.moment().pos[VALENCE];
    let soft = hold.moment().pos[AROUSAL] - core.moment().pos[AROUSAL];
    eprintln!(
        "comfort vs hold  valence {warm:+.4}  arousal softer {soft:+.4}  dominance {:+.4}",
        hold.moment().pos[DOMINANCE] - core.moment().pos[DOMINANCE]
    );
    assert!(warm > 0.02, "warmer by {warm}");
    assert!(soft > 0.02, "softer by {soft}");
    assert!(core.moment().pos[DOMINANCE] < hold.moment().pos[DOMINANCE]);
    assert_eq!(core.baseline().identity_bits(), bits);
    assert_eq!(core.baseline().pitch_centre_hz.to_bits(), pitch);
    assert!(core.persisted_baseline().is_none());
    assert!(!core.inbound_mixed());
    let dumped = format!("{life:?}");
    assert!(!dumped.to_ascii_lowercase().contains("sad"));
}

#[test]
fn a_question_raises_expect_and_the_answer_intent() {
    let mut life = PersonaInner::new();
    let msg = life.turn(seen("where did you go?", None, false));
    assert!(
        life.expect() > EXPECT_HOME + 0.2,
        "expect {}",
        life.expect()
    );
    assert_eq!(msg.verb, Verb::Answer);
    assert_eq!(msg.to_gemi().expect("gemi").verb_str(), "answer");
    assert_eq!(life.care().to_bits(), CARE_HOME.to_bits());
    assert!(!msg.lean().hold_topic);
    assert!((msg.lean().rate - 1.0).abs() < 1.0e-6);
}

#[test]
fn silence_waits_then_checks_in_once() {
    let mut life = PersonaInner::new();
    let mut saw_wait = false;
    let mut checkins = 0u32;
    let frames = CHECKIN_MS / TICK_MS;
    for index in 0..frames {
        let msg = life.tick(TICK_MS, true);
        match msg.verb {
            Verb::Hold => {
                assert!(!saw_wait);
                assert!(life.quiet_ms() < WAIT_MS);
            }
            Verb::Wait => {
                saw_wait = true;
                assert!(msg.lean().hold_topic);
                assert_eq!(checkins, 0);
            }
            Verb::CheckIn => {
                checkins += 1;
                assert!(saw_wait, "check-in arrived before wait");
                assert_eq!(index + 1, frames);
                assert!(msg.lean().hold_topic);
                assert!(msg.strength < 0.5, "check-in is gentle");
                assert!(msg.strength < 0.7);
                assert!(msg.valence.abs() < 0.28);
            }
            other => panic!("silence spoke {other:?}"),
        }
    }
    assert!(saw_wait);
    assert_eq!(checkins, 1);

    for _ in 0..(CHECKIN_GAP_MS / TICK_MS) {
        let msg = life.tick(TICK_MS, true);
        assert_eq!(msg.verb, Verb::Wait);
    }
    assert_eq!(checkins, 1, "one check-in per quiet stretch");

    // She speaks, so the stretch ends. The gap has not elapsed yet: do that
    // on a fresh clock by ticking only a little after the check-in.
    let mut life = PersonaInner::new();
    for _ in 0..(CHECKIN_MS / TICK_MS) {
        let _ = life.tick(TICK_MS, true);
    }
    assert_eq!(life.last().verb, Verb::CheckIn);
    let at = life.clock_ms();
    let _ = life.tick(TICK_MS, false);
    assert_eq!(life.quiet_ms(), 0);
    let mut second = 0u32;
    loop {
        let msg = life.tick(TICK_MS, true);
        if msg.verb == Verb::CheckIn {
            second += 1;
            assert!(
                life.clock_ms().saturating_sub(at) >= CHECKIN_GAP_MS,
                "second check-in at {} after {at}",
                life.clock_ms()
            );
            break;
        }
        if life.clock_ms() > at + CHECKIN_GAP_MS + CHECKIN_MS + 1_000 {
            panic!("no second check-in after the gap");
        }
    }
    assert_eq!(second, 1);
    let msg = life.tick(TICK_MS, true);
    assert_eq!(msg.verb, Verb::Wait);
}

#[test]
fn a_low_confidence_ear_is_no_claim() {
    let shy = 0.34f32;
    assert!(shy < EAR_CONFIDENCE);

    assert!(Heard::from_axes(-0.55, 0.28, shy).is_none());
    assert!(Heard::from_axes(-0.55, 0.28, 0.0).is_none());
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert!(Heard::from_axes(bad, 0.28, 0.9).is_none());
        assert!(Heard::from_axes(-0.55, bad, 0.9).is_none());
        assert!(Heard::from_axes(-0.55, 0.28, bad).is_none());
    }

    let claimed = Heard::from_axes(-0.55, 0.28, EAR_CONFIDENCE).expect("floor is a claim");
    assert_eq!(claimed.valence.to_bits(), (-0.55f32).to_bits());
    assert_eq!(claimed.arousal.to_bits(), 0.28f32.to_bits());
    assert_eq!(claimed.confidence.to_bits(), EAR_CONFIDENCE.to_bits());

    let mut from_words = PersonaInner::new();
    let heard = from_words.turn(seen("I feel sad", Heard::from_axes(0.8, 0.8, shy), true));
    assert_eq!(heard.verb, Verb::Comfort);
    assert!(from_words.care() > CARE_HOME + 0.1);

    let mut quiet = PersonaInner::new();
    let ignored = quiet.turn(seen("", Heard::from_axes(-0.55, 0.28, shy), true));
    assert_ne!(ignored.verb, Verb::Comfort);
    assert_eq!(quiet.care().to_bits(), CARE_HOME.to_bits());
    assert_eq!(quiet.turns(), 0);

    let mut sensed = PersonaInner::new();
    let ear_only = sensed.turn(seen(
        "",
        Heard::from_axes(-0.55, 0.28, EAR_CONFIDENCE),
        true,
    ));
    assert_eq!(ear_only.verb, Verb::Comfort);
    assert!(sensed.care() > CARE_HOME);
}

#[test]
fn ear_off_ignores_the_listener_and_still_reads_the_words() {
    let mut from_ear = PersonaInner::new();
    let ignored = from_ear.turn(seen("", Some(sad_ear()), false));
    assert_ne!(ignored.verb, Verb::Comfort);
    assert_eq!(from_ear.care().to_bits(), CARE_HOME.to_bits());

    let mut from_words = PersonaInner::new();
    let heard = from_words.turn(seen(
        "I feel sad",
        Some(Heard {
            valence: 0.8,
            arousal: 0.8,
            confidence: 1.0,
        }),
        false,
    ));
    assert_eq!(heard.verb, Verb::Comfort);
    assert!(from_words.care() > CARE_HOME + 0.1);

    let mut sensed = PersonaInner::new();
    let ear_only = sensed.turn(seen("", Some(sad_ear()), true));
    assert_eq!(ear_only.verb, Verb::Comfort);
    assert!(sensed.care() > CARE_HOME);
    assert!(ear_only.sense);
    assert_eq!(ear_only.to_gemi().expect("gemi").consent, SENSE);
}

#[test]
fn an_inbound_heard_note_is_the_same_gemi_record() {
    let mut inbound = IntentMsg::hold(80);
    inbound.source = IntentSource::Inbound;
    inbound.set_verb("heard").unwrap();
    inbound.strength = 0.9;
    inbound.set_pull(VALENCE, -0.5).unwrap();
    inbound.set_pull(AROUSAL, 0.3).unwrap();
    inbound.consent = 0;
    let parsed = InnerMsg::from_gemi(&inbound);
    assert_eq!(parsed.dir, MsgDir::In);
    assert_eq!(parsed.verb, Verb::Heard);
    assert_eq!(parsed.care, 0.0, "goals do not ride the 72-byte record");

    let mut life = PersonaInner::new();
    let answer = life.hear(&inbound, "where did you go?");
    assert_eq!(answer.verb, Verb::Answer);
    assert_eq!(life.care().to_bits(), CARE_HOME.to_bits());
    assert!(life.expect() > EXPECT_HOME);

    inbound.consent = SENSE;
    let mut life = PersonaInner::new();
    let comfort = life.hear(&inbound, "");
    assert_eq!(comfort.verb, Verb::Comfort);
    assert!(life.care() > CARE_HOME);

    let wire = comfort.to_gemi().expect("out");
    assert_eq!(wire.factor_mask, 0);
    assert_eq!(wire.consent & REMEMBER, 0);
    let back = InnerMsg::from_gemi(&wire);
    assert_eq!(back.verb, Verb::Comfort);
    assert_eq!(back.dir, MsgDir::Out);
}

#[test]
fn the_note_is_json_and_has_no_transcript_field() {
    let mut life = PersonaInner::new();
    let _ = life.turn(seen("my password is secret and I feel sad", None, false));
    let msg = life.last();
    let json = msg.to_json().expect("json");
    assert!(!json.to_ascii_lowercase().contains("secret"));
    assert!(!json.to_ascii_lowercase().contains("password"));
    assert!(!json.contains("words"));
    assert!(!json.contains("user"));
    let back = InnerMsg::from_json(&json).expect("parse");
    assert_eq!(back, msg);
    assert_eq!(back.verb, Verb::Comfort);

    let mut with_user = json.clone();
    assert!(with_user.ends_with('}'));
    with_user.pop();
    with_user.push_str(",\"user\":\"secret\"}");
    assert!(InnerMsg::from_json(&with_user).is_err());

    let dumped = format!("{life:?}");
    assert!(!dumped.to_ascii_lowercase().contains("secret"));
    assert!(!dumped.to_ascii_lowercase().contains("password"));
}

#[test]
fn curiosity_asks_and_a_greeting_stays_with_them() {
    let mut life = PersonaInner::new();
    let ask = life.turn(seen("I wonder about the river", None, false));
    assert_eq!(ask.verb, Verb::Ask);
    assert!(life.curiosity() > CURIOSITY_HOME);
    assert_eq!(ask.to_gemi().expect("gemi").verb_str(), "ask");

    let mut life = PersonaInner::new();
    let with = life.turn(seen("hello", None, false));
    assert_eq!(with.verb, Verb::With);
    assert_eq!(with.to_gemi().expect("gemi").verb_str(), "with");
    assert_eq!(life.care().to_bits(), CARE_HOME.to_bits());
    assert!((with.lean().rate - 1.0).abs() < 1.0e-6);
    assert!((with.lean().pause_ms - 40.0).abs() < 1.0e-6);
}

#[test]
fn goals_decay_toward_home_and_do_not_fall_through_it() {
    let mut life = PersonaInner::new();
    let _ = life.turn(seen("I feel really sad", None, false));
    let peak = life.care();
    for _ in 0..250 {
        let _ = life.tick(TICK_MS, false);
    }
    assert!(life.care() < peak - 0.05, "peak {peak} now {}", life.care());
    assert!(
        life.care() > CARE_HOME,
        "decayed through home: {}",
        life.care()
    );
    assert_eq!(life.last().verb, Verb::Hold);
}

#[test]
fn a_refused_step_does_not_commit_the_turn() {
    let life = PersonaInner::new();
    let (next, msg) = life.preview(seen("I feel really sad", None, false));
    let mut core = EmotionCore::at_rest();
    let err = core
        .step(&msg.to_gemi_at(40).expect("stamp"))
        .expect_err("off clock");
    assert_eq!(
        err,
        galion_emotion_core::CoreError::OffClock {
            expected_ms: 0,
            got_ms: 40
        }
    );
    assert_eq!(life.care().to_bits(), CARE_HOME.to_bits());
    assert!(next.care() > life.care());
    assert_eq!(core.frames(), 0);
    assert_eq!(
        core.baseline().identity_bits(),
        EmotionCore::at_rest().baseline().identity_bits()
    );

    let mut life = PersonaInner::new();
    let mut core = EmotionCore::at_rest();
    let note = life
        .nudge(&mut core, seen("I feel really sad", None, false))
        .expect("on clock");
    assert_eq!(note.verb, Verb::Comfort);
    assert!(life.care() > CARE_HOME);
    assert_eq!(core.frames(), 1);
}

#[test]
fn one_tick_is_cheap() {
    let mut life = PersonaInner::new();
    for _ in 0..32 {
        black_box_tick(&mut life);
    }
    let n = 4_000u32;
    let started = Instant::now();
    for _ in 0..n {
        black_box_tick(&mut life);
    }
    let us = started.elapsed().as_secs_f64() * 1.0e6;
    let per = us / f64::from(n);
    eprintln!("inner tick mean {per:.3} us over {n} (this profile)");
    assert!(per < 100.0, "tick {per:.3} us");
}

fn black_box_tick(life: &mut PersonaInner) {
    let msg = life.tick(TICK_MS, true);
    std::hint::black_box(msg);
}

fn development_profile() -> &'static galion_emotion::persona_mood::PersonaProfile {
    static ID: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    let id = ID.get_or_init(|| {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../horbm-persona/development.toml");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()));
        text.lines()
            .find_map(|line| {
                let line = line.trim();
                let rest = line.strip_prefix("id = ")?;
                let rest = rest.trim().trim_matches('"');
                if rest.is_empty() {
                    None
                } else {
                    Some(rest.to_string())
                }
            })
            .expect("development id")
    });
    configured(Some(id)).expect("development profile")
}

fn persona_case_needles() -> Vec<String> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../horbm-persona/personas");
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).unwrap_or_else(|err| panic!("{}: {err}", dir.display())) {
        let path = entry.expect("persona entry").path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        out.push(stem.to_string());
        out.push(stem.to_uppercase());
        let mut chars = stem.chars();
        if let Some(first) = chars.next() {
            out.push(format!("{}{}", first.to_uppercase(), chars.as_str()));
        }
    }
    assert!(!out.is_empty());
    out
}

#[test]
fn sources_do_not_name_another_persona_or_a_runtime() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for name in ["lib.rs", "life.rs", "msg.rs"] {
        let text = std::fs::read_to_string(root.join(name)).expect(name);
        for needle in persona_case_needles() {
            assert!(!text.contains(&needle), "{name} contains {needle}");
        }
        for needle in [
            "Jarvis",
            "ort::",
            "onnxruntime",
            "GALION_HORBM_RESEARCH_DIR",
            "cfg(unix",
            "cfg(windows",
            "cfg(target",
            "std::fs",
            "std::net",
            ".wav",
            "include_bytes!",
        ] {
            assert!(!text.contains(needle), "{name} contains {needle}");
        }
    }
}
