// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! Virtual-clock behaviour of the moment core.

use galion_emotion::consent_bits;
use galion_emotion_core::{
    admit_pull, project_universal, BreathPhase, ConductorCue, EmotionCore, IntentMsg, IntentSource,
    VirtualClock, APPROACH, AROUSAL, BREATH_FLOW_AROUSAL, BREATH_VOLUME_AROUSAL, DOMINANCE,
    ENERGY_APPROACH, ENERGY_REST, FATIGUE_AROUSAL, FATIGUE_REST, FRAME_MS, HEALTH_REST,
    HEALTH_VALENCE, STEP_ALLOC_BUDGET, STEP_FLOP_BUDGET, VALENCE, VOLUME_REST,
};

fn run(frames: u32, edit: impl Fn(&mut IntentMsg)) -> EmotionCore {
    let mut core = EmotionCore::at_rest();
    let bits = core.baseline().identity_bits();
    for i in 0..frames {
        let mut msg = IntentMsg::steady(i * FRAME_MS);
        edit(&mut msg);
        core.step(&msg).unwrap();
    }
    assert_eq!(core.baseline().identity_bits(), bits);
    core
}

#[test]
fn the_virtual_clock_steps_every_80_ms() {
    let mut clock = VirtualClock::new();
    let mut core = EmotionCore::at_rest();
    let mut prev = None;
    for _ in 0..10 {
        let t = clock.advance().unwrap();
        let moment = core.step(&IntentMsg::steady(t)).unwrap();
        if let Some(earlier) = prev {
            assert_eq!(moment.t_ms - earlier, FRAME_MS);
        }
        prev = Some(moment.t_ms);
    }
    assert_eq!(prev, Some(9 * FRAME_MS));
    assert_eq!(clock.frames_done(), 10);
    assert_eq!(core.frames(), 10);
    assert_eq!(core.next_ms(), 10 * FRAME_MS);
    assert_eq!(FRAME_MS, 80);

    let untouched = core;
    let err = core
        .step(&IntentMsg::steady(10 * FRAME_MS + 40))
        .unwrap_err();
    match err {
        galion_emotion_core::CoreError::OffClock {
            expected_ms,
            got_ms,
        } => {
            assert_eq!(expected_ms, 800);
            assert_eq!(got_ms, 840);
        }
        other => panic!("unexpected {other:?}"),
    }
    assert_eq!(core, untouched);
}

#[test]
fn moment_swings_leave_the_baseline_bit_identical() {
    let mut core = EmotionCore::at_rest();
    let bits = core.baseline().identity_bits();
    for i in 0..50 {
        let mut msg = IntentMsg::hold(i * FRAME_MS);
        msg.strength = 1.0;
        msg.set_pull(VALENCE, -2.0).unwrap();
        msg.set_pull(AROUSAL, 2.0).unwrap();
        msg.set_pull(DOMINANCE, -2.0).unwrap();
        msg.set_pull(APPROACH, 2.0).unwrap();
        core.step(&msg).unwrap();
    }
    assert_eq!(core.baseline().identity_bits(), bits);
    assert_eq!(core.baseline().persona_id(), "standin-speaker");
    assert!(core.moment().pos[VALENCE] < core.baseline().valence() - 0.2);
    assert_ne!(
        core.moment().pos[AROUSAL].to_bits(),
        core.baseline().arousal().to_bits()
    );
}

#[test]
fn rest_factors_hold_the_moment_on_the_baseline() {
    let core = run(40, |_| {});
    let base = core.baseline().pos;
    let moment = core.moment().pos;
    for i in 0..4 {
        assert_eq!(moment[i].to_bits(), base[i].to_bits(), "axis {i}");
        assert_eq!(core.moment().vel[i].to_bits(), 0.0f32.to_bits());
    }
}

#[test]
fn fatigue_and_breath_and_health_shift_the_moment() {
    let fresh = run(40, |_| {});
    let tired = run(40, |msg| msg.fatigue = 0.90);
    let fatigue_offset = (0.90 - FATIGUE_REST) * FATIGUE_AROUSAL;
    assert!(fatigue_offset < -0.2);
    let drop = fresh.moment().pos[AROUSAL] - tired.moment().pos[AROUSAL];
    assert!(
        drop > -fatigue_offset * 0.8,
        "fatigue drop {drop} expected near {}",
        -fatigue_offset
    );
    assert!(tired.inside().fatigue > fresh.inside().fatigue);

    let full = run(40, |_| {});
    let empty = run(40, |msg| {
        msg.breath_phase = BreathPhase::Exhale;
        msg.lung_volume = 0.10;
        msg.air_flow = -0.8;
    });
    let breath_offset = (0.10 - VOLUME_REST) * BREATH_VOLUME_AROUSAL + (-0.8) * BREATH_FLOW_AROUSAL;
    assert!(breath_offset < -0.1);
    let breath_drop = full.moment().pos[AROUSAL] - empty.moment().pos[AROUSAL];
    assert!(
        breath_drop > -breath_offset * 0.8,
        "breath drop {breath_drop} expected near {}",
        -breath_offset
    );
    assert_eq!(empty.inside().breath.phase, BreathPhase::Exhale);

    let well = run(40, |_| {});
    let unwell = run(40, |msg| msg.health = 0.2);
    let health_offset = (0.2 - HEALTH_REST) * HEALTH_VALENCE;
    let valence_drop = well.moment().pos[VALENCE] - unwell.moment().pos[VALENCE];
    assert!(valence_drop > -health_offset * 0.8);

    let rested = run(40, |_| {});
    let drained = run(40, |msg| msg.energy = 0.2);
    let energy_offset = (0.2 - ENERGY_REST) * ENERGY_APPROACH;
    let approach_drop = rested.moment().pos[APPROACH] - drained.moment().pos[APPROACH];
    assert!(approach_drop > -energy_offset * 0.8);
}

