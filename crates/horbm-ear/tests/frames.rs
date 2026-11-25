// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Synthetic frames only. Nothing here reads or writes an audio file.

use std::time::Instant;

use horbm_ear::consent_bits::{ADAPT, REMEMBER, SENSE};
use horbm_ear::link::{push_frame, FloorCue, FloorTick, ACTIVITY_MARK};
use horbm_ear::port::{EmotionCore, IntentSource, RamPort, VoiceBaseline, AROUSAL, VALENCE};
use horbm_ear::{Ear, EarConfig, FrameOut, Purpose, FRAME_MS, FRAME_SAMPLES, SAMPLE_RATE};

const PI: f32 = std::f32::consts::PI;

fn grant(ear: &mut Ear) {
    ear.set_grant(SENSE, Purpose::PersonalAssistant);
}

fn heard(ear: &mut Ear, pcm: &[f32]) -> horbm_ear::ProsodyFrame {
    match ear.push(pcm) {
        FrameOut::Heard(frame) => frame,
        other => panic!("expected a frame, got {other:?}"),
    }
}

fn fill_sine(buf: &mut [f32], hz: f32, amp: f32) {
    let step = 2.0 * PI * hz / SAMPLE_RATE as f32;
    let mut phase = 0.0f32;
    for s in buf {
        *s = amp * phase.sin();
        phase += step;
    }
}

fn fill_noise(buf: &mut [f32], seed: u64, amp: f32) {
    let mut x = seed;
    for s in buf {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        let u = (x >> 40) as f32 / 16_777_216.0;
        *s = amp * (2.0 * u - 1.0);
    }
}

fn fill_chirp(buf: &mut [f32], f0: f32, f1: f32, amp: f32) {
    let n = buf.len() as f32;
    let mut phase = 0.0f32;
    for (i, s) in buf.iter_mut().enumerate() {
        let hz = f0 + (f1 - f0) * (i as f32 / n);
        *s = amp * phase.sin();
        phase += 2.0 * PI * hz / SAMPLE_RATE as f32;
    }
}

/// Alternate the length of each cycle. `stretch` is the fractional period change.
fn fill_jittered(buf: &mut [f32], hz: f32, amp: f32, stretch: f32) {
    let base = SAMPLE_RATE as f32 / hz;
    let mut pos = 0.0f32;
    let mut cycle = base * (1.0 - stretch);
    let mut alt = false;
    for s in buf {
        *s = amp * (2.0 * PI * pos / cycle).sin();
        pos += 1.0;
        if pos >= cycle {
            pos -= cycle;
            alt = !alt;
            cycle = if alt {
                base * (1.0 + stretch)
            } else {
                base * (1.0 - stretch)
            };
        }
    }
}

/// Integer-period sine whose peak gain alternates.
fn fill_shimmered(buf: &mut [f32], hz: f32, gains: &[f32]) {
    let period = (SAMPLE_RATE as f32 / hz).round() as usize;
    let mut i = 0;
    let mut c = 0;
    while i < buf.len() {
        let g = gains[c % gains.len()];
        let n = period.min(buf.len() - i);
        for k in 0..n {
            let theta = 2.0 * PI * (k as f32) / period as f32;
            buf[i + k] = g * theta.sin();
        }
        i += n;
        c += 1;
    }
}

fn close_hz(got: f32, want: f32) {
    let tol = (want * 0.03).max(2.5);
    assert!(
        (got - want).abs() <= tol,
        "f0 {got} Hz, want {want} Hz (tol {tol})"
    );
}

#[test]
fn frame_matches_the_lock_and_the_core() {
    assert_eq!(FRAME_MS, 80);
    let core = EmotionCore::at_rest();
    assert_eq!(core.frames(), 0);
    assert_eq!(core.next_ms(), 0);
    assert_eq!(SAMPLE_RATE, 16_000);
    assert_eq!(FRAME_SAMPLES, 1_280);
    assert_eq!(horbm_ear::port::AXIS_NAME[VALENCE], "valence");
    assert_eq!(horbm_ear::port::AXIS_NAME[AROUSAL], "arousal");
}

