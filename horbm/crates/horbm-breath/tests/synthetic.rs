// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Synthetic plans only. No audio file and no recorded voice.

use horbm_breath::{
    plan_quiet, plan_speech, quiet_cycle, EmotionMoment, Error, Frame, Inside, Phase, Span, Track,
    VoiceBaseline, BPM_MAX, BPM_MIN, DIRECTIONS, FRAME_MS, NOT_LINKED, QUIET_PS_MAX, QUIET_PS_MIN,
    SCHEMA, SPEECH_OQ_MAX, SPEECH_OQ_MIN, SPEECH_PS_MAX, SPEECH_PS_MIN,
};

fn talk(frames: u32) -> Span {
    Span::talk(frames, 0.5, 0.0)
}

fn speaking(track: &Track) -> Vec<&Frame> {
    track.frames.iter().filter(|frame| frame.speaking).collect()
}

fn mean_ps(frames: &[&Frame]) -> f32 {
    frames
        .iter()
        .map(|frame| frame.subglottal_cm_h2o)
        .sum::<f32>()
        / frames.len() as f32
}

fn inhale_runs(track: &Track) -> usize {
    let mut runs = 0;
    let mut prev = false;
    for frame in &track.frames {
        let on = frame.phase == Phase::Inhale;
        if on && !prev {
            runs += 1;
        }
        prev = on;
    }
    runs
}

fn first_pause(track: &Track) -> &[Frame] {
    let start = track
        .frames
        .iter()
        .position(|frame| frame.pause)
        .expect("pause");
    let end = start
        + track.frames[start..]
            .iter()
            .take_while(|frame| frame.pause)
            .count();
    &track.frames[start..end]
}

fn in_eupnea(period_frames: u32) -> bool {
    let centi = 6_000_000 / (period_frames * FRAME_MS);
    let lo = (BPM_MIN * 100.0) as u32;
    let hi = (BPM_MAX * 100.0) as u32;
    (lo..=hi).contains(&centi)
}

fn assert_speech_shape(track: &Track) {
    assert!(!track.frames.is_empty());
    for (i, frame) in track.frames.iter().enumerate() {
        assert_eq!(frame.t_ms, i as u32 * FRAME_MS);
        assert!(frame.nasal_coupling >= 0.0 && frame.nasal_coupling <= 1.0);
        assert!(frame.mouth_openness >= 0.0 && frame.mouth_openness <= 1.0);
        assert!(frame.breathiness >= 0.0 && frame.breathiness <= 1.0);
        match frame.phase {
            Phase::Inhale => {
                assert!(!frame.speaking);
                assert!(frame.subglottal_cm_h2o < 0.0);
                assert!(frame.subglottal_cm_h2o >= -2.5);
                assert!(frame.airflow_ml_s < 0.0);
                assert!(frame.open_quotient >= 0.90);
            }
            Phase::Exhale => {
                assert!(frame.speaking);
                assert!(!frame.pause);
                assert!(frame.subglottal_cm_h2o >= SPEECH_PS_MIN);
                assert!(frame.subglottal_cm_h2o <= SPEECH_PS_MAX);
                assert!(frame.airflow_ml_s > 0.0);
                assert!(frame.open_quotient >= SPEECH_OQ_MIN);
                assert!(frame.open_quotient <= SPEECH_OQ_MAX);
            }
            Phase::Hold => {
                assert!(!frame.speaking);
                assert_eq!(frame.airflow_ml_s, 0.0);
                assert!(frame.subglottal_cm_h2o.abs() < 0.5);
                assert!(frame.open_quotient >= 0.90);
            }
        }
        if frame.pause {
            assert!(!frame.speaking);
            assert!(matches!(frame.phase, Phase::Inhale | Phase::Hold));
        }
    }
    let mut i = 0;
    while i < track.frames.len() {
        if !track.frames[i].pause {
            i += 1;
            continue;
        }
        let start = i;
        while i < track.frames.len() && track.frames[i].pause {
            i += 1;
        }
        if start > 0 {
            let prev = &track.frames[start - 1];
            assert!(prev.speaking && prev.phase == Phase::Exhale);
        }
        if i < track.frames.len() {
            let next = &track.frames[i];
            assert!(next.speaking && next.phase == Phase::Exhale);
        }
    }
    i = 0;
    while i < track.frames.len() {
        if !track.frames[i].speaking {
            i += 1;
            continue;
        }
        assert!(i > 0, "speech starts after an inhale or a hold");
        assert!(matches!(
            track.frames[i - 1].phase,
            Phase::Inhale | Phase::Hold
        ));
        while i < track.frames.len() && track.frames[i].speaking {
            assert_eq!(track.frames[i].phase, Phase::Exhale);
            i += 1;
        }
        if i < track.frames.len() {
            assert!(matches!(track.frames[i].phase, Phase::Inhale | Phase::Hold));
        }
    }
}

