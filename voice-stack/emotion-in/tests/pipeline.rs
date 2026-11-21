// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Behaviour the unit tests describe from the outside: directions of the
//! features, a 16 kHz resample, and the release-mode 80 ms stage bar.

use galion_emotion_in::clock::STAGE_BUDGET_US;
use galion_emotion_in::consent::Session;
use galion_emotion_in::pad::Pad;
use galion_emotion_in::stream::{analyse_pcm, measure_stages, synthetic_scenarios};
use galion_emotion_in::synth;
use galion_emotion_in::wav::analyse_pcm_s16;
use galion_emotion_in::{FakeBrain, MoodTarget, ReplyEngine};
use std::io::Cursor;

fn scene<'a>(all: &'a [galion_emotion_in::Scenario], name: &str) -> &'a galion_emotion_in::Report {
    &all.iter()
        .find(|s| s.name == name)
        .unwrap_or_else(|| panic!("missing {name}"))
        .report
}

#[test]
fn features_move_the_way_the_rules_say() {
    let all = synthetic_scenarios().unwrap();
    let silence = scene(&all, "silence");
    assert_eq!(silence.reading.mood_target(), Pad::RESTING);
    assert_eq!(silence.reading.prosody.f0_hz, 0.0);
    assert_eq!(silence.audio_retained_samples, 0);

    let quiet = scene(&all, "quiet-low-120hz");
    let loud = scene(&all, "loud-low-120hz");
    assert!(loud.reading.prosody.rms > quiet.reading.prosody.rms * 2.0);
    assert!(loud.reading.mapped.estimate.arousal > quiet.reading.mapped.estimate.arousal + 0.05);
    assert_eq!(
        quiet.reading.prosody.nuclei, 0,
        "a steady tone is not a syllable train"
    );
    assert_eq!(
        loud.reading.prosody.nuclei, 0,
        "a steady tone is not a syllable train"
    );
    assert!(
        (quiet.reading.prosody.f0_hz - 120.0).abs() < 4.0,
        "{:?}",
        quiet.reading.prosody
    );
    assert!(
        (loud.reading.prosody.f0_hz - 120.0).abs() < 4.0,
        "{:?}",
        loud.reading.prosody
    );

    let low = scene(&all, "mid-low-120hz");
    let high = scene(&all, "mid-high-240hz");
    assert!((low.reading.prosody.rms - high.reading.prosody.rms).abs() < 0.02);
    assert!(
        high.reading.prosody.f0_hz > low.reading.prosody.f0_hz + 80.0,
        "{:?} {:?}",
        high.reading.prosody,
        low.reading.prosody
    );
    assert!(high.reading.mapped.estimate.arousal > low.reading.mapped.estimate.arousal);

    let slow = scene(&all, "am-2hz");
    let fast = scene(&all, "am-5hz");
    assert!(
        fast.reading.prosody.nuclei + 1 >= slow.reading.prosody.nuclei + 3,
        "slow {:?} fast {:?}",
        slow.reading.prosody,
        fast.reading.prosody
    );
    assert!(fast.reading.prosody.articulation_hz > slow.reading.prosody.articulation_hz + 1.0);
    assert!(fast.reading.mapped.estimate.arousal > slow.reading.mapped.estimate.arousal);

    let gap = scene(&all, "gap-400ms");
    assert!(
        (360..=440).contains(&gap.reading.prosody.longest_pause_ms),
        "{:?}",
        gap.reading.prosody
    );
    assert!(
        gap.reading.prosody.pause_ratio > 0.2,
        "{:?}",
        gap.reading.prosody
    );

    let flat = scene(&all, "loud-flat-240hz");
    let melodic = scene(&all, "melodic-140-220");
    assert!(
        flat.reading.mapped.anger_press > 0.2,
        "{:?}",
        flat.reading.mapped
    );
    assert_eq!(melodic.reading.mapped.anger_press, 0.0);
    assert!(
        melodic.reading.prosody.f0_range_hz > flat.reading.prosody.f0_range_hz + 30.0,
        "flat {:?} melodic {:?}",
        flat.reading.prosody,
        melodic.reading.prosody
    );
    assert!(melodic.reading.mapped.estimate.valence > flat.reading.mapped.estimate.valence + 0.15);
}

#[test]
fn sixteen_khz_sine_still_reports_its_pitch() {
    let n = 12 * 1_280;
    let samples = synth::sine(180.0, 0.2, n, 16_000);
    let report = analyse_pcm(&samples, 16_000, &Session::grant_sense()).unwrap();
    assert!(
        (report.reading.prosody.f0_hz - 180.0).abs() < 3.6,
        "{:?}",
        report.reading.prosody
    );
    assert_eq!(report.frames, 12);
    assert_eq!(report.audio_retained_samples, 0);
}

#[test]
fn raw_s16_matches_the_wav_path() {
    let samples = synth::steady_frames(12, 180.0, 0.2);
    let mut bytes = Vec::new();
    for s in &samples {
        let c = s.clamp(-1.0, 1.0);
        let v = (c * 32767.0).round() as i16;
        bytes.extend_from_slice(&v.to_le_bytes());
    }
    let report = analyse_pcm_s16(Cursor::new(bytes), 24_000).unwrap();
    assert!(
        (report.reading.prosody.f0_hz - 180.0).abs() < 3.6,
        "{:?}",
        report.reading.prosody
    );
}

#[test]
fn mood_target_is_what_the_fake_brain_ticks_toward() {
    let samples = synth::steady_frames(12, 180.0, 0.2);
    let report = analyse_pcm(&samples, 24_000, &Session::grant_sense()).unwrap();
    let mut brain = FakeBrain::open("/no/memory", "/no/knowledge", "/no/cases");
    brain.set_mood_target(report.reading.mood_target());
    let before = brain.mood();
    let out = brain.turn("heard");
    assert!(out.mood.arousal != before.arousal || report.reading.mood_target() == Pad::RESTING);
    assert_eq!(out.lookup_us, 0);
    assert!(out.sources.is_empty());
}

#[test]
fn release_push_is_inside_the_80ms_bar() {
    let times = measure_stages(30);
    eprintln!("{}", galion_emotion_in::stage_text(&times));
    assert!(
        times.push_p95_us < 500_000,
        "push p95 {} us",
        times.push_p95_us
    );
    if !cfg!(debug_assertions) {
        assert!(
            times.pitch_p95_us < STAGE_BUDGET_US,
            "pitch p95 {}",
            times.pitch_p95_us
        );
        assert!(
            times.energy_p95_us < STAGE_BUDGET_US,
            "energy p95 {}",
            times.energy_p95_us
        );
        assert!(
            times.push_p95_us < STAGE_BUDGET_US,
            "push p95 {}",
            times.push_p95_us
        );
        assert!(
            times.tick_p95_us < STAGE_BUDGET_US,
            "tick p95 {}",
            times.tick_p95_us
        );
    }
}
