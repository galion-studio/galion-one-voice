// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Listener readings from generated sines. No audio file is read or written.

use std::time::Instant;

use horbm_ear::consent_bits::{REMEMBER, SENSE};
use horbm_ear::link::{FloorCue, FloorTick};
use horbm_ear::listen::{
    hear_frame, response_intent, ListenerFeel, DIMENSIONS, PERSONA, RESPONSE_VERB, UNIVERSAL,
};
use horbm_ear::port::{EmotionCore, IntentMsg, IntentSource, VoiceBaseline, AROUSAL, VALENCE};
use horbm_ear::{
    Ear, EarConfig, FrameOut, ProsodyFrame, Purpose, FRAME_MS, FRAME_SAMPLES, SAMPLE_RATE,
};

const PI: f32 = std::f32::consts::PI;
const N: usize = 25;

#[derive(Clone, Copy)]
enum Cue {
    Sad,
    Excited,
    Stressed,
    Flat,
    Neutral,
}

impl Cue {
    fn words(self) -> &'static str {
        match self {
            Self::Flat => "I'm fine",
            _ => "",
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Sad => "sad-low-slow",
            Self::Excited => "excited-high-fast",
            Self::Stressed => "stressed-tense",
            Self::Flat => "flat-im-fine",
            Self::Neutral => "neutral",
        }
    }
}

struct Sink;

impl FloorTick for Sink {
    fn tick_floor(&mut self, _cue: FloorCue) {}
}

fn fill_sine(buf: &mut [f32], hz: f32, amp: f32) {
    let step = 2.0 * PI * hz / SAMPLE_RATE as f32;
    let mut phase = 0.0f32;
    for s in buf {
        *s = amp * phase.sin();
        phase += step;
    }
}

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

fn render(cue: Cue, index: usize, buf: &mut [f32]) {
    buf.fill(0.0);
    match cue {
        Cue::Sad => {
            if index % 8 < 2 {
                fill_sine(buf, 100.0, 0.045);
            }
        }
        Cue::Excited => {
            if index % 4 < 2 {
                fill_sine(buf, 260.0, 0.22);
            }
        }
        Cue::Stressed => fill_jittered(buf, 220.0, 0.18, 0.10),
        Cue::Flat => fill_sine(buf, 105.0, 0.04),
        Cue::Neutral => fill_sine(buf, 176.0, 0.135),
    }
}

struct Taken {
    frame: ProsodyFrame,
    feel: ListenerFeel,
    moment_v: f32,
}

fn drive(cue: Cue) -> Taken {
    let mut ear = Ear::new();
    let mut core = EmotionCore::at_rest();
    let mut floor = Sink;
    let mut buf = [0.0f32; FRAME_SAMPLES];
    let mut last = None;
    for i in 0..N {
        render(cue, i, &mut buf);
        let out = hear_frame(&mut ear, &mut core, &mut floor, &buf, cue.words()).expect("hear");
        let FrameOut::Heard(frame) = out.frame else {
            panic!("expected a heard frame");
        };
        let moment = out.moment.expect("step");
        last = Some(Taken {
            frame,
            feel: out.listener,
            moment_v: moment.pos[VALENCE],
        });
    }
    last.expect("frames")
}

fn pole(feel: &ListenerFeel, id: &str) -> f32 {
    UNIVERSAL
        .iter()
        .zip(feel.scores)
        .find(|(p, _)| p.id == id)
        .map(|(_, score)| score)
        .unwrap_or_else(|| panic!("missing pole {id}"))
}

fn heard_frame(out_frame: FrameOut) -> ProsodyFrame {
    match out_frame {
        FrameOut::Heard(frame) => frame,
        other => panic!("expected a heard frame, got {other:?}"),
    }
}

#[test]
fn dimensions_are_axes_not_a_category_list() {
    assert_eq!(PERSONA, "samantha");
    assert_eq!(RESPONSE_VERB, "with");
    assert_eq!(
        DIMENSIONS,
        ["valence", "arousal", "tension", "fatigue", "confidence"]
    );
    for name in ["sad", "happy", "angry", "fear", "aria", "jarvis"] {
        assert!(
            !DIMENSIONS.contains(&name),
            "{name} is not a listener dimension"
        );
    }
}

