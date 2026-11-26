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
    // 40 × 20 ms = 800 ms. An ear-on hold closes at 720 ms inside this
    // buffer. Ear-off still closes at 640 ms and returns on that frame.
    pcm.extend(std::iter::repeat(0).take(frame.saturating_mul(40)));
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

/// Quiet frames after `speech`, until the take ends. Returns milliseconds of silence.
fn silence_until_end(label: &str, sense_on: bool, speech: &[i16]) -> u32 {
    let mut live = EarLive::scripted(ASK, sense_on);
    assert_eq!(
        live.ear_line(),
        if sense_on { "ear=on" } else { "ear=off" },
        "{label}"
    );
    let opened = live.push_mic(speech, 16_000, 0);
    assert!(
        opened
            .events
            .iter()
            .any(|event| event.contains("\"event\":\"start\"")),
        "{label} did not open the gate: {}",
        opened.events.join("\n")
    );
    assert!(
        opened
            .events
            .iter()
            .all(|event| !event.contains("\"event\":\"end\"")),
        "{label} ended during speech"
    );
    let frame = samples_per_frame(16_000, 20);
    let quiet = vec![0i16; frame];
    let mut silence_ms = 0u32;
    for _ in 0..80 {
        silence_ms += 20;
        let out = live.push_mic(&quiet, 16_000, 0);
        if out
            .events
            .iter()
            .any(|event| event.contains("\"event\":\"end\""))
        {
            assert_eq!(out.reply, CARD, "{label}");
            println!("{label} silence_ms={silence_ms}");
            return silence_ms;
        }
    }
    panic!("{label} did not end after {silence_ms} ms");
}

/// Four 80 ms sines, rising in pitch at a steady level. No audio file.
fn rising_contour() -> Vec<i16> {
    stepped_contour([(140.0, 0.22), (165.0, 0.22), (195.0, 0.22), (230.0, 0.22)])
}

/// Four 80 ms sines, falling in pitch and level. No audio file.
fn falling_contour() -> Vec<i16> {
    stepped_contour([(240.0, 0.30), (200.0, 0.24), (160.0, 0.18), (130.0, 0.12)])
}

fn stepped_contour(steps: [(f32, f32); 4]) -> Vec<i16> {
    let rate = 16_000u32;
    let n = rate as usize * 80 / 1000;
    let mut out = Vec::with_capacity(n * steps.len());
    for (hz, amp) in steps {
        let mut phase = 0.0f32;
        let step = std::f32::consts::TAU * hz / rate as f32;
        for _ in 0..n {
            let sample = phase.sin() * amp;
            out.push((sample * 32767.0).round().clamp(-32768.0, 32767.0) as i16);
            phase += step;
        }
    }
    out
}

#[test]
fn ear_off_ends_on_the_hang_as_today() {
    let speech = tone(16_000, 0.4, 180.0);
    let silence_ms = silence_until_end("ear_off", false, &speech);
    assert_eq!(silence_ms, 640, "pause-only hang is 640 ms");
}

/// `off` and `deny` are the same shut (`Ear::from_env`). `scripted(false)` is that shut.
#[test]
fn denied_ear_is_pause_only() {
    let speech = tone(16_000, 0.4, 180.0);
    let silence_ms = silence_until_end("denied_ear", false, &speech);
    assert_eq!(silence_ms, 640, "a denied ear stays on the 640 ms hang");
}

/// Flat tone. An empty partial holds, so the take waits out the hold pause
/// and still stops at the hang plus 80 ms.
#[test]
fn hold_never_exceeds_hang_plus_80() {
    let speech = tone(16_000, 0.5, 180.0);
    let silence_ms = silence_until_end("hold", true, &speech);
    assert!(silence_ms > 640, "hold silence_ms={silence_ms}");
    assert!(silence_ms <= 640 + 80, "hold silence_ms={silence_ms}");
    assert_eq!(silence_ms, 720);
}

/// Falling contour, then silence. No transcript: the mild pause ends the take
/// before the 640 ms hang.
#[test]
fn clear_fall_then_pause_ends_before_hang() {
    let silence_ms = silence_until_end("clear_fall", true, &falling_contour());
    assert!(silence_ms < 640, "clear_fall silence_ms={silence_ms}");
    assert!(
        (160..=240).contains(&silence_ms),
        "clear_fall silence_ms={silence_ms}, mild pause is 160"
    );
}

/// Rising contour, steady level. No transcript, so the rise holds past the
/// hang and stops at the cap.
#[test]
fn rise_holds_past_the_hang_and_stops_at_the_cap() {
    let silence_ms = silence_until_end("rise", true, &rising_contour());
    assert!(silence_ms > 640, "rise silence_ms={silence_ms}");
    assert!(silence_ms <= 640 + 80, "rise silence_ms={silence_ms}");
    assert_eq!(silence_ms, 720);
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
