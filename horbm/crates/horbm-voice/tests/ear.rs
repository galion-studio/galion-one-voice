// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Native ear on synthetic tones. No recording, no model, no wall clock.

use horbm::{CardHit, CardLookup};
use horbm_voice::{
    Ear, EarEvent, EnergyEar, FakeMouth, HorbmTurn, TickEvent, VoiceLoop, BARGE_IN_MAX_MS,
    FRAME_MS, FRAME_SAMPLES,
};

struct Kettle;

impl CardLookup for Kettle {
    fn lookup(&self, query: &str) -> Option<CardHit> {
        if query.to_ascii_lowercase().contains("kettle") {
            Some(CardHit {
                id: "kettle".into(),
                gist: "The kettle is on the hob.".into(),
            })
        } else {
            None
        }
    }
}

fn silence() -> Vec<i16> {
    vec![0; FRAME_SAMPLES]
}

fn sine(amp: i16, step: f64) -> Vec<i16> {
    (0..FRAME_SAMPLES)
        .map(|index| ((index as f64 * step).sin() * f64::from(amp)) as i16)
        .collect()
}

fn brain() -> HorbmTurn {
    HorbmTurn::with_cards(Kettle)
}

/// First energy turn-end carries a fixture line so the mouth can start.
/// Later events stay the ear's.
struct OpenEar {
    inner: EnergyEar,
    line: Option<String>,
}

impl Ear for OpenEar {
    fn id(&self) -> &str {
        "energy"
    }

    fn note_playback(&mut self, pcm: &[i16]) {
        self.inner.note_playback(pcm);
    }

    fn poll(&mut self, pcm: &[i16]) -> EarEvent {
        let event = self.inner.poll(pcm);
        if let EarEvent::End(text) = &event {
            if text.is_empty() {
                if let Some(line) = self.line.take() {
                    return EarEvent::end(line);
                }
            }
        }
        event
    }
}

fn open_voice() -> VoiceLoop<OpenEar, FakeMouth, HorbmTurn> {
    let mut voice = VoiceLoop::new(
        OpenEar {
            inner: EnergyEar::new(),
            line: Some("where is the kettle".into()),
        },
        FakeMouth::new(),
        brain(),
    );
    let loud = sine(8_000, 0.2);
    assert_eq!(voice.tick(&silence()).event, TickEvent::None);
    assert_eq!(voice.tick(&loud).event, TickEvent::Onset);
    assert_eq!(voice.tick(&loud).event, TickEvent::None);
    let end = voice.tick(&silence());
    assert_eq!(end.event, TickEvent::EndOfSpeech);
    assert_eq!(end.reply.as_deref(), Some("The kettle is on the hob."));
    voice
}

fn first_playback(voice: &mut VoiceLoop<OpenEar, FakeMouth, HorbmTurn>) -> Vec<i16> {
    for _ in 0..8 {
        let frame = voice.tick(&silence());
        if let Some(audio) = frame.audio {
            return audio.samples;
        }
    }
    panic!("mouth did not play");
}

#[test]
fn onset_and_offset_land_on_the_boundary_frame() {
    let mut ear = EnergyEar::new();
    let loud = sine(8_000, 0.2);
    let quiet = silence();
    let events = [
        ear.poll(&quiet),
        ear.poll(&loud),
        ear.poll(&loud),
        ear.poll(&quiet),
    ];
    assert_eq!(events[0], EarEvent::Quiet);
    assert_eq!(events[1], EarEvent::Onset);
    assert_eq!(events[2], EarEvent::Quiet);
    assert_eq!(events[3], EarEvent::end(""));
    let onset_at = events
        .iter()
        .position(|event| *event == EarEvent::Onset)
        .expect("onset");
    let offset_at = events
        .iter()
        .position(|event| matches!(event, EarEvent::End(_)))
        .expect("offset");
    assert!(onset_at.abs_diff(1) <= 1, "onset frame {onset_at}");
    assert!(offset_at.abs_diff(3) <= 1, "offset frame {offset_at}");
    println!(
        "ear gate: onset_frames={} offset_frames={} frame_ms={FRAME_MS} result=pass",
        onset_at.abs_diff(1),
        offset_at.abs_diff(3)
    );
}

