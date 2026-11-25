// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Moment to mouth. No model, no audio file. The render is a generated tone.

use galion_emotion_core::{Breath, BreathPhase, IntentMsg, MomentState, AROUSAL};
use galion_sam_prosody::{map, Pad as ProsodyPad};
use horbm::{Brain, CardHit, CardLookup, Pad as HorbmPad};
use horbm_voice::{
    aria_baseline, instant, is_aria, is_samantha, render_tone, samantha_baseline, BreathPlace,
    EarEvent, FakeEar, FakeMouth, HorbmTurn, MomentVoice, Mouth, SpokenLine, VoiceControls,
    VoiceLoop, AFFECT_TO_VOICE_FRAMES, AFFECT_TO_VOICE_MS, ARIA_PARKED,
};

struct NoCards;

impl CardLookup for NoCards {
    fn lookup(&self, _query: &str) -> Option<CardHit> {
        None
    }
}

fn moment_at(
    base: galion_emotion_core::VoiceBaseline,
    dv: f32,
    da: f32,
    dd: f32,
    dapp: f32,
) -> MomentState {
    let mut pos = base.pos;
    pos[0] = (pos[0] + dv).clamp(-1.0, 1.0);
    pos[1] = (pos[1] + da).clamp(0.0, 1.0);
    pos[2] = (pos[2] + dd).clamp(0.0, 1.0);
    pos[3] = (pos[3] + dapp).clamp(-1.0, 1.0);
    MomentState::at(pos)
}

fn bits(a: f32, b: f32) -> bool {
    a.to_bits() == b.to_bits()
}

fn arrived(
    base: galion_emotion_core::VoiceBaseline,
    moment: MomentState,
    breath: Breath,
) -> VoiceControls {
    let mut voice = MomentVoice::new(base);
    let _ = voice.tick(moment, breath);
    voice.tick(moment, breath)
}

#[test]
fn budgets_match_the_lock() {
    assert_eq!(AFFECT_TO_VOICE_FRAMES, 2);
    assert_eq!(AFFECT_TO_VOICE_MS, 160);
    assert_eq!(AFFECT_TO_VOICE_MS, 80 * AFFECT_TO_VOICE_FRAMES);
}

#[test]
fn samantha_rest_is_the_persona_rest_and_the_e0_line() {
    let rest = galion_emotion::persona_mood::SAMANTHA.dynamics.rest;
    let base = samantha_baseline();
    assert!(is_samantha(&base));
    assert!(!is_aria(&base));
    assert_eq!(base.persona_id(), "samantha");
    assert_eq!(base.voice_id(), "samantha-e0");
    assert_eq!(base.valence(), rest.valence);
    assert_eq!(base.arousal(), rest.arousal);
    assert_eq!(base.dominance(), rest.dominance);
    assert_eq!(base.approach(), 0.0);
    assert_eq!(
        base.pitch_centre_hz,
        galion_emotion_core::VoiceBaseline::standin().pitch_centre_hz
    );

    let controls = instant(base, MomentState::at(base.pos), Breath::rest());
    assert!(!controls.parked);
    assert!(controls.is_neutral_shape());
    assert!(controls.in_bounds());
    assert!(bits(controls.length_scale, 1.0));
    assert!(bits(controls.energy, 1.0));
    assert!(bits(controls.pitch_st, 0.0));
    assert!(bits(controls.pre_pause_ms, 0.0));
    assert!(controls.noise_scale > 0.2);
    assert_eq!(controls.breath, BreathPlace::None);
    assert_eq!(
        controls.pitch_centre_hz.to_bits(),
        base.pitch_centre_hz.to_bits()
    );
    assert_eq!(controls.persona, base.persona);
    assert_eq!(controls.voice, base.voice);
}