#[test]
fn cases_separate_and_flat_fine_follows_the_tone() {
    let sad = drive(Cue::Sad);
    let excited = drive(Cue::Excited);
    let stressed = drive(Cue::Stressed);
    let flat = drive(Cue::Flat);
    let neutral = drive(Cue::Neutral);
    let excited_words = {
        let mut ear = Ear::new();
        let mut core = EmotionCore::at_rest();
        let mut floor = Sink;
        let mut buf = [0.0f32; FRAME_SAMPLES];
        let mut last = None;
        for i in 0..N {
            render(Cue::Excited, i, &mut buf);
            let out = hear_frame(&mut ear, &mut core, &mut floor, &buf, "I'm fine").expect("hear");
            last = Some(out.listener);
        }
        last.expect("frames")
    };

    for (name, taken) in [
        (Cue::Sad.name(), &sad),
        (Cue::Excited.name(), &excited),
        (Cue::Stressed.name(), &stressed),
        (Cue::Flat.name(), &flat),
        (Cue::Neutral.name(), &neutral),
    ] {
        let f = &taken.feel;
        eprintln!(
            "{name:20} f0 {:>6.1} Hz  e {:>6.3}  rate {:>5.2}  pause {:>4} ms  jit {:>6.3}  shim {:>6.3}  v {:>6.3} ({:.2})  a {:>5.3}  ten {:>5.3}  fat {:>5.3}  conf {:>5.3}  led {}",
            taken.frame.f0_hz,
            taken.frame.energy,
            taken.frame.speaking_rate_hz,
            taken.frame.pause_ms,
            taken.frame.jitter,
            taken.frame.shimmer,
            f.valence.value,
            f.valence.confidence,
            f.arousal.value,
            f.tension.value,
            f.fatigue.value,
            f.confidence.value,
            f.prosody_led,
        );
    }

    assert!(sad.frame.voiced, "sad sine was unvoiced");
    assert!(excited.frame.voiced, "excited sine was unvoiced");
    assert!(stressed.frame.voiced, "stressed sine was unvoiced");
    assert!(flat.frame.voiced, "flat sine was unvoiced");
    assert!(neutral.frame.voiced, "neutral sine was unvoiced");

    assert!(
        sad.frame.f0_hz < neutral.frame.f0_hz,
        "sad f0 {}",
        sad.frame.f0_hz
    );
    assert!(excited.frame.f0_hz > neutral.frame.f0_hz);
    assert!(sad.frame.energy < neutral.frame.energy);
    assert!(excited.frame.energy > neutral.frame.energy);
    assert!(
        excited.frame.speaking_rate_hz > sad.frame.speaking_rate_hz + 1.0,
        "fast {} slow {}",
        excited.frame.speaking_rate_hz,
        sad.frame.speaking_rate_hz
    );

    assert!(
        sad.feel.valence.value < -0.2,
        "sad v {}",
        sad.feel.valence.value
    );
    assert!(
        sad.feel.arousal.value < 0.42,
        "sad a {}",
        sad.feel.arousal.value
    );
    assert!(
        sad.feel.fatigue.value > 0.4,
        "sad fat {}",
        sad.feel.fatigue.value
    );
    assert!(
        sad.feel.tension.value < 0.25,
        "sad ten {}",
        sad.feel.tension.value
    );

    assert!(
        excited.feel.valence.value > 0.2,
        "excited v {}",
        excited.feel.valence.value
    );
    assert!(
        excited.feel.arousal.value > 0.65,
        "excited a {}",
        excited.feel.arousal.value
    );
    assert!(excited.feel.fatigue.value < 0.2);
    assert!(excited.feel.tension.value < stressed.feel.tension.value);

    assert!(
        stressed.feel.tension.value > excited.feel.tension.value + 0.12,
        "stressed {} excited {}",
        stressed.feel.tension.value,
        excited.feel.tension.value
    );
    assert!(stressed.feel.tension.value > sad.feel.tension.value + 0.12);
    assert!(stressed.feel.tension.value > neutral.feel.tension.value + 0.12);
    assert!(stressed.feel.fatigue.value < sad.feel.fatigue.value);
    assert!(stressed.feel.arousal.value > neutral.feel.arousal.value);

    assert!(
        neutral.feel.valence.value.abs() < 0.15,
        "neutral v {}",
        neutral.feel.valence.value
    );
    assert!(
        (0.38..=0.62).contains(&neutral.feel.arousal.value),
        "neutral a {}",
        neutral.feel.arousal.value
    );
    assert!(neutral.feel.fatigue.value < sad.feel.fatigue.value);
    assert!(neutral.feel.tension.value < stressed.feel.tension.value);
    assert!(!neutral.feel.prosody_led);

    assert!(flat.feel.prosody_led, "flat 'I'm fine' kept the word");
    assert!(
        flat.feel.valence.value < 0.0,
        "flat v {}",
        flat.feel.valence.value
    );
    assert!(flat.feel.valence.value < excited_words.valence.value);
    assert!(!excited_words.prosody_led);
    assert!(excited_words.valence.value > 0.15);
    assert!(flat.feel.confidence.value < neutral.feel.confidence.value);
    assert!(
        flat.feel.valence.confidence > 0.4,
        "the low tone is evidence"
    );

    assert!(sad.feel.valence.value < neutral.feel.valence.value);
    assert!(neutral.feel.valence.value < excited.feel.valence.value);
    assert!(sad.feel.arousal.value < neutral.feel.arousal.value);
    assert!(neutral.feel.arousal.value < excited.feel.arousal.value);

    assert!(pole(&sad.feel, "sad") > pole(&sad.feel, "excited"));
    assert!(pole(&excited.feel, "excited") > pole(&excited.feel, "sad"));
    assert!(pole(&neutral.feel, "neutral") > pole(&neutral.feel, "angry"));
    let sum: f32 = sad.feel.scores.iter().sum();
    assert!(sum > 1.2, "scores collapsed toward one category, sum {sum}");

    assert!(
        (sad.moment_v - sad.feel.valence.value).abs() > 0.15,
        "her moment {} copied the listener {}",
        sad.moment_v,
        sad.feel.valence.value
    );
}