#[test]
fn quiet_cycles_stay_in_the_eupnea_band() {
    let cycle = quiet_cycle(VoiceBaseline::ADULT, Inside::REST, EmotionMoment::STILL).unwrap();
    assert_eq!(cycle.period_frames, 60);
    assert_eq!(cycle.inhale_frames, 20);
    assert_eq!(cycle.exhale_frames, 40);
    assert!(cycle.exhale_frames > cycle.inhale_frames);
    assert!(in_eupnea(cycle.period_frames));

    for period in [38, 60, 75] {
        for ie in [1.5, 2.0, 3.0] {
            let baseline = VoiceBaseline {
                quiet_period_frames: period,
                ie_ratio: ie,
                ..VoiceBaseline::ADULT
            };
            let got = quiet_cycle(baseline, Inside::REST, EmotionMoment::STILL).unwrap();
            assert!(got.exhale_frames > got.inhale_frames);
            assert!(in_eupnea(got.period_frames));
        }
    }

    let fast = quiet_cycle(
        VoiceBaseline::ADULT,
        Inside {
            health: 0.0,
            ..Inside::REST
        },
        EmotionMoment {
            arousal: 1.0,
            fear: 1.0,
            ..EmotionMoment::STILL
        },
    )
    .unwrap();
    assert!(fast.period_frames < 60);
    assert!(fast.exhale_frames > fast.inhale_frames);
    assert!(in_eupnea(fast.period_frames));
}

#[test]
fn quiet_frames_keep_the_cycle_and_the_signs() {
    let cycle = quiet_cycle(VoiceBaseline::ADULT, Inside::REST, EmotionMoment::STILL).unwrap();
    let track = plan_quiet(
        VoiceBaseline::ADULT,
        Inside::REST,
        EmotionMoment::STILL,
        cycle.period_frames * 3,
    )
    .unwrap();
    assert_eq!(track.frames.len(), 180);
    for frame in &track.frames {
        assert!(!frame.speaking && !frame.pause);
        assert_eq!(frame.breathiness, 1.0);
        assert!(frame.open_quotient >= 0.90);
        match frame.phase {
            Phase::Inhale => {
                assert!(frame.subglottal_cm_h2o <= -QUIET_PS_MIN);
                assert!(frame.subglottal_cm_h2o >= -QUIET_PS_MAX);
                assert!(frame.airflow_ml_s < 0.0);
            }
            Phase::Exhale => {
                assert!(frame.subglottal_cm_h2o >= QUIET_PS_MIN);
                assert!(frame.subglottal_cm_h2o <= QUIET_PS_MAX);
                assert!(frame.airflow_ml_s > 0.0);
            }
            Phase::Hold => panic!("quiet breathing has no hold"),
        }
    }
    for cycle_i in 0..3 {
        let at = cycle_i * cycle.period_frames as usize;
        assert!(track.frames[at..at + 20]
            .iter()
            .all(|frame| frame.phase == Phase::Inhale));
        assert!(track.frames[at + 20..at + 60]
            .iter()
            .all(|frame| frame.phase == Phase::Exhale));
    }
    let again = plan_quiet(
        VoiceBaseline::ADULT,
        Inside::REST,
        EmotionMoment::STILL,
        cycle.period_frames * 3,
    )
    .unwrap();
    assert_eq!(track.to_bytes(), again.to_bytes());
}

#[test]
fn a_pause_between_phrases_is_an_exhale_inhale_boundary() {
    let spans = [talk(20), Span::pause(8), talk(20)];
    let track = plan_speech(
        VoiceBaseline::ADULT,
        Inside::REST,
        EmotionMoment::STILL,
        &spans,
    )
    .unwrap();
    assert_speech_shape(&track);
    let pause = first_pause(&track);
    assert!(pause.len() >= 8);
    assert_eq!(pause[0].phase, Phase::Inhale);
    assert!(pause.iter().any(|frame| frame.phase == Phase::Hold));
    assert!(pause.iter().all(|frame| frame.phase != Phase::Exhale));
    let speech = speaking(&track);
    assert!(speech.len() > 8);
    assert!(speech[0].subglottal_cm_h2o > speech.last().unwrap().subglottal_cm_h2o);
    assert!(speech
        .iter()
        .all(|frame| frame.subglottal_cm_h2o > 5.0 && frame.subglottal_cm_h2o < 10.0));
    assert!(speech
        .iter()
        .all(|frame| frame.airflow_ml_s > 80.0 && frame.airflow_ml_s < 350.0));
}