#[test]
fn denied_sense_computes_and_stores_nothing() {
    let mut ear = Ear::from_env(Some("off"));
    let mut port = RamPort::default();
    assert!(!ear.sensing());
    assert_eq!(ear.frames_analysed(), 0);
    assert_eq!(ear.retained_samples(), 0);
    let mut loud = [0.0f32; FRAME_SAMPLES];
    fill_sine(&mut loud, 200.0, 0.25);
    for _ in 0..10 {
        assert!(matches!(ear.push_with(&loud, &mut port), FrameOut::Denied));
    }
    assert_eq!(port.takes, 0);
    assert!(port.last.is_none());
    assert_eq!(ear.frames_analysed(), 0);
    assert_eq!(ear.retained_samples(), 0);

    grant(&mut ear);
    let quiet = [0.0f32; FRAME_SAMPLES];
    let frame = heard(&mut ear, &quiet);
    assert_eq!(frame.pause_ms, FRAME_MS);
    assert_eq!(ear.frames_analysed(), 1);
    assert_eq!(ear.retained_samples(), 0);
}

#[test]
fn prohibited_purposes_stay_shut_with_the_sense_bit_set() {
    let mut tone = [0.0f32; FRAME_SAMPLES];
    fill_sine(&mut tone, 180.0, 0.2);
    for purpose in [Purpose::Workplace, Purpose::Education] {
        let mut ear = Ear::new();
        ear.set_grant(SENSE | ADAPT | REMEMBER, purpose);
        assert!(!ear.sensing());
        assert!(matches!(ear.push(&tone), FrameOut::Denied));
        assert_eq!(ear.frames_analysed(), 0);
        assert_eq!(ear.retained_samples(), 0);
    }
    let mut ear = Ear::new();
    ear.set_grant(SENSE, Purpose::MedicalOrSafety);
    assert!(ear.sensing());
    assert!(matches!(ear.push(&tone), FrameOut::Heard(_)));
}

#[test]
fn closing_the_grant_drops_the_pause() {
    let mut ear = Ear::new();
    grant(&mut ear);
    let quiet = [0.0f32; FRAME_SAMPLES];
    for n in 1..=5 {
        assert_eq!(heard(&mut ear, &quiet).pause_ms, n * FRAME_MS);
    }
    ear.deny();
    assert!(!ear.sensing());
    assert_eq!(ear.frames_analysed(), 0);
    assert!(matches!(ear.push(&quiet), FrameOut::Denied));
    grant(&mut ear);
    assert_eq!(heard(&mut ear, &quiet).pause_ms, FRAME_MS);
}

#[test]
fn remember_does_not_retain_samples() {
    let mut ear = Ear::new();
    ear.set_grant(SENSE | REMEMBER, Purpose::PersonalAssistant);
    let mut tone = [0.0f32; FRAME_SAMPLES];
    fill_sine(&mut tone, 160.0, 0.2);
    let frame = heard(&mut ear, &tone);
    assert!(frame.voiced);
    assert_eq!(ear.retained_samples(), 0);
    ear.deny();
    assert_eq!(ear.retained_samples(), 0);
    assert_eq!(ear.frames_analysed(), 0);
}

#[test]
fn a_bad_buffer_does_not_move_the_rings() {
    let mut ear = Ear::new();
    grant(&mut ear);
    let quiet = [0.0f32; FRAME_SAMPLES];
    assert_eq!(heard(&mut ear, &quiet).pause_ms, FRAME_MS);
    assert!(matches!(
        ear.push(&quiet[..FRAME_SAMPLES - 1]),
        FrameOut::Rejected
    ));
    let mut bad = quiet;
    bad[10] = f32::NAN;
    assert!(matches!(ear.push(&bad), FrameOut::Rejected));
    assert_eq!(ear.frames_analysed(), 1);
    assert_eq!(heard(&mut ear, &quiet).pause_ms, 2 * FRAME_MS);
}