#[test]
fn the_map_is_deterministic_and_matches_the_existing_samantha_map() {
    let base = samantha_baseline();
    let moment = moment_at(base, 0.20, 0.10, -0.05, 0.0);
    let once = instant(base, moment, Breath::rest());
    let twice = instant(base, moment, Breath::rest());
    assert!(bits(once.length_scale, twice.length_scale));
    assert!(bits(once.noise_scale, twice.noise_scale));
    assert!(bits(once.noise_w, twice.noise_w));
    assert!(bits(once.pitch_st, twice.pitch_st));
    assert!(bits(once.energy, twice.energy));
    assert!(bits(once.pre_pause_ms, twice.pre_pause_ms));
    assert!(bits(once.rate, twice.rate));
    assert_eq!(once.breath, twice.breath);

    let expect = map(ProsodyPad {
        arousal: ProsodyPad::RESTING.arousal + 0.10,
        valence: ProsodyPad::RESTING.valence + 0.20,
        dominance: ProsodyPad::RESTING.dominance - 0.05,
    });
    assert!((once.length_scale - expect.length_scale).abs() < 1.0e-5);
    assert!((once.noise_scale - expect.noise_scale).abs() < 1.0e-5);
    assert!((once.noise_w - expect.noise_w).abs() < 1.0e-5);
    assert!((once.pitch_st - expect.pitch_shift_st).abs() < 1.0e-5);
    assert!((once.energy - expect.energy).abs() < 1.0e-5);
    assert!((once.pre_pause_ms - expect.pre_pause_ms).abs() < 1.0e-5);
    assert!((once.sentence_silence_s - expect.sentence_silence_s).abs() < 1.0e-5);
    assert!(!once.parked);
    assert_eq!(
        once.pitch_centre_hz.to_bits(),
        base.pitch_centre_hz.to_bits()
    );
}

#[test]
fn excited_is_faster_and_louder_than_calm_on_the_controls() {
    let base = samantha_baseline();
    let excited = arrived(
        base,
        moment_at(base, 0.30, 0.50, 0.10, 0.20),
        Breath::rest(),
    );
    let calm = arrived(
        base,
        moment_at(base, 0.0, -0.30, -0.10, -0.10),
        Breath::rest(),
    );
    assert!(excited.in_bounds() && calm.in_bounds());
    assert!(excited.length_scale < 1.0);
    assert!(calm.length_scale > 1.0);
    assert!(excited.length_scale < calm.length_scale);
    assert!(excited.rate > calm.rate);
    assert!(excited.energy > calm.energy);
    assert!(excited.noise_scale > calm.noise_scale);
    assert!(excited.noise_w >= calm.noise_w);
    assert!(excited.pitch_st > calm.pitch_st);
    assert!(calm.pre_pause_ms >= excited.pre_pause_ms);
    assert_eq!(
        excited.pitch_centre_hz.to_bits(),
        calm.pitch_centre_hz.to_bits()
    );
    assert_eq!(excited.persona, base.persona);
    assert_eq!(excited.voice, base.voice);
}

#[test]
fn a_held_target_arrives_in_two_frames_without_a_jump() {
    let base = samantha_baseline();
    let mut voice = MomentVoice::new(base);
    let start_bits = voice.identity();
    let moment = moment_at(base, 0.30, 0.50, 0.10, 0.0);
    let goal = instant(base, moment, Breath::rest());
    let start = voice.applied();
    let mid = voice.tick(moment, Breath::rest());
    let full = (goal.length_scale - start.length_scale).abs();
    let first = (mid.length_scale - start.length_scale).abs();
    assert!(full > 0.05);
    assert!(first < full - 1.0e-3, "frame 1 jumped the whole gap");
    assert!((mid.length_scale - (start.length_scale + goal.length_scale) * 0.5).abs() < 1.0e-4);
    assert!((mid.energy - (start.energy + goal.energy) * 0.5).abs() < 1.0e-4);
    assert_ne!(mid.length_scale.to_bits(), goal.length_scale.to_bits());

    let done = voice.tick(moment, Breath::rest());
    assert_eq!(done.length_scale.to_bits(), goal.length_scale.to_bits());
    assert_eq!(done.energy.to_bits(), goal.energy.to_bits());
    assert_eq!(done.noise_scale.to_bits(), goal.noise_scale.to_bits());
    assert_eq!(done.noise_w.to_bits(), goal.noise_w.to_bits());
    assert_eq!(done.pitch_st.to_bits(), goal.pitch_st.to_bits());
    assert_eq!(done.rate.to_bits(), goal.rate.to_bits());
    let held = voice.tick(moment, Breath::rest());
    assert_eq!(held.length_scale.to_bits(), done.length_scale.to_bits());
    assert_eq!(voice.baseline().identity_bits(), start_bits);
    assert_eq!(voice.identity(), start_bits);
}

