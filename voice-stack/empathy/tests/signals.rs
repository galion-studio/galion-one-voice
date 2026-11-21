// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Emotion-in readings and the prosody weight. No samples are stored.

use galion_emotion_in::{synth, EmotionIn, Session};
use galion_sam_empathy::{Empathy, FeelingName, InboundSignal, MoodPoint, TickIn};
use galion_sam_prosody::map;

fn tick_text(layer: &mut Empathy, text: &str, invite: bool) -> galion_sam_empathy::TickOut {
    layer.tick(TickIn {
        inbound: None,
        asr: text,
        asr_final: false,
        invite_name: invite,
        frame_ms: 80,
    })
}

#[test]
fn emotion_in_reading_is_the_voice_side_of_the_tick() {
    let frame = synth::sine_frame(180.0, 0.2);
    let mut ear = EmotionIn::new();
    let gate = Session::grant_sense();
    let mut reading = ear.push_frame(&gate, &frame).unwrap();
    for _ in 0..7 {
        reading = ear.push_frame(&gate, &frame).unwrap();
    }
    let signal = InboundSignal::from_reading(reading);
    assert_eq!(signal.confidence, reading.mapped.confidence);
    assert_eq!(signal.arousal, reading.mood_target().arousal);
    assert_eq!(signal.valence, reading.mood_target().valence);
    assert_eq!(signal.dominance, reading.mood_target().dominance);
    assert_eq!(reading.retained_audio_samples(), 0);

    let mut layer = Empathy::new();
    let out = layer.tick(TickIn {
        inbound: Some(signal),
        asr: "",
        asr_final: false,
        invite_name: false,
        frame_ms: 80,
    });
    assert!(out.mood.arousal.is_finite() && (0.0..=1.0).contains(&out.mood.arousal));
    assert!(out.mood.valence.is_finite() && (-1.0..=1.0).contains(&out.mood.valence));
    assert!(out.mood.dominance.is_finite() && (0.0..=1.0).contains(&out.mood.dominance));
    assert!(out.named.is_none());
    assert_eq!(out.t_ms, 80);
    assert!(out.e >= 0.0 && out.e <= 1.0);
}

#[test]
fn a_distressed_pad_raises_e_and_a_low_confidence_pad_does_not() {
    let mut layer = Empathy::new();
    let loud = InboundSignal {
        arousal: 0.9,
        valence: -0.8,
        dominance: 0.8,
        confidence: 0.6,
    };
    let out = layer.tick(TickIn {
        inbound: Some(loud),
        asr: "",
        asr_final: false,
        invite_name: true,
        frame_ms: 80,
    });
    assert!(out.e > 0.4, "{}", out.e);
    assert!(out.mood.valence > 0.0);
    assert_eq!(out.named, Some(FeelingName::Angry));
    let plain = map(out.mood.to_prosody_pad());
    assert!(out.prosody.speaking.pitch_shift_st < plain.pitch_shift_st);
    assert!(out.prosody.speaking.pre_pause_ms > plain.pre_pause_ms);
    assert!(out.prosody.control.warmth > plain.prosody_control().warmth);
    assert!(out.prosody.speaking.energy >= 0.6);
    assert!(out.prosody.speaking.tag.is_none());

    let mut quiet = Empathy::new();
    let ignored = InboundSignal {
        arousal: 0.95,
        valence: -1.0,
        dominance: 0.9,
        confidence: 0.1,
    };
    let held = quiet.tick(TickIn {
        inbound: Some(ignored),
        asr: "",
        asr_final: false,
        invite_name: true,
        frame_ms: 80,
    });
    assert_eq!(held.e, 0.0);
    assert_eq!(held.mood, MoodPoint::RESTING);
    assert!(held.named.is_none());
    assert_eq!(
        held.prosody.speaking,
        map(MoodPoint::RESTING.to_prosody_pad())
    );
}

#[test]
fn text_and_voice_both_feed_the_same_step() {
    let mut layer = Empathy::new();
    let mild = InboundSignal {
        arousal: 0.4,
        valence: 0.2,
        dominance: 0.5,
        confidence: 0.4,
    };
    let out = layer.tick(TickIn {
        inbound: Some(mild),
        asr: "I am furious",
        asr_final: true,
        invite_name: false,
        frame_ms: 80,
    });
    assert!(out.e > 0.0, "{}", out.e);
    assert!(out.named.is_none());
    assert!(out.mood.valence > 0.0);
}

#[test]
fn joy_text_keeps_e_at_zero_across_the_slew() {
    let mut layer = Empathy::new();
    let mut last = layer.mood();
    for _ in 0..30 {
        let out = tick_text(&mut layer, "I am happy", false);
        assert_eq!(out.e, 0.0);
        assert!(out.named.is_none());
        last = out.mood;
    }
    assert!(last.valence > MoodPoint::RESTING.valence);
}

#[test]
fn the_router_sees_counts_without_a_spoken_line() {
    let mut layer = Empathy::new();
    layer.note_name("Maciej", galion_sam_empathy::PronounClass::He);
    layer.tick(TickIn {
        inbound: None,
        asr: "I want the notes from him",
        asr_final: true,
        invite_name: false,
        frame_ms: 80,
    });
    let view = layer.view();
    assert_eq!(view.open, 1);
    assert_eq!(view.met, 0);
    assert_eq!(view.dropped, 0);
    assert_eq!(layer.rewrite_query("ask him"), "ask Maciej");
    let id = layer.wants()[0].id;
    assert!(layer.mark_want(id, galion_sam_empathy::WantState::Met));
    assert_eq!(layer.view().met, 1);
    assert_eq!(layer.view().open, 0);
    let prep = layer.prepare_lookup("ask him about the notes");
    assert_eq!(prep.query, "ask Maciej about the notes");
    assert_eq!(prep.query_intent, galion_sam_empathy::Intent::Ask);
    assert_eq!(prep.met, 1);
    assert_eq!(prep.mood.arousal, layer.mood().arousal);
}