#[test]
fn sustained_cues_move_her_without_touching_baseline_or_body() {
    let baseline = VoiceBaseline::standin();
    let mut sad_core = EmotionCore::from_baseline(baseline);
    let mut hot_core = EmotionCore::from_baseline(baseline);
    let mut hold = EmotionCore::from_baseline(baseline);
    let sad_bits = sad_core.baseline().identity_bits();
    let fatigue = sad_core.inside().fatigue;
    let mut sad_ear = Ear::new();
    let mut hot_ear = Ear::new();
    let mut floor = Sink;
    let mut low = [0.0f32; FRAME_SAMPLES];
    let mut high = [0.0f32; FRAME_SAMPLES];
    fill_sine(&mut low, 100.0, 0.045);
    fill_sine(&mut high, 260.0, 0.22);

    let mut sad_feel = ListenerFeel::neutral();
    let mut hot_feel = ListenerFeel::neutral();
    for i in 0..8 {
        let sad = hear_frame(&mut sad_ear, &mut sad_core, &mut floor, &low, "").expect("sad");
        let hot = hear_frame(&mut hot_ear, &mut hot_core, &mut floor, &high, "").expect("hot");
        sad_feel = sad.listener;
        hot_feel = hot.listener;
        let msg = IntentMsg::hold(i * FRAME_MS);
        hold.step(&msg).expect("hold");
        assert!(!sad_core.inbound_mixed());
        assert!(!hot_core.inbound_mixed());
    }

    let sad_m = sad_core.moment();
    let hot_m = hot_core.moment();
    let hold_m = hold.moment();
    eprintln!(
        "after 8 sustained frames  sad valence {:+.3} hold {:+.3}  excited arousal {:.3} hold {:.3}",
        sad_m.pos[VALENCE], hold_m.pos[VALENCE], hot_m.pos[AROUSAL], hold_m.pos[AROUSAL]
    );
    assert!(
        sad_m.pos[VALENCE] < hold_m.pos[VALENCE] - 0.01,
        "sad {} hold {}",
        sad_m.pos[VALENCE],
        hold_m.pos[VALENCE]
    );
    assert!(
        hot_m.pos[AROUSAL] > hold_m.pos[AROUSAL] + 0.01,
        "hot {} hold {}",
        hot_m.pos[AROUSAL],
        hold_m.pos[AROUSAL]
    );
    assert!(sad_m.pos[VALENCE] < hot_m.pos[VALENCE]);
    assert!((sad_m.pos[VALENCE] - sad_feel.valence.value).abs() > 0.15);
    assert!((hot_m.pos[AROUSAL] - hot_feel.arousal.value).abs() > 0.05);
    assert_eq!(sad_core.baseline().identity_bits(), sad_bits);
    assert_eq!(hot_core.baseline().identity_bits(), sad_bits);
    assert_eq!(
        sad_core.baseline().pitch_centre_hz.to_bits(),
        216.0f32.to_bits()
    );
    assert_eq!(sad_core.inside().fatigue.to_bits(), fatigue.to_bits());
    assert_eq!(hot_core.inside().fatigue.to_bits(), fatigue.to_bits());
    assert!(sad_core.persisted_baseline().is_none());
    assert_eq!(sad_core.consent() & REMEMBER, 0);
    assert_eq!(sad_core.consent(), SENSE);
    assert_eq!(sad_core.frames(), 8);
}