#[test]
fn a_new_target_mid_ramp_does_not_jump() {
    let base = samantha_baseline();
    let mut voice = MomentVoice::new(base);
    let excited = moment_at(base, 0.30, 0.50, 0.0, 0.0);
    let calm = moment_at(base, -0.10, -0.30, 0.0, 0.0);
    let mid = voice.tick(excited, Breath::rest());
    let calm_goal = instant(base, calm, Breath::rest());
    let next = voice.tick(calm, Breath::rest());
    assert_ne!(
        next.length_scale.to_bits(),
        calm_goal.length_scale.to_bits()
    );
    let halfway = (mid.length_scale + calm_goal.length_scale) * 0.5;
    assert!((next.length_scale - halfway).abs() < 1.0e-4);
    let landed = voice.tick(calm, Breath::rest());
    assert_eq!(
        landed.length_scale.to_bits(),
        calm_goal.length_scale.to_bits()
    );
    assert_eq!(voice.baseline().identity_bits(), base.identity_bits());
}

#[test]
fn clamps_hold_for_a_poisoned_moment() {
    let base = samantha_baseline();
    let wild = MomentState {
        t_ms: 80,
        tick: 1,
        pos: [1.0e20, f32::NAN, -50.0, f32::INFINITY],
        vel: [f32::NAN, f32::INFINITY, 0.0, -1.0e20],
    };
    let controls = instant(base, wild, Breath::rest());
    assert!(controls.in_bounds());
    assert!(controls.length_scale.is_finite());
    assert!(controls.energy.is_finite());
    assert!(controls.pitch_st.is_finite());
    assert!(controls.noise_scale.is_finite());
    assert!(controls.noise_w.is_finite());
    assert_eq!(
        controls.pitch_centre_hz.to_bits(),
        base.pitch_centre_hz.to_bits()
    );
    assert_eq!(voice_bits(&MomentVoice::new(base)), base.identity_bits());

    let corner = instant(
        base,
        MomentState::at([-1.0, 1.0, 1.0, 1.0]),
        Breath {
            phase: BreathPhase::Inhale,
            volume: 4.0,
            flow: -4.0,
        },
    );
    assert!(corner.in_bounds());
    assert!(corner.breath_ms <= AFFECT_TO_VOICE_MS);
    assert!(corner.pre_pause_ms <= 250.0);
}

fn voice_bits(voice: &MomentVoice) -> galion_emotion_core::BaselineBits {
    voice.baseline().identity_bits()
}

#[test]
fn breath_is_placed_on_the_arriving_frame() {
    let base = samantha_baseline();
    let mut voice = MomentVoice::new(base);
    let inhale = Breath {
        phase: BreathPhase::Inhale,
        volume: 0.8,
        flow: 0.7,
    };
    let first = voice.tick(MomentState::at(base.pos), inhale);
    assert_eq!(first.breath, BreathPlace::None);
    let second = voice.tick(MomentState::at(base.pos), inhale);
    assert_eq!(second.breath, BreathPlace::Before);
    assert_eq!(second.breath_ms, 80);

    let exhale = instant(
        base,
        MomentState::at(base.pos),
        Breath {
            phase: BreathPhase::Exhale,
            volume: 0.4,
            flow: -0.6,
        },
    );
    assert_eq!(exhale.breath, BreathPlace::After);
    assert_eq!(exhale.breath_ms, 80);
    let speech = instant(
        base,
        MomentState::at(base.pos),
        Breath {
            phase: BreathPhase::Speech,
            volume: 0.5,
            flow: 0.0,
        },
    );
    assert_eq!(speech.breath, BreathPlace::None);
    assert_eq!(speech.breath_ms, 0);
}

#[test]
fn aria_is_parked_and_the_baseline_does_not_move() {
    let aria = aria_baseline();
    assert!(is_aria(&aria));
    assert!(!is_samantha(&aria));
    let mut voice = MomentVoice::for_persona(aria, true);
    let bits = voice.identity();
    let excited = moment_at(samantha_baseline(), 0.4, 0.5, 0.2, 0.3);
    let inhale = Breath {
        phase: BreathPhase::Inhale,
        volume: 0.9,
        flow: 0.8,
    };
    let first = voice.tick(excited, inhale);
    let second = voice.tick(moment_at(aria, -0.4, -0.2, -0.2, -0.4), Breath::rest());
    assert_eq!(ARIA_PARKED, first.parked);
    assert!(first.parked && second.parked);
    assert!(first.is_neutral_shape() && second.is_neutral_shape());
    assert!(first.in_bounds() && second.in_bounds());
    assert_eq!(first.length_scale.to_bits(), second.length_scale.to_bits());
    assert_eq!(first.energy.to_bits(), second.energy.to_bits());
    assert_eq!(first.noise_scale, 0.0);
    assert_eq!(first.noise_w, 0.0);
    assert_eq!(
        first.pitch_centre_hz.to_bits(),
        aria.pitch_centre_hz.to_bits()
    );
    assert_eq!(first.persona, aria.persona);
    assert_eq!(voice.identity(), bits);
    assert_eq!(voice.baseline().identity_bits(), bits);
    assert_eq!(voice.baseline().pos, aria.pos);
}

