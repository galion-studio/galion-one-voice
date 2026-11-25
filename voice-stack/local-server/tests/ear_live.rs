// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Mic prosody on the live pipeline. PCM is generated here. No audio file.
//!
//! The shipped example card (`knowledge/kettle.md`) says
//! "The kettle is on the stove." The hob sentence is the in-memory fixture
//! in the `horbm` crate, not the clause this host speaks.
//!
//! The f32 frame in the pipeline is one array filled in place. This crate
//! has no allocation counter, so there is no `no_allocation_per_frame_after_setup`
//! test. A short tail under 1280 samples stays in a fixed carry array.

use samantha_local_server::audio::samples_per_frame;
use samantha_local_server::reply::{card_turn, EarLive, TurnOut};

const CARD: &str = "The kettle is on the stove.";
const ASK: &str = "where is the kettle";

fn tone(rate: u32, secs: f32, hz: f32) -> Vec<i16> {
    let n = (rate as f32 * secs) as usize;
    let mut out = Vec::with_capacity(n);
    let mut i = 0;
    while i < n {
        let t = i as f32 / rate as f32;
        let env = 0.65 + 0.35 * (std::f32::consts::TAU * 3.0 * t).sin();
        let sample = (std::f32::consts::TAU * hz * t).sin() * env * 0.25;
        out.push((sample * 32767.0).round().clamp(-32768.0, 32767.0) as i16);
        i += 1;
    }
    out
}

fn push_utterance(live: &mut EarLive, rate: u32) -> samantha_local_server::TurnProduct {
    let mut pcm = tone(rate, 1.0, 180.0);
    let frame = samples_per_frame(rate, 20);
    pcm.extend(std::iter::repeat(0).take(frame.saturating_mul(32)));
    live.push_mic(&pcm, rate, 0)
}

fn same_clause(got: &TurnOut, expect: &TurnOut) {
    assert_eq!(got.reply, expect.reply);
    assert_eq!(got.sources, expect.sources);
    assert_eq!(got.first_clause, expect.first_clause);
    assert_eq!(got.rest, expect.rest);
}

fn same_text_turn(got: &TurnOut, expect: &TurnOut) {
    same_clause(got, expect);
    assert_eq!(got.intent_json, expect.intent_json);
    assert_eq!(got.arm_json, expect.arm_json);
    assert_eq!(got.evidence, expect.evidence);
    assert_eq!(got.violations, expect.violations);
    assert_eq!(got.mood, expect.mood);
    assert_eq!(got.empathy_e.to_bits(), expect.empathy_e.to_bits());
    assert_eq!(got.prosody_rate.to_bits(), expect.prosody_rate.to_bits());
    assert_eq!(
        got.prosody_energy.to_bits(),
        expect.prosody_energy.to_bits()
    );
    assert_eq!(
        got.prosody_pitch_st.to_bits(),
        expect.prosody_pitch_st.to_bits()
    );
    assert_eq!(
        got.prosody_pause_ms.to_bits(),
        expect.prosody_pause_ms.to_bits()
    );
    assert_eq!(got.propensity.to_bits(), expect.propensity.to_bits());
}

#[test]
fn a_voiced_utterance_reaches_the_brain_as_a_heard_point() {
    let mut live = EarLive::scripted(ASK, true);
    assert_eq!(live.ear_line(), "ear=on");
    let out = push_utterance(&mut live, 16_000);
    assert_eq!(out.reply, CARD);
    let point = live
        .last_point()
        .expect("voiced frames keep a listener point");
    assert!(point.0.is_finite() && point.1.is_finite() && point.2.is_finite());
    assert!(point.2 >= 0.35, "confidence {}", point.2);
    assert_eq!(live.points_passed(), 1);
    assert_eq!(live.turns_without_point(), 0);
    let got = live.last_turn().expect("turn");
    let reference = card_turn(ASK).expect("card");
    same_clause(&got, &reference);
    assert!(got.sources.iter().any(|source| source == "card:kettle"));
}

#[test]
fn ear_off_switch_passes_no_point() {
    let mut live = EarLive::scripted(ASK, false);
    assert_eq!(live.ear_line(), "ear=off");
    let out = push_utterance(&mut live, 16_000);
    assert_eq!(out.reply, CARD);
    assert!(live.last_point().is_none());
    assert_eq!(live.points_passed(), 0);
    assert_eq!(live.turns_without_point(), 1);
    let got = live.last_turn().expect("turn");
    assert_eq!(got.reply, CARD);
    assert!(got.sources.iter().any(|source| source == "card:kettle"));
}

#[test]
fn typed_text_turn_is_byte_identical() {
    let mut live = EarLive::scripted("unused", true);
    let out = live.push_text(ASK, 0);
    assert_eq!(out.reply, CARD);
    assert_eq!(live.points_passed(), 0);
    assert_eq!(live.turns_without_point(), 1);
    assert!(live.last_point().is_none());
    let got = live.last_turn().expect("turn");
    let reference = card_turn(ASK).expect("card");
    same_text_turn(&got, &reference);
}

#[test]
fn input_24k_is_downsampled_before_the_ear() {
    let mut live = EarLive::scripted(ASK, true);
    let out = push_utterance(&mut live, 24_000);
    assert_eq!(out.reply, CARD);
    let point = live
        .last_point()
        .expect("24 kHz tone downsamples into a point");
    assert!(point.0.is_finite() && point.1.is_finite() && point.2.is_finite());
    assert_eq!(live.points_passed(), 1);
    let got = live.last_turn().expect("turn");
    let reference = card_turn(ASK).expect("card");
    same_clause(&got, &reference);
}
