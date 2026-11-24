// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Lungs behaviour. Same drive, same ticks, same frames. No category names.

use galion_emotion_core::{
    BreathPhase, EmotionCore, IntentMsg, VoiceBaseline, AROUSAL, ENERGY_REST, FRAME_MS,
    MASK_BREATH, VOLUME_REST,
};
use horbm_breath::{
    plan_pauses, stamp_intent, BreathError, Drive, Lungs, Pause, PauseWhy, INHALE_SAMPLES,
    NEUTRAL_RMS, PAUSE_CAP, SAMANTHA,
};

fn pauses(text: &str, drive: Drive) -> Vec<Pause> {
    let mut out = [Pause::marker(); PAUSE_CAP];
    let plan = plan_pauses(text, drive, &mut out);
    out[..plan.count].to_vec()
}

fn long_reply() -> String {
    let mut text = String::new();
    for i in 0..48 {
        text.push_str("word");
        text.push_str(&i.to_string());
        text.push(' ');
    }
    text
}

#[test]
fn only_samantha_opens() {
    assert!(Lungs::open(SAMANTHA).is_ok());
    assert!(matches!(Lungs::open("aria"), Err(BreathError::Persona)));
    assert!(matches!(Lungs::open("Samantha"), Err(BreathError::Persona)));
    assert!(matches!(Lungs::open(""), Err(BreathError::Persona)));
    let _ = Lungs::samantha();
}

#[test]
fn pitch_and_persona_bytes_do_not_change_the_drive() {
    let mut low = VoiceBaseline::standin();
    low.pitch_centre_hz = 110.0;
    low.persona = *b"samantha\0\0\0\0\0\0\0\0";
    let mut high = low;
    high.pitch_centre_hz = 440.0;
    high.voice = *b"other-voice\0\0\0\0\0";
    let mut a = EmotionCore::from_baseline(low);
    let mut b = EmotionCore::from_baseline(high);
    a.step(&IntentMsg::steady(0)).unwrap();
    b.step(&IntentMsg::steady(0)).unwrap();
    assert_eq!(Drive::from_core(&a), Drive::from_core(&b));
    assert_eq!(a.baseline().identity_bits(), low.identity_bits());
    assert_eq!(b.baseline().identity_bits(), high.identity_bits());
}

#[test]
fn the_moment_moves_the_drive_and_the_baseline_stays() {
    let mut still = EmotionCore::at_rest();
    let mut moved = EmotionCore::at_rest();
    let before = moved.baseline().identity_bits();
    for i in 0..40 {
        let t = i * FRAME_MS;
        still.step(&IntentMsg::steady(t)).unwrap();
        let mut msg = IntentMsg::steady(t);
        msg.strength = 1.0;
        msg.set_pull(AROUSAL, 1.0).unwrap();
        moved.step(&msg).unwrap();
    }
    assert!(Drive::from_core(&moved).arousal > Drive::from_core(&still).arousal + 0.05);
    assert_eq!(moved.baseline().identity_bits(), before);
}

#[test]
fn energy_on_the_core_raises_the_rate() {
    let mut core = EmotionCore::at_rest();
    let mut msg = IntentMsg::steady(0);
    msg.energy = 1.0;
    core.step(&msg).unwrap();
    let drive = Drive::from_core(&core);
    assert!(drive.energy > ENERGY_REST);
    assert!(horbm_breath::rate_mul(&drive) > horbm_breath::rate_mul(&Drive::rest()));
    assert!(horbm_breath::group_limit_s(&drive) < horbm_breath::group_limit_s(&Drive::rest()));
}

#[test]
fn the_lungs_do_not_copy_the_stand_in_volume() {
    let mut core = EmotionCore::at_rest();
    for i in 0..=15 {
        core.step(&IntentMsg::hold(i * FRAME_MS)).unwrap();
    }
    assert!((core.inside().breath.volume - 0.85).abs() < 1.0e-5);
    let mut lungs = Lungs::samantha();
    let frame = lungs.tick(Drive::from_core(&core), false);
    assert!((frame.volume - VOLUME_REST).abs() < 1.0e-4);
    assert_ne!(
        frame.volume.to_bits(),
        core.inside().breath.volume.to_bits()
    );
}

#[test]
fn a_short_reply_is_one_breath_and_sentences_break() {
    let rest = Drive::rest();
    assert!(pauses("The kettle is on the hob.", rest).is_empty());
    assert!(pauses("   ", rest).is_empty());
    let sentences = pauses("Stop. Go now. Wait here.", rest);
    assert!(sentences.len() >= 2, "got {}", sentences.len());
    assert!(sentences
        .iter()
        .all(|pause| pause.why == PauseWhy::Boundary));
    assert!(sentences
        .windows(2)
        .all(|pair| pair[0].at_byte < pair[1].at_byte));
}