#[test]
fn synthetic_excited_renders_faster_and_higher_energy_than_calm() {
    // No Piper feature, no model, no file. A 180 Hz tone in memory.
    let base = samantha_baseline();
    let excited = arrived(
        base,
        moment_at(base, 0.30, 0.50, 0.10, 0.20),
        Breath::rest(),
    );
    let calm = arrived(
        base,
        moment_at(base, 0.0, -0.30, -0.10, -0.10),
        Breath::rest(),
    );
    let fast = render_tone(excited, 16_000);
    let slow = render_tone(calm, 16_000);
    assert_eq!(fast.sample_rate, 16_000);
    assert!(fast.duration_ms < slow.duration_ms);
    assert!(slow.duration_ms - fast.duration_ms >= 10);
    assert!(fast.rms > slow.rms);
    assert!(fast.duration_ms < 2_000 && slow.duration_ms < 2_000);
    assert!(!fast.samples.is_empty() && !slow.samples.is_empty());
    assert!(fast.samples.iter().any(|sample| *sample != 0));

    let rest = instant(base, MomentState::at(base.pos), Breath::rest());
    let flat = render_tone(rest, 16_000);
    assert_eq!(flat.duration_ms, 200);

    let mut fast_mouth = FakeMouth::new();
    fast_mouth.feel(excited);
    fast_mouth.start(&line("hello"));
    let mut slow_mouth = FakeMouth::new();
    slow_mouth.feel(calm);
    slow_mouth.start(&line("hello"));
    assert!(frames_until_done(&mut fast_mouth) < frames_until_done(&mut slow_mouth));
}

fn line(text: &str) -> SpokenLine {
    SpokenLine {
        text: text.to_string(),
        mood: HorbmPad::RESTING,
        rate: 1.0,
        energy: 1.0,
        pitch_st: 0.0,
        pause_ms: 0.0,
    }
}

fn frames_until_done(mouth: &mut FakeMouth) -> u32 {
    let mut frames = 0;
    while mouth.speaking() {
        let _ = mouth.pull();
        frames += 1;
        assert!(frames < 64, "mouth did not finish");
    }
    frames
}

#[test]
fn the_loop_puts_the_moment_on_the_mouth_inside_two_frames() {
    let mut turn = HorbmTurn::with_cards(NoCards);
    assert_eq!(turn.brain().emotion().baseline().persona_id(), "samantha");
    let bits = turn.brain().emotion().baseline().identity_bits();
    let mood = turn.brain().mood_meta();
    let mut msg = IntentMsg::steady(0);
    msg.strength = 1.0;
    msg.set_pull(AROUSAL, 0.6).unwrap();
    turn.brain_mut().step_moment(&msg).unwrap();
    assert_eq!(turn.brain().emotion().baseline().identity_bits(), bits);
    assert_eq!(turn.brain().mood_meta(), mood);
    let cue = turn.brain().conductor_cue();
    assert_eq!(cue.breath_phase, BreathPhase::Rest);
    let expect = instant(
        turn.brain().emotion().baseline(),
        turn.brain().emotion().moment(),
        Breath::rest(),
    );
    assert!(expect.length_scale < 1.0);

    let mut voice = VoiceLoop::new(FakeEar::script([]), FakeMouth::new(), turn);
    let first = voice.tick(&[]);
    assert_eq!(first.t_ms, 0);
    let mid = voice.mouth().controls();
    assert_ne!(mid.length_scale.to_bits(), expect.length_scale.to_bits());
    let second = voice.tick(&[]);
    assert_eq!(second.t_ms, 80);
    let done = voice.mouth().controls();
    assert_eq!(done.length_scale.to_bits(), expect.length_scale.to_bits());
    assert_eq!(done.energy.to_bits(), expect.energy.to_bits());
    assert_eq!(done.noise_scale.to_bits(), expect.noise_scale.to_bits());
    assert_eq!(done.pitch_st.to_bits(), expect.pitch_st.to_bits());
    assert!(!done.parked);
    assert_eq!(
        done.pitch_centre_hz.to_bits(),
        samantha_baseline().pitch_centre_hz.to_bits()
    );
    assert_eq!(done.persona, samantha_baseline().persona);
}