#[test]
fn a_cue_moves_her_within_two_frames() {
    let (sad_ms, sad_frame) = reaction(false);
    let (hot_ms, hot_frame) = reaction(true);
    eprintln!(
        "reaction sad {sad_ms} ms ({sad_frame} frames) excited {hot_ms} ms ({hot_frame} frames)"
    );
    assert!(sad_ms <= 160, "sad reaction {sad_ms} ms");
    assert!(hot_ms <= 160, "excited reaction {hot_ms} ms");
    assert!(sad_frame <= 2 && hot_frame <= 2);
}

fn reaction(excited: bool) -> (u32, u32) {
    const WARM: usize = 16;
    const TAIL: usize = 8;
    let mut ear = Ear::new();
    let mut core = EmotionCore::at_rest();
    let mut hold = EmotionCore::at_rest();
    let mut floor = Sink;
    let mut buf = [0.0f32; FRAME_SAMPLES];
    let mut prev = 0.0f32;
    let axis = if excited { AROUSAL } else { VALENCE };
    for i in 0..(WARM + TAIL) {
        if i < WARM {
            render(Cue::Neutral, i, &mut buf);
        } else if excited {
            fill_sine(&mut buf, 260.0, 0.22);
        } else {
            fill_sine(&mut buf, 100.0, 0.045);
        }
        let out = hear_frame(&mut ear, &mut core, &mut floor, &buf, "").expect("hear");
        let moment = out.moment.expect("step");
        let control = hold
            .step(&IntentMsg::hold(i as u32 * FRAME_MS))
            .expect("hold");
        let gap = moment.pos[axis] - control.pos[axis];
        if i >= WARM {
            let moved = (gap - prev).abs();
            if moved > 0.002 {
                let frames = (i - WARM + 1) as u32;
                return (frames * FRAME_MS, frames);
            }
        }
        prev = gap;
    }
    (u32::MAX, u32::MAX)
}

#[test]
fn the_off_switch_is_neutral_and_does_not_step() {
    let mut loud = [0.0f32; FRAME_SAMPLES];
    fill_sine(&mut loud, 260.0, 0.22);
    let neutral = ListenerFeel::neutral();

    let mut denied = Ear::new();
    denied.deny();
    expect_off(&mut denied, &loud);

    let mut configured = Ear::from_config(EarConfig { sense: false });
    expect_off(&mut configured, &loud);

    for word in ["0", "off", "deny", " OFF "] {
        let mut ear = Ear::from_env(Some(word));
        expect_off(&mut ear, &loud);
    }

    let mut work = Ear::new();
    work.set_grant(SENSE | REMEMBER, Purpose::Workplace);
    expect_off(&mut work, &loud);
    let mut school = Ear::new();
    school.set_grant(SENSE, Purpose::Education);
    expect_off(&mut school, &loud);

    assert!(neutral.unclaimed());
    assert!(neutral.valence.value.abs() < 1.0e-6);
    assert!((neutral.arousal.value - 0.5).abs() < 1.0e-6);
}

