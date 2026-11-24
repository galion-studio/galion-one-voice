// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Closed tests. Synthetic samples only. No model, no audio file.

use horbm_asr::{
    place, within_barge_window, ComputePlace, RecogError, SpeechRecognizer, StandIn,
    TranscriptKind, BARGE_IN_MAX_MS, FRAME_MS, FRAME_SAMPLES, SAMPLE_RATE,
};

fn frame(fill: i16) -> Vec<i16> {
    vec![fill; FRAME_SAMPLES]
}

fn pattern() -> Vec<i16> {
    (0..FRAME_SAMPLES)
        .map(|index| ((index * 17) % 2000) as i16 - 1000)
        .collect()
}

fn run(
    mut ear: StandIn,
    frames: &[Vec<i16>],
) -> (Vec<horbm_asr::Transcript>, Vec<horbm_asr::Transcript>) {
    let mut partials = Vec::new();
    for frame in frames {
        partials.extend(ear.push_frame(frame).unwrap());
    }
    let finals = ear.finish().unwrap();
    (partials, finals)
}

#[test]
fn the_frame_is_80_ms_at_the_ear_rate() {
    assert_eq!(FRAME_MS, 80);
    assert_eq!(SAMPLE_RATE, 16_000);
    assert_eq!(FRAME_SAMPLES, 1_280);
    assert_eq!(BARGE_IN_MAX_MS, 200);
    assert_eq!(place(), ComputePlace::Cpu);
}

#[test]
fn a_mouth_frame_and_a_20_ms_slice_are_rejected() {
    let mut ear = StandIn::new("where is the kettle");
    let mouth = RecogError::Frame {
        got: 1_920,
        expected: FRAME_SAMPLES,
    };
    let slice = RecogError::Frame {
        got: 320,
        expected: FRAME_SAMPLES,
    };
    assert_eq!(ear.push_frame(&vec![0; 1_920]).unwrap_err(), mouth);
    assert_eq!(ear.push_frame(&vec![0; 320]).unwrap_err(), slice);
    assert_eq!(
        ear.push_frame(&[]).unwrap_err().to_string(),
        "audio frame is 0 samples, want 1280"
    );
    assert_eq!(ear.audio_ms(), 0);
    assert!(ear.finish().unwrap().is_empty());
}

#[test]
fn partials_grow_one_word_per_frame_with_timestamps() {
    let mut ear = StandIn::new("  where   is the kettle ");
    assert_eq!(ear.name(), "stand-in");
    let first = ear.push_frame(&frame(0)).unwrap();
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].kind, TranscriptKind::Partial);
    assert_eq!(first[0].text, "where");
    assert_eq!(first[0].start_ms, 0);
    assert_eq!(first[0].end_ms, 80);
    assert_eq!(first[0].words[0].start_ms, 0);
    assert_eq!(first[0].words[0].end_ms, 80);
    let second = ear.push_frame(&pattern()).unwrap();
    assert_eq!(second[0].text, "where is");
    assert_eq!(second[0].end_ms, 160);
    assert_eq!(second[0].words[1].text, "is");
    assert_eq!(second[0].words[1].start_ms, 80);
    assert_eq!(second[0].words[1].end_ms, 160);
    assert!(within_barge_window(ear.audio_ms()));
    ear.push_frame(&frame(0)).unwrap();
    let fourth = ear.push_frame(&frame(1)).unwrap();
    assert_eq!(fourth[0].text, "where is the kettle");
    assert_eq!(fourth[0].end_ms, 320);
    assert!(ear.push_frame(&frame(0)).unwrap().is_empty());
    assert_eq!(ear.audio_ms(), 400);
    let done = ear.finish().unwrap();
    assert_eq!(done.len(), 1);
    assert_eq!(done[0].kind, TranscriptKind::Final);
    assert_eq!(done[0].text, "where is the kettle");
    assert_eq!(done[0].start_ms, 0);
    assert_eq!(done[0].end_ms, 400);
    assert_eq!(done[0].words.len(), 4);
    assert_eq!(done[0].words[3].end_ms, 320);
    assert_eq!(ear.audio_ms(), 0);
    assert!(ear.finish().unwrap().is_empty());
}

#[test]
fn the_same_frames_produce_the_same_transcripts() {
    let zeros = vec![frame(0), frame(0), pattern()];
    let other = vec![pattern(), frame(9), frame(-3)];
    let a = run(StandIn::new("where is the kettle"), &zeros);
    let b = run(StandIn::new("where is the kettle"), &other);
    assert_eq!(a, b);
    assert_eq!(a.0.len(), 3);
    assert_eq!(a.1[0].text, "where is the");
    assert_eq!(a.1[0].end_ms, 240);
}

#[test]
fn cancel_inside_the_lock_emits_no_final() {
    let mut ear = StandIn::new("where is the kettle");
    ear.push_frame(&frame(0)).unwrap();
    let second = ear.push_frame(&pattern()).unwrap();
    assert_eq!(second[0].text, "where is");
    assert_eq!(ear.audio_ms(), 160);
    assert!(within_barge_window(ear.audio_ms()));
    assert!(!within_barge_window(0));
    assert!(within_barge_window(BARGE_IN_MAX_MS));
    assert!(!within_barge_window(FRAME_MS * 3));
    ear.cancel().unwrap();
    assert_eq!(ear.audio_ms(), 0);
    assert!(ear.finish().unwrap().is_empty());
    let again = ear.push_frame(&frame(0)).unwrap();
    assert_eq!(again[0].kind, TranscriptKind::Partial);
    assert_eq!(again[0].text, "where");
    assert_eq!(again[0].start_ms, 0);
    assert_eq!(again[0].end_ms, 80);
}

#[test]
fn an_empty_script_never_invents_a_line() {
    let mut ear = StandIn::new("   ");
    assert!(ear.push_frame(&pattern()).unwrap().is_empty());
    assert_eq!(ear.audio_ms(), 80);
    assert!(ear.finish().unwrap().is_empty());
    assert_eq!(ear.audio_ms(), 0);
}

#[test]
fn finish_before_audio_is_empty() {
    let mut ear = StandIn::new("where");
    assert!(ear.finish().unwrap().is_empty());
    assert!(ear.cancel().is_ok());
}

#[test]
fn stand_in_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<StandIn>();
}
