// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Glottis, nose, and mouth on the lungs frame. No audio file.

use galion_emotion_core::BreathPhase;
use horbm_breath::{controls, Drive, Lungs};

#[test]
fn the_same_drive_is_the_same_bytes() {
    let rest = Drive::rest();
    let once = controls(&rest, BreathPhase::Speech, 0.0, 0.5);
    let twice = controls(&rest, BreathPhase::Speech, 0.0, 0.5);
    assert_eq!(once.to_bytes(), twice.to_bytes());
    let tired = controls(
        &Drive {
            fatigue: 1.0,
            ..rest
        },
        BreathPhase::Speech,
        0.0,
        0.5,
    );
    assert_ne!(once.to_bytes(), tired.to_bytes());
}

#[test]
fn fatigue_opens_the_glottis_and_closes_the_mouth() {
    let rest = controls(&Drive::rest(), BreathPhase::Speech, 0.0, 0.5);
    let tired = controls(
        &Drive {
            fatigue: 1.0,
            ..Drive::rest()
        },
        BreathPhase::Speech,
        0.0,
        0.5,
    );
    assert!(tired.open_quotient > rest.open_quotient);
    assert!(tired.breathiness > rest.breathiness);
    assert!(tired.mouth_openness < rest.mouth_openness);
    assert!((0.35..=0.85).contains(&rest.open_quotient));
    assert!((0.35..=0.85).contains(&tired.open_quotient));
}

#[test]
fn low_health_reduces_nasal_coupling() {
    let well = controls(&Drive::rest(), BreathPhase::Speech, 1.0, 0.4);
    let sick = controls(
        &Drive {
            health: 0.0,
            ..Drive::rest()
        },
        BreathPhase::Speech,
        1.0,
        0.4,
    );
    assert!(well.nasal_coupling > 0.9);
    assert!(sick.nasal_coupling < well.nasal_coupling);
    let oral = controls(&Drive::rest(), BreathPhase::Speech, 0.0, 0.5);
    assert!(oral.nasal_coupling < 0.05);
}

#[test]
fn air_frames_are_not_a_phonatory_quotient() {
    let inhale = controls(&Drive::rest(), BreathPhase::Inhale, 1.0, 0.9);
    assert_eq!(inhale.breathiness, 1.0);
    assert!(inhale.open_quotient >= 0.9);
    assert!(inhale.nasal_coupling < 0.1);
}

#[test]
fn a_speech_tick_carries_the_same_controls() {
    let drive = Drive::rest();
    let mut lungs = Lungs::open("northwind").expect("fixture");
    lungs.set_articulation(0.0, 0.5);
    let mut speech = None;
    for _ in 0..40 {
        let frame = lungs.tick(drive, true);
        if frame.phase == BreathPhase::Speech {
            speech = Some(frame);
            break;
        }
    }
    let frame = speech.expect("speech");
    let expect = controls(&drive, BreathPhase::Speech, 0.0, 0.5);
    assert_eq!(
        frame.open_quotient.to_bits(),
        expect.open_quotient.to_bits()
    );
    assert_eq!(frame.breathiness.to_bits(), expect.breathiness.to_bits());
    assert_eq!(
        frame.nasal_coupling.to_bits(),
        expect.nasal_coupling.to_bits()
    );
    assert_eq!(
        frame.mouth_openness.to_bits(),
        expect.mouth_openness.to_bits()
    );
    assert!(frame.pressure.is_finite());
    let sample = frame.lungs();
    assert_eq!(sample.phase, frame.phase);
    assert_eq!(sample.volume.to_bits(), frame.volume.to_bits());
    assert_eq!(sample.flow.to_bits(), frame.flow.to_bits());
}