#[test]
fn a_short_pause_grows_to_a_whole_inhale() {
    let spans = [talk(20), Span::pause(2), talk(20)];
    let track = plan_speech(
        VoiceBaseline::ADULT,
        Inside::REST,
        EmotionMoment::STILL,
        &spans,
    )
    .unwrap();
    assert_speech_shape(&track);
    let pause = first_pause(&track);
    assert!(pause.len() > 2);
    assert!(pause.iter().all(|frame| frame.phase == Phase::Inhale));
}

#[test]
fn trailing_silence_holds_after_the_exhale() {
    let track = plan_speech(
        VoiceBaseline::ADULT,
        Inside::REST,
        EmotionMoment::STILL,
        &[talk(20), Span::pause(5)],
    )
    .unwrap();
    assert_speech_shape(&track);
    let pause = first_pause(&track);
    assert_eq!(pause.len(), 5);
    assert!(pause.iter().all(|frame| frame.phase == Phase::Hold));
    assert!(track.frames.last().unwrap().pause);
}

#[test]
fn fatigue_lowers_speech_pressure() {
    let spans = [talk(20)];
    let rested = plan_speech(
        VoiceBaseline::ADULT,
        Inside::REST,
        EmotionMoment::STILL,
        &spans,
    )
    .unwrap();
    let tired = plan_speech(
        VoiceBaseline::ADULT,
        Inside {
            fatigue: 1.0,
            ..Inside::REST
        },
        EmotionMoment::STILL,
        &spans,
    )
    .unwrap();
    let fresh = speaking(&rested);
    let spent = speaking(&tired);
    assert_eq!(fresh.len(), spent.len());
    assert!(fresh.len() > 4);
    for (a, b) in fresh.iter().zip(&spent) {
        assert!(b.subglottal_cm_h2o < a.subglottal_cm_h2o);
    }
    assert!(mean_ps(&fresh) - mean_ps(&spent) > 1.0);
    assert_ne!(rested.to_bytes(), tired.to_bytes());
}

#[test]
fn energy_raises_pressure_and_low_health_raises_breathiness() {
    let spans = [talk(20)];
    let rest = plan_speech(
        VoiceBaseline::ADULT,
        Inside::REST,
        EmotionMoment::STILL,
        &spans,
    )
    .unwrap();
    let driven = plan_speech(
        VoiceBaseline::ADULT,
        Inside {
            energy: 1.0,
            ..Inside::REST
        },
        EmotionMoment::STILL,
        &spans,
    )
    .unwrap();
    let unwell = plan_speech(
        VoiceBaseline::ADULT,
        Inside {
            health: 0.0,
            ..Inside::REST
        },
        EmotionMoment::STILL,
        &spans,
    )
    .unwrap();
    for (a, b) in speaking(&rest).iter().zip(speaking(&driven).iter()) {
        assert!(b.subglottal_cm_h2o > a.subglottal_cm_h2o);
    }
    for (a, b) in speaking(&rest).iter().zip(speaking(&unwell).iter()) {
        assert!(b.subglottal_cm_h2o < a.subglottal_cm_h2o);
        assert!(b.open_quotient > a.open_quotient);
        assert!(b.breathiness > a.breathiness);
    }
}

#[test]
fn amusement_shortens_the_inhale_and_disgust_holds() {
    let spans = [talk(20)];
    let still = plan_speech(
        VoiceBaseline::ADULT,
        Inside::REST,
        EmotionMoment::STILL,
        &spans,
    )
    .unwrap();
    let amused = plan_speech(
        VoiceBaseline::ADULT,
        Inside::REST,
        EmotionMoment {
            amusement: 1.0,
            ..EmotionMoment::STILL
        },
        &spans,
    )
    .unwrap();
    let disgust = plan_speech(
        VoiceBaseline::ADULT,
        Inside::REST,
        EmotionMoment {
            disgust: 1.0,
            ..EmotionMoment::STILL
        },
        &spans,
    )
    .unwrap();
    let lead = |track: &Track| {
        track
            .frames
            .iter()
            .position(|frame| frame.speaking)
            .unwrap()
    };
    assert!(lead(&amused) < lead(&still));
    assert!(lead(&disgust) > lead(&still));
    assert!(disgust.frames[lead(&still)..lead(&disgust)]
        .iter()
        .all(|frame| frame.phase == Phase::Hold));
    let still_ps: Vec<f32> = speaking(&still)
        .iter()
        .map(|frame| frame.subglottal_cm_h2o)
        .collect();
    let disgust_ps: Vec<f32> = speaking(&disgust)
        .iter()
        .map(|frame| frame.subglottal_cm_h2o)
        .collect();
    assert_eq!(still_ps.len(), disgust_ps.len());
    for (a, b) in still_ps.iter().zip(&disgust_ps) {
        assert_eq!(a.to_bits(), b.to_bits());
    }
}