#[test]
fn sine_sweep_tracks_f0_and_noise_does_not() {
    let targets = [90.0, 140.0, 180.0, 220.0, 270.0, 330.0];
    for hz in targets {
        let mut ear = Ear::new();
        grant(&mut ear);
        let mut buf = [0.0f32; FRAME_SAMPLES];
        fill_sine(&mut buf, hz, 0.2);
        let frame = heard(&mut ear, &buf);
        assert!(frame.voiced, "{hz} Hz was unvoiced");
        assert!(frame.voicing > 0.5, "{hz} clarity {}", frame.voicing);
        close_hz(frame.f0_hz, hz);
        let expect = 0.2 / 2.0f32.sqrt();
        assert!(
            (frame.energy - expect).abs() < 0.01,
            "energy {}",
            frame.energy
        );
    }

    let mut ear = Ear::new();
    grant(&mut ear);
    let mut noise = [0.0f32; FRAME_SAMPLES];
    fill_noise(&mut noise, 0x5eed_5eed_u64, 0.2);
    let frame = heard(&mut ear, &noise);
    assert!(!frame.voiced, "noise looked voiced at {} Hz", frame.f0_hz);
    assert_eq!(frame.f0_hz, 0.0);
    assert_eq!(frame.jitter, 0.0);
    assert_eq!(frame.shimmer, 0.0);

    let mut chirp = [0.0f32; FRAME_SAMPLES];
    fill_chirp(&mut chirp, 150.0, 280.0, 0.2);
    let swept = heard(&mut ear, &chirp);
    assert!(swept.energy.is_finite() && swept.energy > 0.05);
    assert!(swept.offer.point.iter().all(|v| v.is_finite()));
    assert!(swept.f0_hz.is_finite());
}

#[test]
fn pause_clears_on_a_tone_and_bursts_raise_the_rate() {
    let mut ear = Ear::new();
    grant(&mut ear);
    let quiet = [0.0f32; FRAME_SAMPLES];
    let mut tone = [0.0f32; FRAME_SAMPLES];
    fill_sine(&mut tone, 180.0, 0.22);
    for n in 1..=4 {
        assert_eq!(heard(&mut ear, &quiet).pause_ms, n * FRAME_MS);
    }
    let voiced = heard(&mut ear, &tone);
    assert_eq!(voiced.pause_ms, 0);
    assert!(voiced.voiced);

    let mut steady = Ear::new();
    grant(&mut steady);
    let mut bursty = Ear::new();
    grant(&mut bursty);
    let mut last_steady = 0.0;
    let mut last_bursty = 0.0;
    for i in 0..16 {
        last_steady = heard(&mut steady, &tone).speaking_rate_hz;
        let pcm = if i % 4 < 2 { &tone } else { &quiet };
        last_bursty = heard(&mut bursty, pcm).speaking_rate_hz;
    }
    assert!(
        last_bursty > last_steady + 1.5,
        "bursts {last_bursty} Hz, steady {last_steady} Hz"
    );
    assert!(
        last_steady < 1.0,
        "steady tone counted as speech at {last_steady}"
    );
}

#[test]
fn jitter_and_shimmer_rise_above_a_pure_sine() {
    let mut pure_ear = Ear::new();
    grant(&mut pure_ear);
    let mut pure = [0.0f32; FRAME_SAMPLES];
    fill_sine(&mut pure, 200.0, 0.2);
    let clean = heard(&mut pure_ear, &pure);

    let mut jit_ear = Ear::new();
    grant(&mut jit_ear);
    let mut warped = [0.0f32; FRAME_SAMPLES];
    fill_jittered(&mut warped, 200.0, 0.2, 0.08);
    let rough = heard(&mut jit_ear, &warped);
    assert!(rough.voiced, "jittered sine was unvoiced");
    assert!(
        rough.jitter > clean.jitter + 0.03,
        "jitter {} vs clean {}",
        rough.jitter,
        clean.jitter
    );

    let mut shim_ear = Ear::new();
    grant(&mut shim_ear);
    let mut alt = [0.0f32; FRAME_SAMPLES];
    fill_shimmered(&mut alt, 200.0, &[0.22, 0.10]);
    let dull = heard(&mut shim_ear, &alt);
    assert!(dull.voiced, "shimmered sine was unvoiced");
    assert!(
        dull.shimmer > clean.shimmer + 0.08,
        "shimmer {} vs clean {}",
        dull.shimmer,
        clean.shimmer
    );
    assert!(
        dull.offer.valence() < clean.offer.valence(),
        "rough valence {} clean {}",
        dull.offer.valence(),
        clean.offer.valence()
    );
}

