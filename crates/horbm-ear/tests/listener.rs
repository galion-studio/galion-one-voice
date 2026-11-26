// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Listener readings from generated sines. No audio file is read or written.

use std::sync::{Mutex, MutexGuard};
use std::time::Instant;

use horbm_ear::consent_bits::{REMEMBER, SENSE};
use horbm_ear::link::{FloorCue, FloorTick};
use horbm_ear::listen::{
    appraise, configured_profile, hear_frame, listener_feel, profile_from_env, response_intent,
    ListenerFeel, ObsSource, Persona, PersonaProfile, Reading, UserObservation, DEV_PERSONA_ENV,
    DIMENSIONS, RESPONSE_VERB, UNIVERSAL,
};
use horbm_ear::port::{
    EmotionCore, IntentMsg, IntentSource, VoiceBaseline, APPROACH, AROUSAL, DOMINANCE, VALENCE,
};
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
    let profile = wired_profile();
    let mut ear = Ear::new();
    let mut core = EmotionCore::at_rest();
    let mut floor = Sink;
    let mut buf = [0.0f32; FRAME_SAMPLES];
    let mut last = None;
    for i in 0..N {
        render(cue, i, &mut buf);
        let out =
            hear_frame(&mut ear, &mut core, &mut floor, &buf, cue.words(), profile).expect("hear");
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

/// Id from the committed boot file. Not a name written into this test.
fn boot_persona_id() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../horbm/crates/horbm-persona/development.toml");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("development config {}: {err}", path.display()));
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() != "development_persona" {
            continue;
        }
        let value = value.trim().trim_matches('"');
        assert!(!value.is_empty(), "development persona id is blank");
        return value.to_string();
    }
    panic!("development config has no persona id");
}

/// The profile this layer used to hard-wire, looked up from the boot id.
fn wired_profile() -> &'static PersonaProfile {
    let id = boot_persona_id();
    configured_profile(Some(&id)).unwrap_or_else(|| {
        panic!("configured id {id} has no emotion profile and must not fall back")
    })
}

fn feel_at(cue: Cue, index: usize) -> ListenerFeel {
    let mut ear = Ear::new();
    let mut buf = [0.0f32; FRAME_SAMPLES];
    let mut feel = ListenerFeel::neutral();
    for i in 0..=index {
        render(cue, i, &mut buf);
        let frame = heard_frame(ear.push(&buf));
        feel = listener_feel(&frame, cue.words());
    }
    feel
}

fn cue_named(name: &str) -> Cue {
    [
        Cue::Sad,
        Cue::Excited,
        Cue::Stressed,
        Cue::Flat,
        Cue::Neutral,
    ]
    .into_iter()
    .find(|cue| cue.name() == name)
    .unwrap_or_else(|| panic!("missing fixture {name}"))
}

/// Pulls [`response_intent`] must match, from [`appraise`] of the same reading.
fn predicted_pulls(profile: &PersonaProfile, feel: &ListenerFeel) -> [f32; 4] {
    let obs = UserObservation {
        valence: feel.valence.value,
        arousal: feel.arousal.value,
        dominance: 0.5,
        confidence: feel.valence.confidence,
        source: ObsSource::Fused,
    };
    let mood = appraise(profile, &obs);
    let rest = profile.dynamics.rest;
    let clamp = 2.0;
    let valence = (mood.valence - rest.valence - 0.10 * feel.fatigue.value).clamp(-clamp, clamp);
    let arousal = (mood.arousal - rest.arousal + 0.20 * feel.tension.value
        - 0.12 * feel.fatigue.value)
        .clamp(-clamp, clamp);
    let dominance = (mood.dominance - rest.dominance).clamp(-clamp, clamp);
    let approach = if feel.valence.value < -0.15 {
        (0.30 * (-feel.valence.value)).clamp(0.0, clamp)
    } else if feel.valence.value > 0.20 {
        (0.15 * feel.valence.value).clamp(0.0, clamp)
    } else {
        0.0
    };
    let mut pulls = [0.0; 4];
    pulls[VALENCE] = valence;
    pulls[AROUSAL] = arousal;
    pulls[DOMINANCE] = dominance;
    pulls[APPROACH] = approach;
    pulls
}