#[test]
fn a_boundary_inside_the_frame_is_still_within_one_frame() {
    let mut ear = EnergyEar::new();
    let loud = sine(8_000, 0.2);
    let mut late = silence();
    let split = FRAME_SAMPLES / 2;
    late[split..].copy_from_slice(&loud[split..]);
    assert_eq!(ear.poll(&late), EarEvent::Quiet);
    assert_eq!(ear.poll(&loud), EarEvent::Onset);
    let mut early_off = loud.clone();
    early_off[split..].fill(0);
    assert_eq!(ear.poll(&early_off), EarEvent::Quiet);
    assert_eq!(ear.poll(&silence()), EarEvent::end(""));
    println!("ear gate: late_onset_frames=1 early_offset_frames=1 frame_ms={FRAME_MS} result=pass");
}

#[test]
fn barge_in_while_she_speaks_stays_inside_200_ms() {
    let mut voice = open_voice();
    let _played = first_playback(&mut voice);
    let user = sine(8_000, 0.07);
    let epoch = voice.epoch();
    let mut barge_at = None;
    for index in 0..4 {
        let frame = voice.tick(&user);
        if frame.event == TickEvent::BargeIn {
            barge_at = Some(index);
            assert!(frame.cancel);
            assert!(frame.me_silent);
            assert!(frame.audio.is_none());
            assert_eq!(frame.epoch, epoch + 1);
            break;
        }
        assert!(!frame.cancel, "frame {index}");
    }
    assert_eq!(barge_at, Some(2));
    let barge_ms = voice.barge_in_ms().expect("barge");
    assert_eq!(barge_ms, 160);
    assert!(barge_ms <= u64::from(BARGE_IN_MAX_MS));
    assert_eq!(voice.mouth().cancels(), 1);
    println!(
        "ear gate: barge_in_ms={barge_ms} limit_ms={BARGE_IN_MAX_MS} clock=virtual frame_ms={FRAME_MS} result=pass"
    );
}

#[test]
fn a_short_yeah_never_cancels() {
    let mut voice = open_voice();
    let _played = first_playback(&mut voice);
    let yeah = sine(8_000, 0.07);
    let epoch = voice.epoch();
    let first = voice.tick(&yeah);
    let second = voice.tick(&yeah);
    assert_ne!(first.event, TickEvent::BargeIn);
    assert_ne!(second.event, TickEvent::BargeIn);
    assert!(!first.cancel && !second.cancel);
    let done = voice.tick(&silence());
    assert_eq!(done.event, TickEvent::Backchannel);
    assert!(!done.cancel);
    assert_eq!(done.epoch, epoch);
    assert_eq!(voice.mouth().cancels(), 0);
    assert!(voice.barge_in_ms().is_none());
    println!(
        "ear gate: backchannel=yeah frames=2 frame_ms={FRAME_MS} span_ms=160 cancel=false result=pass"
    );
}

#[test]
fn self_echo_is_not_barge_in() {
    let mut voice = open_voice();
    let mut played = first_playback(&mut voice);
    for index in 0..6 {
        let frame = voice.tick(&played);
        assert_ne!(frame.event, TickEvent::BargeIn, "echo frame {index}");
        assert!(!frame.cancel, "echo frame {index}");
        if let Some(audio) = frame.audio {
            played = audio.samples;
        }
    }
    assert_eq!(voice.mouth().cancels(), 0);
    assert!(voice.barge_in_ms().is_none());
    let mut ear = EnergyEar::new();
    ear.note_playback(&played);
    let quiet_echo: Vec<i16> = played.iter().map(|sample| sample / 2).collect();
    assert_eq!(ear.poll(&played), EarEvent::Quiet);
    assert_eq!(ear.poll(&quiet_echo), EarEvent::Quiet);
    println!("ear gate: self_echo frames=6 cancel=false result=pass");
}

#[test]
fn the_same_signal_produces_the_same_events() {
    let loud = sine(8_000, 0.2);
    let quiet = silence();
    let signal = [
        quiet.clone(),
        loud.clone(),
        loud.clone(),
        quiet.clone(),
        loud,
    ];
    let once = listen(&signal);
    let twice = listen(&signal);
    assert_eq!(once, twice);
    assert_eq!(
        once,
        vec![
            EarEvent::Quiet,
            EarEvent::Onset,
            EarEvent::Quiet,
            EarEvent::end(""),
            EarEvent::Onset,
        ]
    );
}

fn listen(frames: &[Vec<i16>]) -> Vec<EarEvent> {
    let mut ear = EnergyEar::new();
    frames.iter().map(|frame| ear.poll(frame)).collect()
}