#[test]
fn emotion_is_a_continuous_point_not_a_bucket() {
    let mut prev = -1.0f32;
    for step in 0..8 {
        let amp = 0.04 + step as f32 * 0.03;
        let mut ear = Ear::new();
        grant(&mut ear);
        let mut buf = [0.0f32; FRAME_SAMPLES];
        fill_sine(&mut buf, 180.0, amp);
        let frame = heard(&mut ear, &buf);
        let a = frame.offer.arousal();
        assert!(
            a > prev,
            "arousal stepped backward at amp {amp}: {a} <= {prev}"
        );
        assert!((a - prev) < 0.25 || prev < 0.0);
        prev = a;
        for (i, v) in frame.offer.point.iter().enumerate() {
            let (lo, hi) = if i == AROUSAL || i == horbm_ear::port::DOMINANCE {
                (0.0, 1.0)
            } else {
                (-1.0, 1.0)
            };
            assert!((lo..=hi).contains(v), "axis {i} = {v}");
        }
    }

    let mut quiet_ear = Ear::new();
    grant(&mut quiet_ear);
    let quiet = heard(&mut quiet_ear, &[0.0; FRAME_SAMPLES]);
    let mut loud_ear = Ear::new();
    grant(&mut loud_ear);
    let mut loud_buf = [0.0f32; FRAME_SAMPLES];
    fill_sine(&mut loud_buf, 220.0, 0.24);
    let loud = heard(&mut loud_ear, &loud_buf);
    assert!(loud.offer.arousal() > quiet.offer.arousal() + 0.2);
    assert!(quiet.offer.strength < loud.offer.strength);
}

#[test]
fn the_same_frame_is_bit_identical() {
    let mut a = Ear::new();
    let mut b = Ear::new();
    grant(&mut a);
    grant(&mut b);
    let mut buf = [0.0f32; FRAME_SAMPLES];
    fill_sine(&mut buf, 175.0, 0.18);
    fill_noise_onto(&mut buf, 0xA11CE, 0.01);
    let fa = heard(&mut a, &buf);
    let fb = heard(&mut b, &buf);
    assert_eq!(fa.f0_hz.to_bits(), fb.f0_hz.to_bits());
    assert_eq!(fa.energy.to_bits(), fb.energy.to_bits());
    assert_eq!(fa.jitter.to_bits(), fb.jitter.to_bits());
    assert_eq!(fa.shimmer.to_bits(), fb.shimmer.to_bits());
    assert_eq!(fa.offer.point[0].to_bits(), fb.offer.point[0].to_bits());
    assert_eq!(fa.offer.strength.to_bits(), fb.offer.strength.to_bits());
}

fn fill_noise_onto(buf: &mut [f32], seed: u64, amp: f32) {
    let mut x = seed;
    for s in buf.iter_mut() {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        let u = (x >> 40) as f32 / 16_777_216.0;
        *s += amp * (2.0 * u - 1.0);
    }
}