#[test]
fn inner_life_moves_the_moment_and_not_who_she_is() {
    let mut turn = HorbmTurn::with_cards(NoCards);
    let bits = turn.brain().emotion().baseline().identity_bits();
    let mood = turn.brain().mood_meta();
    let nudged = turn.brain_mut().inner_nudge("what?", None).unwrap();
    assert!(nudged.surprise > 0.0);
    assert_eq!(turn.brain().inner().learned_count(), 0);
    assert!(!galion_emotion_core::learning_allowed());
    assert_eq!(turn.brain().mood_meta(), mood);
    assert_eq!(turn.brain().emotion().baseline().identity_bits(), bits);
    let baseline = turn.brain().emotion().baseline();
    let moment = turn.brain().emotion().moment();
    assert!(moment.pos[AROUSAL] > baseline.pos[AROUSAL]);
    let cue = turn.brain().conductor_cue();
    let breath = Breath {
        phase: cue.breath_phase,
        volume: cue.lung_volume,
        flow: cue.air_flow,
    };
    let expect = instant(baseline, moment, breath);
    let mut voice = VoiceLoop::new(FakeEar::script([EarEvent::quiet()]), FakeMouth::new(), turn);
    let _ = voice.tick(&[]);
    let done = voice.tick(&[]).t_ms;
    assert_eq!(done, 80);
    let controls = voice.mouth().controls();
    assert_eq!(controls.energy.to_bits(), expect.energy.to_bits());
    assert_eq!(
        controls.length_scale.to_bits(),
        expect.length_scale.to_bits()
    );
    assert_eq!(controls.breath, expect.breath);
    assert_eq!(
        controls.pitch_centre_hz.to_bits(),
        baseline.pitch_centre_hz.to_bits()
    );
    assert!(!controls.parked);

    let mut seated = HorbmTurn::with_cards(NoCards);
    seated.seat(aria_baseline());
    let aria_bits = seated.brain().emotion().baseline().identity_bits();
    seated.brain_mut().inner_nudge("what?", None).unwrap();
    assert_eq!(seated.brain().inner().learned_count(), 0);
    assert_eq!(
        seated.brain().emotion().baseline().identity_bits(),
        aria_bits
    );
    let baseline = seated.brain().emotion().baseline();
    let moment = seated.brain().emotion().moment();
    let cue = seated.brain().conductor_cue();
    let breath = Breath {
        phase: cue.breath_phase,
        volume: cue.lung_volume,
        flow: cue.air_flow,
    };
    let expect = instant(baseline, moment, breath);
    let mut voice = VoiceLoop::new(FakeEar::script([]), FakeMouth::new(), seated);
    let _ = voice.tick(&[]);
    let _ = voice.tick(&[]);
    let controls = voice.mouth().controls();
    assert!(!controls.parked);
    assert_eq!(controls.energy.to_bits(), expect.energy.to_bits());
    assert_eq!(
        controls.length_scale.to_bits(),
        expect.length_scale.to_bits()
    );
    assert_eq!(controls.persona, aria_baseline().persona);
    assert_eq!(
        controls.pitch_centre_hz.to_bits(),
        aria_baseline().pitch_centre_hz.to_bits()
    );
}

#[test]
fn seating_does_not_rewrite_empathy() {
    let mut brain = Brain::with_cards(NoCards, None);
    let mood = brain.mood_meta();
    brain.seat(samantha_baseline());
    assert_eq!(brain.mood_meta(), mood);
    assert_eq!(brain.emotion().baseline().persona_id(), "samantha");
    brain.seat(aria_baseline());
    assert_eq!(brain.mood_meta(), mood);
    assert_eq!(brain.emotion().baseline().persona_id(), "aria");
    assert_eq!(brain.inner().learned_count(), 0);
}

fn id_slot(id: &str) -> [u8; 16] {
    let mut out = [0u8; 16];
    let bytes = id.as_bytes();
    let n = bytes.len().min(15);
    out[..n].copy_from_slice(&bytes[..n]);
    out
}