#[test]
fn fear_adds_breath_groups_and_negative_arousal_does_not() {
    let spans = [Span::talk(100, 0.5, 0.0)];
    let still = plan_speech(
        VoiceBaseline::ADULT,
        Inside::REST,
        EmotionMoment::STILL,
        &spans,
    )
    .unwrap();
    let afraid = plan_speech(
        VoiceBaseline::ADULT,
        Inside::REST,
        EmotionMoment {
            fear: 1.0,
            ..EmotionMoment::STILL
        },
        &spans,
    )
    .unwrap();
    let low = plan_speech(
        VoiceBaseline::ADULT,
        Inside::REST,
        EmotionMoment {
            arousal: -1.0,
            ..EmotionMoment::STILL
        },
        &spans,
    )
    .unwrap();
    assert!(inhale_runs(&afraid) > inhale_runs(&still));
    assert_eq!(inhale_runs(&low), inhale_runs(&still));
    assert_eq!(low.to_bytes(), still.to_bytes());
}

#[test]
fn nasal_coupling_follows_the_caller_and_falls_when_unwell() {
    let oral = plan_speech(
        VoiceBaseline::ADULT,
        Inside::REST,
        EmotionMoment::STILL,
        &[Span::talk(12, 0.5, 0.0)],
    )
    .unwrap();
    let nasal = plan_speech(
        VoiceBaseline::ADULT,
        Inside::REST,
        EmotionMoment::STILL,
        &[Span::talk(12, 0.4, 1.0)],
    )
    .unwrap();
    let sick = plan_speech(
        VoiceBaseline::ADULT,
        Inside {
            health: 0.0,
            ..Inside::REST
        },
        EmotionMoment::STILL,
        &[Span::talk(12, 0.4, 1.0)],
    )
    .unwrap();
    assert!(speaking(&oral)
        .iter()
        .all(|frame| frame.nasal_coupling < 0.05));
    assert!(speaking(&nasal)
        .iter()
        .all(|frame| frame.nasal_coupling > 0.9));
    let well = speaking(&nasal)[0].nasal_coupling;
    let ill = speaking(&sick)[0].nasal_coupling;
    assert!(ill < well);
    let tired_mouth = plan_speech(
        VoiceBaseline::ADULT,
        Inside {
            fatigue: 1.0,
            ..Inside::REST
        },
        EmotionMoment::STILL,
        &[Span::talk(12, 0.5, 0.0)],
    )
    .unwrap();
    assert!(speaking(&tired_mouth)[0].mouth_openness < speaking(&oral)[0].mouth_openness);
}

#[test]
fn the_same_input_is_the_same_bytes_and_the_baseline_stays_put() {
    let baseline = VoiceBaseline::ADULT;
    let spans = [talk(16), Span::pause(4), talk(16)];
    let once = plan_speech(baseline, Inside::REST, EmotionMoment::STILL, &spans).unwrap();
    let twice = plan_speech(baseline, Inside::REST, EmotionMoment::STILL, &spans).unwrap();
    assert_eq!(once.to_bytes(), twice.to_bytes());
    assert_eq!(&once.to_bytes()[..4], b"HB01");
    assert!(SCHEMA.starts_with("horbm-breath/"));
    let back = Track::from_bytes(&once.to_bytes()).unwrap();
    assert_eq!(back.to_bytes(), once.to_bytes());
    for (a, b) in once.frames.iter().zip(&back.frames) {
        assert_eq!(a.t_ms, b.t_ms);
        assert_eq!(a.phase, b.phase);
        assert_eq!(a.speaking, b.speaking);
        assert_eq!(a.pause, b.pause);
        assert_eq!(a.subglottal_cm_h2o.to_bits(), b.subglottal_cm_h2o.to_bits());
        assert_eq!(a.airflow_ml_s.to_bits(), b.airflow_ml_s.to_bits());
        assert_eq!(a.open_quotient.to_bits(), b.open_quotient.to_bits());
        assert_eq!(a.breathiness.to_bits(), b.breathiness.to_bits());
        assert_eq!(a.nasal_coupling.to_bits(), b.nasal_coupling.to_bits());
        assert_eq!(a.mouth_openness.to_bits(), b.mouth_openness.to_bits());
    }
    let moved = plan_speech(
        baseline,
        Inside::REST,
        EmotionMoment {
            arousal: 0.6,
            amusement: 0.2,
            ..EmotionMoment::STILL
        },
        &spans,
    )
    .unwrap();
    assert_ne!(once.to_bytes(), moved.to_bytes());
    assert_eq!(baseline, VoiceBaseline::ADULT);
    assert!(Track::from_bytes(b"RIFF").is_err());
    assert!(once.to_bytes().windows(4).all(|w| w != b"RIFF"));
}