#[test]
fn own_intent_moves_without_caller_consent_and_inbound_does_not() {
    let mut own = EmotionCore::at_rest();
    for i in 0..40 {
        let mut msg = IntentMsg::steady(i * FRAME_MS);
        msg.consent = 0;
        msg.strength = 1.0;
        msg.set_pull(VALENCE, 1.0).unwrap();
        own.step(&msg).unwrap();
    }
    assert!(own.moment().pos[VALENCE] > own.baseline().valence() + 0.2);
    assert!(!own.inbound_mixed());
    assert!(own.persisted_baseline().is_none());
    assert!(own.shared_wire(1).is_none());
    assert_eq!(own.local_wire(1)[7] & 0x0f, 0);

    let mut denied = EmotionCore::at_rest();
    let mut allowed = EmotionCore::at_rest();
    for i in 0..40 {
        let mut msg = IntentMsg::steady(i * FRAME_MS);
        msg.source = IntentSource::Inbound;
        msg.strength = 1.0;
        msg.set_pull(VALENCE, -1.5).unwrap();
        msg.fatigue = 0.95;
        msg.consent = 0;
        assert!(!admit_pull(&msg));
        denied.step(&msg).unwrap();
        msg.consent = consent_bits::SENSE | consent_bits::ADAPT;
        assert!(admit_pull(&msg));
        allowed.step(&msg).unwrap();
    }
    assert!(!denied.inbound_mixed());
    assert!(allowed.inbound_mixed());
    assert!(allowed.moment().pos[VALENCE] < denied.moment().pos[VALENCE] - 0.2);
    assert!((denied.inside().fatigue - FATIGUE_REST).abs() < 1e-6);
    assert!((allowed.inside().fatigue - FATIGUE_REST).abs() < 1e-6);

    let mut msg = IntentMsg::steady(40 * FRAME_MS);
    msg.consent = consent_bits::REMEMBER | consent_bits::SHARE;
    denied.step(&msg).unwrap();
    let saved = denied.persisted_baseline().expect("remember");
    assert_eq!(saved.identity_bits(), denied.baseline().identity_bits());
    assert!(denied.shared_wire(4).is_some());
    let cue: ConductorCue = denied.conductor_cue();
    assert!(cue.may_remember && cue.may_share);
    assert_eq!(cue.baseline, denied.baseline().pos);
}

#[test]
fn intent_message_roundtrips_and_the_step_budget_is_stated() {
    let mut msg = IntentMsg::hold(160);
    msg.set_verb("comfort").unwrap();
    msg.strength = 0.25;
    msg.set_pull(AROUSAL, -0.4).unwrap();
    msg.consent = consent_bits::SENSE;
    let bytes = msg.encode().unwrap();
    let back = IntentMsg::decode(&bytes).unwrap();
    assert_eq!(back.encode().unwrap(), bytes);
    assert_eq!(back, msg);
    assert_eq!(STEP_ALLOC_BUDGET, 0);
    assert!(std::hint::black_box(STEP_FLOP_BUDGET) <= 128);

    let scores = project_universal(&EmotionCore::at_rest().moment());
    assert_eq!(scores.len(), 12);
    assert!(scores.iter().all(|s| s.is_finite() && *s > 0.0));
}

#[test]
fn frame_ms_matches_the_lock_file() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../LOCKS.toml");
    let text = std::fs::read_to_string(path).unwrap();
    assert!(text.contains("frame_ms = 80"));
    assert!(text.contains("frame_hz = 12.5"));
    assert!(text.contains("[\"sense\", \"adapt\", \"remember\", \"share\"]"));
    assert_eq!(consent_bits::SENSE, 1 << 0);
    assert_eq!(consent_bits::ADAPT, 1 << 1);
    assert_eq!(consent_bits::REMEMBER, 1 << 2);
    assert_eq!(consent_bits::SHARE, 1 << 3);
}