#[test]
fn the_offer_is_an_inbound_intent_and_the_ear_does_not_step_the_core() {
    let mut core = EmotionCore::at_rest();
    let mut ear = Ear::new();
    ear.set_grant(SENSE, Purpose::PersonalAssistant);
    let mut buf = [0.0f32; FRAME_SAMPLES];
    fill_sine(&mut buf, 190.0, 0.22);
    let frame = heard(&mut ear, &buf);
    assert_eq!(core.frames(), 0, "the ear must not step the core");
    assert_eq!(frame.offer.t_ms, 0);
    assert_eq!(frame.offer.consent, SENSE);

    let baseline = VoiceBaseline::standin();
    let msg = frame.offer.inbound_intent(baseline).expect("intent");
    let bytes = msg.encode().expect("encode");
    assert_eq!(bytes.len(), 72);
    assert_eq!(&bytes[0..4], b"GEMI");
    assert_eq!(msg.source, IntentSource::Inbound);
    assert_eq!(msg.verb_str(), "hear");
    assert_eq!(msg.factor_mask, 0);
    assert_eq!(msg.consent, SENSE);
    for i in 0..4 {
        let delta = frame.offer.point[i] - baseline.pos[i];
        assert!((msg.pull[i] - delta).abs() < 1.0e-5, "pull {i}");
    }
    let back = horbm_ear::port::IntentMsg::decode(&bytes).expect("decode");
    assert_eq!(back.verb_str(), msg.verb_str());
    assert_eq!(back.pull, msg.pull);

    core.step(&msg).expect("step");
    assert!(!core.inbound_mixed(), "sense alone must not mix the pull");

    let mut adapted = Ear::new();
    adapted.set_grant(SENSE | ADAPT, Purpose::Accessibility);
    let again = heard(&mut adapted, &buf);
    let mixed = again
        .offer
        .inbound_intent(VoiceBaseline::standin())
        .expect("intent");
    let mut core = EmotionCore::at_rest();
    core.step(&mixed).expect("step");
    assert!(core.inbound_mixed());
    assert!(again.offer.strength > 0.0);
}

#[test]
fn a_denied_port_stays_quiet_until_the_grant() {
    let mut ear = Ear::from_config(horbm_ear::EarConfig { sense: false });
    let mut port = RamPort::default();
    let mut buf = [0.0f32; FRAME_SAMPLES];
    fill_sine(&mut buf, 150.0, 0.2);
    assert!(matches!(ear.push_with(&buf, &mut port), FrameOut::Denied));
    assert_eq!(port.takes, 0);
    grant(&mut ear);
    let out = ear.push_with(&buf, &mut port);
    assert!(matches!(out, FrameOut::Heard(_)));
    assert_eq!(port.takes, 1);
    assert!(port.last.unwrap().arousal() > 0.1);
}

#[test]
fn one_frame_stays_inside_the_lock_budget() {
    let mut ear = Ear::new();
    grant(&mut ear);
    let mut buf = [0.0f32; FRAME_SAMPLES];
    fill_sine(&mut buf, 200.0, 0.2);
    for _ in 0..8 {
        let _ = ear.push(&buf);
    }
    let mut sum = 0u128;
    let mut max_us = 0u128;
    let n = 40u128;
    for _ in 0..n {
        let t = Instant::now();
        let _ = ear.push(&buf);
        let us = t.elapsed().as_micros();
        sum += us;
        if us > max_us {
            max_us = us;
        }
    }
    let mean = sum / n;
    eprintln!(
        "horbm-ear frame mean {mean} us max {max_us} us over {n} (debug or release, this host)"
    );
    assert!(
        max_us < 60_000,
        "max frame {max_us} us exceeds the 60 ms compute budget"
    );
}

#[test]
fn default_is_on_and_only_the_off_switch_closes_it() {
    assert!(Ear::new().sensing());
    assert!(Ear::default().sensing());
    assert!(EarConfig::default().sense);
    assert!(Ear::from_config(EarConfig::default()).sensing());
    assert!(Ear::from_env(None).sensing());
    assert!(Ear::from_env(Some("")).sensing());
    assert!(Ear::from_env(Some("1")).sensing());
    assert!(Ear::from_env(Some("on")).sensing());
    assert!(Ear::from_env(Some("ALLOW")).sensing());
    for off in ["0", "off", "deny", " OFF "] {
        let ear = Ear::from_env(Some(off));
        assert!(!ear.sensing(), "{off} left the ear open");
        assert_eq!(ear.frames_analysed(), 0);
    }
    assert!(!ACTIVITY_MARK.chars().any(|c| c.is_ascii_digit()));
}