fn northwind_baseline() -> galion_emotion_core::VoiceBaseline {
    let dev = samantha_baseline();
    galion_emotion_core::VoiceBaseline {
        persona: id_slot("northwind"),
        voice: id_slot("northwind-e0"),
        pos: dev.pos,
        pitch_centre_hz: dev.pitch_centre_hz,
    }
}

#[test]
fn northwind_baseline_moves_its_voice() {
    let base = northwind_baseline();
    let moment = moment_at(base, 0.20, 0.25, 0.0, 0.10);
    let controls = instant(base, moment, Breath::rest());
    let rest = instant(base, MomentState::at(base.pos), Breath::rest());
    assert!(!controls.parked);
    assert!(controls.in_bounds());
    assert_ne!(controls.length_scale.to_bits(), rest.length_scale.to_bits());
    assert_ne!(controls.energy.to_bits(), rest.energy.to_bits());
    assert_eq!(controls.persona, id_slot("northwind"));
    assert_ne!(controls.persona, samantha_baseline().persona);

    let dev = samantha_baseline();
    let dev_controls = instant(dev, moment_at(dev, 0.20, 0.25, 0.0, 0.10), Breath::rest());
    assert_eq!(
        controls.length_scale.to_bits(),
        dev_controls.length_scale.to_bits()
    );
    assert_eq!(controls.energy.to_bits(), dev_controls.energy.to_bits());
    assert_eq!(controls.pitch_st.to_bits(), dev_controls.pitch_st.to_bits());
    assert_eq!(
        controls.noise_scale.to_bits(),
        dev_controls.noise_scale.to_bits()
    );
    assert_eq!(
        controls.pre_pause_ms.to_bits(),
        dev_controls.pre_pause_ms.to_bits()
    );
}

/// Bits of `instant` on the development baseline, taken from main before
/// the map stopped gating on a persona id. A retune of the mouth changes
/// these. No audio.
#[test]
fn dev_persona_controls_bit_identical() {
    let base = samantha_baseline();
    let rows = [
        row(
            0.0,
            0.0,
            0.0,
            0.0,
            Snap {
                length: 0x3f80_0000,
                noise: 0x3f2a_c083,
                noise_w: 0x3f4c_cccd,
                silence: 0x3e4c_cccd,
                pitch: 0x0000_0000,
                energy: 0x3f80_0000,
                pause: 0x0000_0000,
                rate: 0x3f80_0000,
            },
        ),
        row(
            0.20,
            0.10,
            -0.05,
            0.0,
            Snap {
                length: 0x3f75_c28f,
                noise: 0x3f34_fdf3,
                noise_w: 0x3f54_7ae1,
                silence: 0x3e33_3334,
                pitch: 0x3f0a_3d70,
                energy: 0x3f83_c0ec,
                pause: 0x0000_0000,
                rate: 0x3f85_5556,
            },
        ),
        row(
            0.30,
            0.50,
            0.10,
            0.20,
            Snap {
                length: 0x3f4c_cccd,
                noise: 0x3f54_fdf4,
                noise_w: 0x3f73_3334,
                silence: 0x3da3_d70a,
                pitch: 0x3fca_3d71,
                energy: 0x3f99_999a,
                pause: 0x0000_0000,
                rate: 0x3f99_999a,
            },
        ),
        row(
            0.0,
            -0.30,
            -0.10,
            -0.10,
            Snap {
                length: 0x3f90_0000,
                noise: 0x3f13_b646,
                noise_w: 0x3f35_c290,
                silence: 0x3e8c_cccd,
                pitch: 0xbf28_f5c3,
                energy: 0x3f55_844e,
                pause: 0x4348_0000,
                rate: 0x3f63_8e39,
            },
        ),
        row(
            0.0,
            0.0,
            0.0,
            0.40,
            Snap {
                length: 0x3f7a_e148,
                noise: 0x3f2a_c083,
                noise_w: 0x3f4c_cccd,
                silence: 0x3e4c_cccd,
                pitch: 0x0000_0000,
                energy: 0x3f82_8f5c,
                pause: 0x0000_0000,
                rate: 0x3f82_9cbc,
            },
        ),
    ];
    for (dv, da, dd, dapp, expect) in rows {
        let moment = moment_at(base, dv, da, dd, dapp);
        let controls = instant(base, moment, Breath::rest());
        assert_eq!(controls.breath, BreathPlace::None);
        assert_eq!(controls.breath_ms, 0);
        assert_snap(&controls, &base, expect);
        let landed = arrived(base, moment, Breath::rest());
        assert_eq!(landed.breath, BreathPlace::None);
        assert_eq!(landed.breath_ms, 0);
        assert_snap(&landed, &base, expect);
    }

    let inhale = instant(
        base,
        MomentState::at(base.pos),
        Breath {
            phase: BreathPhase::Inhale,
            volume: 0.8,
            flow: 0.4,
        },
    );
    assert_snap(
        &inhale,
        &base,
        Snap {
            length: 0x3f80_0000,
            noise: 0x3f2a_c083,
            noise_w: 0x3f4c_cccd,
            silence: 0x3e4c_cccd,
            pitch: 0x0000_0000,
            energy: 0x3f80_0000,
            pause: 0x0000_0000,
            rate: 0x3f80_0000,
        },
    );
    assert_eq!(inhale.breath, BreathPlace::Before);
    assert_eq!(inhale.breath_ms, 80);

    let exhale = instant(
        base,
        MomentState::at(base.pos),
        Breath {
            phase: BreathPhase::Exhale,
            volume: 0.4,
            flow: -0.3,
        },
    );
    assert_eq!(exhale.length_scale.to_bits(), 0x3f80_0000);
    assert_eq!(exhale.breath, BreathPlace::After);
    assert_eq!(exhale.breath_ms, 80);
    assert!(!exhale.parked);
}