fn decode_hex(text: &str) -> [u8; 72] {
    assert_eq!(text.len(), 144, "intent hex is 72 bytes");
    let mut out = [0u8; 72];
    for (index, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16)
            .unwrap_or_else(|_| panic!("bad hex at {index}"));
    }
    out
}

static ENV_LOCK: Mutex<()> = Mutex::new(());

struct EnvVar {
    prev: Option<std::ffi::OsString>,
}

impl EnvVar {
    fn set(value: Option<&str>) -> Self {
        let prev = std::env::var_os(DEV_PERSONA_ENV);
        match value {
            Some(value) => std::env::set_var(DEV_PERSONA_ENV, value),
            None => std::env::remove_var(DEV_PERSONA_ENV),
        }
        Self { prev }
    }
}

impl Drop for EnvVar {
    fn drop(&mut self) {
        match &self.prev {
            Some(value) => std::env::set_var(DEV_PERSONA_ENV, value),
            None => std::env::remove_var(DEV_PERSONA_ENV),
        }
    }
}

struct HeldEnv {
    _var: EnvVar,
    _lock: MutexGuard<'static, ()>,
}

fn hold_env(value: Option<&str>) -> HeldEnv {
    let lock = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
    HeldEnv {
        _var: EnvVar::set(value),
        _lock: lock,
    }
}