fn offer_at(valence: f32, arousal: f32, strength: f32) -> horbm_ear::ListenerOffer {
    let mut point = [0.0f32; 4];
    point[VALENCE] = valence;
    point[AROUSAL] = arousal;
    horbm_ear::ListenerOffer {
        t_ms: 0,
        consent: SENSE,
        point,
        strength,
    }
}

#[test]
fn a_heard_frame_exposes_axes_and_a_denied_frame_does_not() {
    let core = EmotionCore::at_rest();
    assert_eq!(core.frames(), 0);

    let mut ear = Ear::new();
    assert!(ear.sensing(), "sensing starts on");
    let mut tone = [0.0f32; FRAME_SAMPLES];
    fill_sine(&mut tone, 180.0, 0.2);
    let frame = heard(&mut ear, &tone);
    assert!(frame.voiced, "a 180 Hz sine was unvoiced");
    close_hz(frame.f0_hz, 180.0);

    let axes = frame
        .offer
        .heard_axes()
        .expect("a heard frame exposes valence, arousal, and strength");
    assert_eq!(axes.0.to_bits(), frame.offer.valence().to_bits());
    assert_eq!(axes.1.to_bits(), frame.offer.arousal().to_bits());
    assert_eq!(axes.2.to_bits(), frame.offer.strength.to_bits());
    assert!(axes.0.is_finite() && (-1.0..=1.0).contains(&axes.0));
    assert!(axes.1.is_finite() && (0.0..=1.0).contains(&axes.1));
    assert!(axes.2.is_finite() && (0.0..=1.0).contains(&axes.2));

    let mut quiet_ear = Ear::new();
    let quiet = heard(&mut quiet_ear, &[0.0; FRAME_SAMPLES]);
    let quiet_axes = quiet
        .offer
        .heard_axes()
        .expect("a granted quiet frame still has a point");
    assert!(
        axes.1 > quiet_axes.1 + 0.2,
        "tone arousal {} quiet {}",
        axes.1,
        quiet_axes.1
    );
    assert!(
        axes.2 > quiet_axes.2,
        "tone strength {} quiet {}",
        axes.2,
        quiet_axes.2
    );
    assert_eq!(
        core.frames(),
        0,
        "heard_axes must not call EmotionCore::step"
    );

    for (valence, arousal, strength) in [
        (f32::NAN, 0.4, 0.5),
        (0.2, f32::NAN, 0.5),
        (0.2, 0.4, f32::NAN),
        (f32::INFINITY, 0.4, 0.5),
        (0.2, f32::NEG_INFINITY, 0.5),
        (0.2, 0.4, f32::INFINITY),
    ] {
        assert!(
            offer_at(valence, arousal, strength).heard_axes().is_none(),
            "non-finite ({valence}, {arousal}, {strength}) must be None"
        );
    }

    let mut aside = offer_at(0.2, 0.4, 0.5);
    aside.point[horbm_ear::port::DOMINANCE] = f32::NAN;
    aside.point[horbm_ear::port::APPROACH] = f32::INFINITY;
    assert_eq!(aside.heard_axes(), Some((0.2, 0.4, 0.5)));
    assert_eq!(core.frames(), 0);

    for off in ["0", "off", "deny"] {
        let mut shut = Ear::from_env(Some(off));
        assert!(!shut.sensing(), "{off} left the ear open");
        match shut.push(&tone) {
            FrameOut::Denied => {}
            other => panic!("{off} exposed a frame {other:?}"),
        }
        assert_eq!(shut.frames_analysed(), 0);
        assert_eq!(shut.retained_samples(), 0);
    }

    ear.deny();
    assert!(!ear.sensing());
    match ear.push(&tone) {
        FrameOut::Denied => {}
        other => panic!("a denied frame exposed {other:?}"),
    }
    assert_eq!(ear.frames_analysed(), 0);
    assert_eq!(core.frames(), 0);
}