#[test]
fn a_low_anchor_under_fatigue_stays_on_the_speech_floor() {
    let baseline = VoiceBaseline {
        speech_ps_cm_h2o: 5.0,
        ..VoiceBaseline::ADULT
    };
    let track = plan_speech(
        baseline,
        Inside {
            energy: 0.0,
            fatigue: 1.0,
            health: 0.0,
        },
        EmotionMoment::STILL,
        &[talk(12)],
    )
    .unwrap();
    assert_speech_shape(&track);
    assert!(speaking(&track)
        .iter()
        .all(|frame| frame.subglottal_cm_h2o == SPEECH_PS_MIN));
}

#[test]
fn bad_inputs_are_refused_and_an_empty_plan_is_stable() {
    let nan = plan_speech(
        VoiceBaseline::ADULT,
        Inside {
            energy: f32::NAN,
            ..Inside::REST
        },
        EmotionMoment::STILL,
        &[talk(4)],
    );
    assert!(matches!(nan, Err(Error::NonFinite("energy"))));
    let high = plan_speech(
        VoiceBaseline::ADULT,
        Inside {
            fatigue: 1.5,
            ..Inside::REST
        },
        EmotionMoment::STILL,
        &[talk(4)],
    );
    assert!(matches!(high, Err(Error::OutOfRange("fatigue"))));
    let mouth = plan_speech(
        VoiceBaseline::ADULT,
        Inside::REST,
        EmotionMoment::STILL,
        &[Span::talk(4, 1.2, 0.0)],
    );
    assert!(matches!(mouth, Err(Error::OutOfRange("mouth"))));
    let long = plan_speech(
        VoiceBaseline::ADULT,
        Inside::REST,
        EmotionMoment::STILL,
        &[Span::talk(8_000, 0.5, 0.0)],
    );
    assert!(matches!(long, Err(Error::TooLong)));
    let period = quiet_cycle(
        VoiceBaseline {
            quiet_period_frames: 10,
            ..VoiceBaseline::ADULT
        },
        Inside::REST,
        EmotionMoment::STILL,
    );
    assert!(matches!(
        period,
        Err(Error::OutOfRange("quiet_period_frames"))
    ));

    let empty = plan_speech(
        VoiceBaseline::ADULT,
        Inside::REST,
        EmotionMoment::STILL,
        &[],
    )
    .unwrap();
    assert!(empty.frames.is_empty());
    let bytes = empty.to_bytes();
    assert_eq!(bytes.len(), 12);
    assert_eq!(
        bytes,
        plan_speech(
            VoiceBaseline::ADULT,
            Inside::REST,
            EmotionMoment::STILL,
            &[],
        )
        .unwrap()
        .to_bytes()
    );
    assert!(Track::from_bytes(&bytes[..11]).is_err());
}

#[test]
fn the_manifest_does_not_pull_audio_or_an_emotion_crate() {
    let manifest = include_str!("../Cargo.toml");
    assert!(manifest.contains("[dependencies]"));
    let tail = manifest.split("[dependencies]").nth(1).unwrap();
    assert!(tail.trim().is_empty());
    assert!(!manifest.to_ascii_lowercase().contains("emotion"));
    assert!(!manifest.contains("organpool"));
    assert!(!manifest.contains("phoneme"));
    assert!(!manifest.contains(".wav"));
    assert!(!manifest.contains(".exe"));
    assert!(DIRECTIONS.iter().any(|line| line.contains("Hixon")));
    assert!(DIRECTIONS.iter().any(|line| line.contains("Fant")));
    assert!(NOT_LINKED.iter().any(|line| line.contains("organpool")));
}