#[test]
fn dimensions_are_axes_not_a_category_list() {
    assert_eq!(RESPONSE_VERB, "with");
    assert_eq!(
        DIMENSIONS,
        ["valence", "arousal", "tension", "fatigue", "confidence"]
    );
    for name in ["sad", "happy", "angry", "fear", "parked-fixture", "northwind"] {
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
        let profile = wired_profile();
        let mut ear = Ear::new();
        let mut core = EmotionCore::at_rest();
        let mut floor = Sink;
        let mut buf = [0.0f32; FRAME_SAMPLES];
        let mut last = None;
        for i in 0..N {
            render(Cue::Excited, i, &mut buf);
            let out = hear_frame(&mut ear, &mut core, &mut floor, &buf, "I'm fine", profile)
                .expect("hear");
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

    let profile = wired_profile();
    let mut sad_feel = ListenerFeel::neutral();
    let mut hot_feel = ListenerFeel::neutral();
    for i in 0..8 {
        let sad =
            hear_frame(&mut sad_ear, &mut sad_core, &mut floor, &low, "", profile).expect("sad");
        let hot =
            hear_frame(&mut hot_ear, &mut hot_core, &mut floor, &high, "", profile).expect("hot");
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
    let profile = wired_profile();
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
        let out = hear_frame(&mut ear, &mut core, &mut floor, &buf, "", profile).expect("hear");
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
    let profile = wired_profile();
    let mut core = EmotionCore::at_rest();
    let bits = core.baseline().identity_bits();
    let start = core.moment();
    let mut floor = CueLog { cues: Vec::new() };
    for _ in 0..4 {
        let out = hear_frame(ear, &mut core, &mut floor, pcm, "I'm fine", profile).expect("off");
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
    let profile = wired_profile();
    let quiet = [0.0f32; FRAME_SAMPLES];
    for i in 0..6 {
        let out = hear_frame(&mut ear, &mut core, &mut floor, &quiet, "I'm fine", profile)
            .expect("quiet");
        assert_eq!(out.listener, ListenerFeel::neutral());
        let moment = out.moment.expect("quiet still steps");
        let control = hold.step(&IntentMsg::hold(i * FRAME_MS)).expect("hold");
        assert!((moment.pos[VALENCE] - control.pos[VALENCE]).abs() < 1.0e-4);
        assert!((moment.pos[AROUSAL] - control.pos[AROUSAL]).abs() < 1.0e-4);
    }

    let before = core.frames();
    let rejected = hear_frame(
        &mut ear, &mut core, &mut floor, &[0.0; 4], "I'm fine", profile,
    )
    .expect("bad");
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
    let out = hear_frame(
        &mut ear,
        &mut core,
        &mut floor,
        &buf,
        "I'm fine",
        wired_profile(),
    )
    .expect("hear");
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
    let msg = response_intent(&feel, 0, wired_profile()).expect("intent");
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
    let profile = wired_profile();
    fill_sine(&mut buf, 200.0, 0.2);
    for _ in 0..8 {
        let _ = hear_frame(&mut ear, &mut core, &mut floor, &buf, "I'm fine", profile);
    }
    let mut sum = 0u128;
    let mut max_us = 0u128;
    let n = 40u128;
    for _ in 0..n {
        let t = Instant::now();
        let _ = hear_frame(&mut ear, &mut core, &mut floor, &buf, "I'm fine", profile);
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
fn configured_profile_is_the_named_one() {
    assert!(!Persona::ALL.is_empty());
    for persona in Persona::ALL {
        let id = persona.as_str();
        let profile =
            configured_profile(Some(id)).unwrap_or_else(|| panic!("{id} has no emotion profile"));
        assert!(std::ptr::eq(profile, persona.profile()));
        assert_eq!(profile.persona.as_str(), id);
        let upper = id.to_ascii_uppercase();
        let folded = configured_profile(Some(&upper))
            .unwrap_or_else(|| panic!("{upper} did not resolve through parse"));
        assert!(std::ptr::eq(folded, profile));
    }
}

#[test]
fn unknown_persona_has_no_profile_and_no_fallback() {
    for id in [Some("northwind"), Some(""), None] {
        assert!(
            configured_profile(id).is_none(),
            "{id:?} resolved to a profile"
        );
    }
}

#[test]
fn response_intent_follows_the_profile() {
    let mut feel = ListenerFeel::neutral();
    feel.valence = Reading {
        value: -0.55,
        confidence: 0.8,
    };
    feel.arousal = Reading {
        value: 0.82,
        confidence: 0.7,
    };
    feel.tension = Reading {
        value: 0.45,
        confidence: 0.7,
    };
    feel.fatigue = Reading {
        value: 0.35,
        confidence: 0.6,
    };
    let profiles: Vec<&'static PersonaProfile> = Persona::ALL
        .into_iter()
        .map(|persona| configured_profile(Some(persona.as_str())).expect("built-in profile"))
        .collect();
    assert!(profiles.len() >= 2, "two built-in profiles are required");
    let mut encoded = Vec::new();
    for profile in &profiles {
        let msg = response_intent(&feel, 80, profile).expect("intent");
        let predicted = predicted_pulls(profile, &feel);
        for (axis, expected) in predicted.iter().enumerate() {
            assert_eq!(msg.pull[axis].to_bits(), expected.to_bits(), "axis {axis}");
        }
        encoded.push(msg.encode().expect("encode"));
    }
    assert_ne!(encoded[0], encoded[1]);
}

#[test]
fn same_profile_same_bits_as_before() {
    let profile = wired_profile();
    let rest = response_intent(&ListenerFeel::neutral(), 0, profile)
        .expect("intent")
        .encode()
        .expect("encode");
    assert_eq!(rest, decode_hex(REST_INTENT));

    for (name, index, expected) in FIXTURE_INTENTS {
        let feel = feel_at(cue_named(name), *index);
        let t_ms = *index as u32 * FRAME_MS;
        let bytes = response_intent(&feel, t_ms, profile)
            .expect("intent")
            .encode()
            .expect("encode");
        assert_eq!(bytes, decode_hex(expected), "{name} frame {index}");
    }
}

#[test]
fn profile_from_env_reads_the_configured_id_once_and_does_not_fall_back() {
    let known = Persona::ALL[0];
    {
        let _held = hold_env(Some(known.as_str()));
        let got = profile_from_env().expect("known id");
        assert!(std::ptr::eq(got, known.profile()));
        let again = profile_from_env().expect("second read");
        assert!(std::ptr::eq(again, got));
    }
    {
        let _held = hold_env(Some("northwind"));
        assert!(profile_from_env().is_none());
    }
    {
        let _held = hold_env(Some(""));
        assert!(profile_from_env().is_none());
    }
    {
        let _held = hold_env(None);
        assert!(profile_from_env().is_none());
    }
}

/// Pre-change encoding of a neutral reading at t = 0.
const REST_INTENT: &str = "47454d4901000001000000007769746800000000000000000000000000000000000000003c0ad73d00000000000000000f0000003333333fcdcc4c3d0000803fcdcc0c3f00000000";

/// Pre-change encodings of the sine fixtures at frame 0 and frame 24.
const FIXTURE_INTENTS: &[(&str, usize, &str)] = &[
    (
        "sad-low-slow",
        0,
        "47454d49010000010000000077697468000000000000000000000000feff7f3f675c90be0e7246bea0a37bbc85ee083e0f0000003333333fcdcc4c3d0000803fcdcc0c3f00000000",
    ),
    (
        "sad-low-slow",
        24,
        "47454d49010000018007000077697468000000000000000000000000feff7f3fb39b8bbef8093bbea0a37bbc85ee083e0f0000003333333fcdcc4c3d0000803fcdcc0c3f00000000",
    ),
    (
        "excited-high-fast",
        0,
        "47454d4901000001000000007769746800000000000000000000000050df7f3f60a8363e324a993e0000000098af893d0f0000003333333fcdcc4c3d0000803fcdcc0c3f00000000",
    ),
    (
        "excited-high-fast",
        24,
        "47454d4901000001800700007769746800000000000000000000000050df7f3f1ac77b3e7800ad3e00000000d6b9a03d0f0000003333333fcdcc4c3d0000803fcdcc0c3f00000000",
    ),
    (
        "stressed-tense",
        0,
        "47454d49010000010000000077697468000000000000000000000000069c773f000000005449943e00000000000000000f0000003333333fcdcc4c3d0000803fcdcc0c3f00000000",
    ),
    (
        "stressed-tense",
        24,
        "47454d49010000018007000077697468000000000000000000000000069c773f000000005449943e00000000000000000f0000003333333fcdcc4c3d0000803fcdcc0c3f00000000",
    ),
    (
        "flat-im-fine",
        0,
        "47454d490100000100000000776974680000000000000000000000006cfc7f3f90e890bed99f40be40137bbc36c3083e0f0000003333333fcdcc4c3d0000803fcdcc0c3f00000000",
    ),
    (
        "flat-im-fine",
        24,
        "47454d490100000180070000776974680000000000000000000000006cfc7f3f90e890bed99f40be40137bbc36c3083e0f0000003333333fcdcc4c3d0000803fcdcc0c3f00000000",
    ),
    (
        "neutral",
        0,
        "47454d4901000001000000007769746800000000000000000000000068ff7f3f0044e6b9b684dc3d00000000000000000f0000003333333fcdcc4c3d0000803fcdcc0c3f00000000",
    ),
    (
        "neutral",
        24,
        "47454d4901000001800700007769746800000000000000000000000068ff7f3f0044e6b9b684dc3d00000000000000000f0000003333333fcdcc4c3d0000803fcdcc0c3f00000000",
    ),
];

fn persona_case_needles() -> Vec<String> {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../horbm/crates/horbm-persona/personas");
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).unwrap_or_else(|err| panic!("{}: {err}", dir.display())) {
        let path = entry.expect("persona entry").path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        out.push(stem.to_string());
        out.push(stem.to_uppercase());
        let mut chars = stem.chars();
        if let Some(first) = chars.next() {
            out.push(format!("{}{}", first.to_uppercase(), chars.as_str()));
        }
    }
    assert!(!out.is_empty());
    out
}

#[test]
fn sources_do_not_name_a_persona_file_ort_or_a_research_dir() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/listen.rs");
    let text = std::fs::read_to_string(&path).expect("listen.rs");
    for needle in persona_case_needles() {
        assert!(!text.contains(&needle), "listen.rs contains {needle}");
    }
    for needle in [
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