struct CueLog {
    cues: Vec<FloorCue>,
}

impl FloorTick for CueLog {
    fn tick_floor(&mut self, cue: FloorCue) {
        self.cues.push(cue);
    }
}

#[test]
fn a_frame_reaches_the_core_and_the_floor() {
    let mut ear = Ear::new();
    let mut core = EmotionCore::at_rest();
    let mut floor = CueLog { cues: Vec::new() };
    let baseline = VoiceBaseline::standin();
    let mut tone = [0.0f32; FRAME_SAMPLES];
    fill_sine(&mut tone, 200.0, 0.22);
    let quiet = [0.0f32; FRAME_SAMPLES];

    for i in 0..2 {
        let out = push_frame(&mut ear, &mut core, &mut floor, baseline, &tone).expect("link");
        let frame = match out.frame {
            FrameOut::Heard(frame) => frame,
            other => panic!("expected a heard frame, got {other:?}"),
        };
        close_hz(frame.f0_hz, 200.0);
        assert!(frame.energy > 0.1);
        assert!(frame.voiced);
        assert!(
            frame.speaking_rate_hz < 1.0,
            "steady tone rate {}",
            frame.speaking_rate_hz
        );
        assert_eq!(out.moment.expect("moment").tick, i + 1);
        assert_eq!(out.cue, Some(FloorCue::Voiced));
    }
    assert_eq!(core.frames(), 2);
    assert!(!core.inbound_mixed(), "sense without adapt does not mix");

    for _ in 0..2 {
        let out = push_frame(&mut ear, &mut core, &mut floor, baseline, &quiet).expect("quiet");
        assert_eq!(out.cue, Some(FloorCue::Quiet));
        let frame = match out.frame {
            FrameOut::Heard(frame) => frame,
            other => panic!("expected a heard quiet frame, got {other:?}"),
        };
        assert!(frame.pause_ms >= FRAME_MS);
        assert!(out.moment.is_some());
    }
    assert_eq!(
        floor.cues,
        vec![
            FloorCue::Voiced,
            FloorCue::Voiced,
            FloorCue::Quiet,
            FloorCue::Quiet
        ]
    );
    assert_eq!(core.frames(), 4);

    let mut ear = Ear::from_env(Some("0"));
    let mut core = EmotionCore::at_rest();
    let mut floor = CueLog { cues: Vec::new() };
    for _ in 0..4 {
        let out = push_frame(&mut ear, &mut core, &mut floor, baseline, &tone).expect("off");
        assert!(matches!(out.frame, FrameOut::Denied));
        assert!(out.moment.is_none());
        assert_eq!(out.cue, Some(FloorCue::Quiet));
    }
    assert_eq!(core.frames(), 0);
    assert_eq!(ear.frames_analysed(), 0);
    assert_eq!(ear.retained_samples(), 0);
    assert_eq!(floor.cues, vec![FloorCue::Quiet; 4]);
}

#[test]
fn sources_do_not_open_a_store_or_a_socket() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    let rd = std::fs::read_dir(&root).expect("src");
    for ent in rd {
        let ent = ent.expect("entry");
        let name = ent.file_name();
        let name = name.to_string_lossy();
        if name.ends_with(".rs") {
            files.push(ent.path());
        }
    }
    assert!(files.len() >= 3);
    let banned = [
        "std::fs",
        "std::net",
        "File::",
        "TcpStream",
        "UdpSocket",
        "include_bytes!",
        ".wav",
        ".onnx",
        "ggml",
        "gguf",
        "127.0.0.1",
        "0.0.0.0",
    ];
    for path in files {
        let text = std::fs::read_to_string(&path).expect("read");
        for needle in banned {
            assert!(
                !text.contains(needle),
                "{} contains {needle}",
                path.display()
            );
        }
    }
}