fn expect_off(ear: &mut Ear, pcm: &[f32]) {
    assert!(!ear.sensing());
    let mut core = EmotionCore::at_rest();
    let bits = core.baseline().identity_bits();
    let start = core.moment();
    let mut floor = CueLog { cues: Vec::new() };
    for _ in 0..4 {
        let out = hear_frame(ear, &mut core, &mut floor, pcm, "I'm fine").expect("off");
        assert!(matches!(out.frame, FrameOut::Denied));
        assert_eq!(out.listener, ListenerFeel::neutral());
        assert!(out.moment.is_none());
        assert_eq!(out.cue, Some(FloorCue::Quiet));
    }
    assert_eq!(core.frames(), 0);
    assert_eq!(core.moment(), start);
    assert_eq!(core.baseline().identity_bits(), bits);
    assert!(core.persisted_baseline().is_none());
    assert_eq!(ear.frames_analysed(), 0);
    assert_eq!(ear.retained_samples(), 0);
    assert_eq!(floor.cues, vec![FloorCue::Quiet; 4]);
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
fn silence_while_sensing_does_not_pull_and_a_bad_buffer_does_not_step() {
    let mut ear = Ear::new();
    let mut core = EmotionCore::at_rest();
    let mut hold = EmotionCore::at_rest();
    let mut floor = Sink;
    let quiet = [0.0f32; FRAME_SAMPLES];
    for i in 0..6 {
        let out = hear_frame(&mut ear, &mut core, &mut floor, &quiet, "I'm fine").expect("quiet");
        assert_eq!(out.listener, ListenerFeel::neutral());
        let moment = out.moment.expect("quiet still steps");
        let control = hold.step(&IntentMsg::hold(i * FRAME_MS)).expect("hold");
        assert!((moment.pos[VALENCE] - control.pos[VALENCE]).abs() < 1.0e-4);
        assert!((moment.pos[AROUSAL] - control.pos[AROUSAL]).abs() < 1.0e-4);
    }

    let before = core.frames();
    let rejected = hear_frame(&mut ear, &mut core, &mut floor, &[0.0; 4], "I'm fine").expect("bad");
    assert!(matches!(rejected.frame, FrameOut::Rejected));
    assert_eq!(rejected.listener, ListenerFeel::neutral());
    assert!(rejected.moment.is_none());
    assert_eq!(core.frames(), before);
}

#[test]
fn remember_on_the_ear_does_not_store_the_listener() {
    let mut ear = Ear::new();
    ear.set_grant(SENSE | REMEMBER, Purpose::PersonalAssistant);
    let mut core = EmotionCore::at_rest();
    let mut floor = Sink;
    let mut buf = [0.0f32; FRAME_SAMPLES];
    fill_sine(&mut buf, 180.0, 0.16);
    let out = hear_frame(&mut ear, &mut core, &mut floor, &buf, "I'm fine").expect("hear");
    let msg_consent = core.consent();
    assert_eq!(msg_consent & REMEMBER, 0);
    assert!(core.persisted_baseline().is_none());
    assert_eq!(ear.retained_samples(), 0);
    assert!(out.listener.valence.confidence > 0.0);
    let heard = heard_frame(out.frame);
    assert!(heard.voiced);
    assert_eq!(out.moment.expect("moment").pos.len(), 4);
}

#[test]
fn her_response_is_her_own_intent() {
    let feel = ListenerFeel::neutral();
    let msg = response_intent(&feel, 0).expect("intent");
    assert_eq!(msg.source, IntentSource::Own);
    assert_eq!(msg.verb_str(), RESPONSE_VERB);
    assert_eq!(msg.factor_mask, 0);
    assert_eq!(msg.consent, SENSE);
    assert_eq!(msg.strength, 0.0);
    let bytes = msg.encode().expect("encode");
    assert_eq!(bytes.len(), 72);
}

#[test]
fn one_hear_stays_inside_the_frame_budget() {
    let mut ear = Ear::new();
    let mut core = EmotionCore::at_rest();
    let mut floor = Sink;
    let mut buf = [0.0f32; FRAME_SAMPLES];
    fill_sine(&mut buf, 200.0, 0.2);
    for _ in 0..8 {
        let _ = hear_frame(&mut ear, &mut core, &mut floor, &buf, "I'm fine");
    }
    let mut sum = 0u128;
    let mut max_us = 0u128;
    let n = 40u128;
    for _ in 0..n {
        let t = Instant::now();
        let _ = hear_frame(&mut ear, &mut core, &mut floor, &buf, "I'm fine");
        let us = t.elapsed().as_micros();
        sum += us;
        if us > max_us {
            max_us = us;
        }
    }
    let mean = sum / n;
    eprintln!(
        "listener hear mean {mean} us max {max_us} us over {n} (debug or release, this host)"
    );
    assert!(
        max_us < 60_000,
        "max frame {max_us} us exceeds the 60 ms budget"
    );
}

#[test]
fn sources_do_not_name_aria_ort_or_a_research_dir() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/listen.rs");
    let text = std::fs::read_to_string(&path).expect("listen.rs");
    for needle in [
        "Aria",
        "ARIA",
        "Jarvis",
        "ort::",
        "onnxruntime",
        "GALION_HORBM_RESEARCH_DIR",
        "cfg(unix",
        "cfg(windows",
        "cfg(target",
        "std::fs",
        "std::net",
        ".wav",
        "include_bytes!",
    ] {
        assert!(!text.contains(needle), "listen.rs contains {needle}");
    }
    assert!(text.contains("Perpetual Open MIT"));
    assert!(text.contains("text_affect"));
}