#[test]
fn arousal_and_energy_add_phrase_breaks_when_air_runs_low() {
    let text = long_reply();
    let rest = pauses(&text, Drive::rest());
    let hot = pauses(
        &text,
        Drive {
            arousal: 1.0,
            ..Drive::rest()
        },
    );
    let charged = pauses(
        &text,
        Drive {
            energy: 1.0,
            ..Drive::rest()
        },
    );
    assert!(!rest.is_empty(), "a long reply still needs a breath");
    assert!(rest.iter().any(|pause| pause.why == PauseWhy::Air));
    assert!(
        hot.len() > rest.len(),
        "hot {} rest {}",
        hot.len(),
        rest.len()
    );
    assert!(
        charged.len() >= rest.len(),
        "charged {} rest {}",
        charged.len(),
        rest.len()
    );
    let again = pauses(&text, Drive::rest());
    assert_eq!(rest, again);
}

#[test]
fn the_pause_buffer_reports_truncation() {
    let mut text = String::new();
    for _ in 0..40 {
        text.push_str("Go. ");
    }
    let mut out = [Pause::marker(); PAUSE_CAP];
    let plan = plan_pauses(&text, Drive::rest(), &mut out);
    assert!(plan.truncated);
    assert_eq!(plan.count, PAUSE_CAP);
    assert!(plan.group_ms >= 480 && plan.group_ms <= 4_000);
}

#[test]
fn speech_pressure_falls_and_the_glottis_matches() {
    let mut lungs = Lungs::samantha();
    let mut first = None;
    let mut last = None;
    let mut saw_inhale = false;
    let mut saw_speech = false;
    let mut left_first_run = false;
    for _ in 0..80 {
        if left_first_run {
            break;
        }
        let frame = lungs.tick(Drive::rest(), true);
        assert!(frame.volume.is_finite() && frame.flow.is_finite() && frame.pressure.is_finite());
        assert_eq!(frame.glottis.pressure.to_bits(), frame.pressure.to_bits());
        assert_eq!(frame.glottis.flow.to_bits(), frame.flow.to_bits());
        assert!((-1.0..=1.0).contains(&frame.flow));
        assert!((0.0..=1.0).contains(&frame.volume));
        match frame.phase {
            BreathPhase::Inhale => {
                saw_inhale = true;
                assert!(frame.pause);
                assert!(frame.flow >= 0.0);
                assert_eq!(frame.loudness_gain.to_bits(), 0.0f32.to_bits());
                if saw_speech {
                    left_first_run = true;
                }
            }
            BreathPhase::Speech => {
                saw_speech = true;
                assert!(!frame.pause);
                assert!(frame.flow <= 0.0);
                assert!(frame.loudness_gain > 0.0);
                if first.is_none() {
                    first = Some(frame.pressure);
                }
                last = Some(frame.pressure);
            }
            _ => {
                if saw_speech {
                    left_first_run = true;
                }
            }
        }
    }
    assert!(saw_inhale && saw_speech);
    let first = first.expect("speech");
    let last = last.expect("speech");
    assert!(last < first, "pressure {first} -> {last}");
}

#[test]
fn a_higher_drive_takes_more_breaths_in_the_same_run() {
    let rest = speech_breaks(Drive::rest());
    let hot = speech_breaks(Drive {
        arousal: 1.0,
        energy: 1.0,
        ..Drive::rest()
    });
    assert!(rest >= 2, "rest breaks {rest}");
    assert!(hot > rest, "hot {hot} rest {rest}");
}

fn speech_breaks(drive: Drive) -> u32 {
    let mut lungs = Lungs::samantha();
    let mut breaks = 0u32;
    let mut prev = BreathPhase::Rest;
    for _ in 0..700 {
        let frame = lungs.tick(drive, true);
        if prev == BreathPhase::Speech && frame.phase == BreathPhase::Inhale {
            breaks += 1;
        }
        prev = frame.phase;
    }
    breaks
}

#[test]
fn lower_health_inhales_less_deeply() {
    let well = peak_volume(Drive::rest());
    let unwell = peak_volume(Drive {
        health: 0.0,
        ..Drive::rest()
    });
    assert!(unwell < well, "unwell {unwell} well {well}");
}

fn peak_volume(drive: Drive) -> f32 {
    let mut lungs = Lungs::samantha();
    let mut peak = 0.0f32;
    for _ in 0..20 {
        let frame = lungs.tick(drive, true);
        peak = peak.max(frame.volume);
        if frame.phase == BreathPhase::Speech {
            break;
        }
    }
    peak
}