#[test]
fn parked_flag_gives_the_parked_packet() {
    let base = northwind_baseline();
    let mut voice = MomentVoice::for_persona(base, true);
    let moment = moment_at(base, 0.40, 0.50, 0.20, 0.30);
    let inhale = Breath {
        phase: BreathPhase::Inhale,
        volume: 0.9,
        flow: 0.8,
    };
    let first = voice.tick(moment, inhale);
    let second = voice.tick(moment, Breath::rest());
    let packet = VoiceControls::unsteered();
    assert!(first.parked && second.parked);
    assert!(first.is_neutral_shape() && second.is_neutral_shape());
    assert_eq!(first.noise_scale, 0.0);
    assert_eq!(first.noise_w, 0.0);
    assert_eq!(first.length_scale.to_bits(), packet.length_scale.to_bits());
    assert_eq!(first.energy.to_bits(), packet.energy.to_bits());
    assert_eq!(first.pitch_st.to_bits(), packet.pitch_st.to_bits());
    assert_eq!(first.breath, BreathPlace::None);
    assert_eq!(first.breath_ms, 0);
    assert_eq!(first.persona, base.persona);
    assert_eq!(first.voice, base.voice);
    assert_eq!(
        first.pitch_centre_hz.to_bits(),
        base.pitch_centre_hz.to_bits()
    );
    assert_eq!(voice.baseline().identity_bits(), base.identity_bits());
    assert_eq!(voice.baseline().pos, base.pos);
    assert_eq!(first.length_scale.to_bits(), second.length_scale.to_bits());
}

#[derive(Clone, Copy)]
struct Snap {
    length: u32,
    noise: u32,
    noise_w: u32,
    silence: u32,
    pitch: u32,
    energy: u32,
    pause: u32,
    rate: u32,
}

fn row(dv: f32, da: f32, dd: f32, dapp: f32, snap: Snap) -> (f32, f32, f32, f32, Snap) {
    (dv, da, dd, dapp, snap)
}

fn assert_snap(controls: &VoiceControls, base: &galion_emotion_core::VoiceBaseline, expect: Snap) {
    assert!(!controls.parked);
    assert_eq!(controls.length_scale.to_bits(), expect.length);
    assert_eq!(controls.noise_scale.to_bits(), expect.noise);
    assert_eq!(controls.noise_w.to_bits(), expect.noise_w);
    assert_eq!(controls.sentence_silence_s.to_bits(), expect.silence);
    assert_eq!(controls.pitch_st.to_bits(), expect.pitch);
    assert_eq!(controls.energy.to_bits(), expect.energy);
    assert_eq!(controls.pre_pause_ms.to_bits(), expect.pause);
    assert_eq!(controls.rate.to_bits(), expect.rate);
    assert_eq!(controls.persona, base.persona);
    assert_eq!(controls.voice, base.voice);
    assert_eq!(
        controls.pitch_centre_hz.to_bits(),
        base.pitch_centre_hz.to_bits()
    );
}