#[test]
fn the_same_ticks_produce_the_same_frames_and_the_same_inhale() {
    let left = run_pair();
    let right = run_pair();
    assert_eq!(left.0, right.0);
    assert_eq!(left.1, right.1);
    let mut quiet = Lungs::samantha();
    let frame = quiet.tick(Drive::rest(), false);
    assert_eq!(frame.phase, BreathPhase::Rest);
    let mut buf = [1.0f32; 8];
    assert_eq!(quiet.write_inhale(&mut buf), 0);
    assert!(buf.iter().all(|sample| *sample == 1.0));
}

fn run_pair() -> (Vec<horbm_breath::Frame>, Vec<f32>) {
    let mut lungs = Lungs::samantha();
    let drive = Drive::rest();
    let mut frames = Vec::new();
    let mut noise = Vec::new();
    let mut buf = [0.0f32; INHALE_SAMPLES];
    for i in 0..30 {
        let speaking = i > 2;
        let frame = lungs.tick(drive, speaking);
        if frame.phase == BreathPhase::Inhale {
            let n = lungs.write_inhale(&mut buf);
            noise.extend_from_slice(&buf[..n]);
        }
        frames.push(frame);
    }
    (frames, noise)
}

#[test]
fn inhale_noise_is_windowed_deterministic_and_near_the_neutral_rms() {
    let mut lungs = Lungs::samantha();
    let frame = lungs.tick(Drive::rest(), true);
    assert_eq!(frame.phase, BreathPhase::Inhale);
    let mut buf = [0.0f32; INHALE_SAMPLES];
    let n = lungs.write_inhale(&mut buf);
    assert_eq!(n, INHALE_SAMPLES);
    assert_eq!(buf[0].to_bits(), 0.0f32.to_bits());
    assert_eq!(buf[n - 1].to_bits(), 0.0f32.to_bits());
    assert!(buf
        .iter()
        .all(|sample| sample.is_finite() && sample.abs() < 0.2));
    let energy: f32 = buf.iter().map(|sample| sample * sample).sum();
    let rms = (energy / n as f32).sqrt();
    assert!(
        (rms - NEUTRAL_RMS).abs() < 1.0e-4,
        "rms {rms} target {NEUTRAL_RMS}"
    );

    let mut again = Lungs::samantha();
    let _ = again.tick(Drive::rest(), true);
    let mut second = [0.0f32; INHALE_SAMPLES];
    again.write_inhale(&mut second);
    assert_eq!(buf, second);
}

#[test]
fn a_nan_drive_does_not_poison_the_frame() {
    let mut lungs = Lungs::samantha();
    let drive = Drive {
        arousal: f32::NAN,
        energy: f32::NAN,
        fatigue: f32::NAN,
        health: f32::NAN,
    };
    for _ in 0..12 {
        let frame = lungs.tick(drive, true);
        assert!(frame.volume.is_finite());
        assert!(frame.pressure.is_finite());
        assert!(frame.flow.is_finite());
        assert!(frame.loudness_gain.is_finite());
    }
}

#[test]
fn stamp_writes_breath_and_leaves_the_verb() {
    let mut lungs = Lungs::samantha();
    let frame = lungs.tick(Drive::rest(), true);
    let mut msg = IntentMsg::steady(0);
    let verb = msg.verb;
    stamp_intent(&frame, &mut msg);
    assert_eq!(msg.verb, verb);
    assert_eq!(msg.breath_phase, frame.phase);
    assert_eq!(msg.lung_volume.to_bits(), frame.volume.to_bits());
    assert_eq!(msg.air_flow.to_bits(), frame.flow.to_bits());
    assert_ne!(msg.factor_mask & MASK_BREATH, 0);
}

#[test]
fn quiet_recovery_stays_finite_and_near_rest() {
    let mut lungs = Lungs::samantha();
    for _ in 0..40 {
        let _ = lungs.tick(Drive::rest(), true);
    }
    for _ in 0..40 {
        let frame = lungs.tick(Drive::rest(), false);
        assert!(frame.volume.is_finite());
        assert!(!frame.pause);
    }
    let end = lungs.volume();
    assert!((end - VOLUME_REST).abs() < 0.15, "recovered volume {end}");
}

#[test]
fn frame_plans_once_and_then_ticks() {
    let mut lungs = Lungs::samantha();
    let core = EmotionCore::at_rest();
    let first = lungs.frame(&core, true, Some("The kettle is on the hob."));
    let second = lungs.frame(&core, true, Some("The kettle is on the hob."));
    assert_eq!(first.phase, BreathPhase::Inhale);
    assert_ne!(first.volume.to_bits(), second.volume.to_bits());
    assert_eq!(lungs.frames(), 2);
}
