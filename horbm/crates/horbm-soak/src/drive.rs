// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Seeded conversation on the virtual clock.
//!
//! Turns, pauses, a listed backchannel, and barge-ins are a function of
//! [`SEED`]. The microphone is a generated sine. Silence is zeros.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use galion_emotion_core::{MomentState, AROUSAL, AXES, DOMINANCE, VEL_LIMIT};
use horbm::{CardHit, CardLookup};
use horbm_duplex::{
    CoreAffect, Duplex, EarSense, Emotion, Floor, FrameOut, LoopEvent, Parts, PlaybackEcho,
    ProsodyEar, SamanthaPersona, SpeakingBreath, FRAME_SAMPLES, SAMPLE_RATE,
};
use horbm_voice::Mouth;

use crate::buffer::OutputBuffer;
use crate::mouth::SoakMouth;
use crate::recog::Scripted;
use crate::stats::{percentile, vm_rss_kib};
use crate::{AllocSpan, Report, Window};

/// Fixed seed. The schedule and the sine phase come from this and nothing else.
pub const SEED: u64 = 0x5341_4D41_4E54_4831;

const WARMUP_TICKS: u64 = 250;
const WINDOW_TICKS: u64 = 3_750;
const END_SLACK: u64 = 800;

const PHRASES: [&[&str]; 3] = [
    &["where", "is", "the", "kettle"],
    &["the", "kettle"],
    &["kettle", "please"],
];
const STOP: &[&str] = &["stop", "now"];
const YEAH: &[&str] = &["yeah"];
const EMPTY: &[&str] = &[];

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

    fn len(&self) -> usize {
        1
    }
}

#[derive(Clone, Copy)]
struct Action {
    voiced: bool,
    reveal: bool,
    steady: bool,
    mark_onset: bool,
    words: &'static [&'static str],
}

enum Phase {
    Gap {
        left: u32,
    },
    User {
        words: &'static [&'static str],
        index: usize,
        holds: u32,
    },
    Release {
        left: u32,
    },
    Choose,
    Backchannel {
        left: u32,
    },
    Interrupt {
        sent: u32,
    },
    AfterBarge {
        left: u32,
    },
    Drain {
        sent: u32,
    },
}

struct Machine {
    phase: Phase,
    state: u64,
    turns: u32,
    start_probe: bool,
    end_probe: bool,
    onset: Option<u64>,
    barges: Vec<u64>,
    failures: Vec<String>,
}

impl Machine {
    fn new() -> Self {
        Self {
            phase: Phase::Gap { left: 20 },
            state: SEED,
            turns: 0,
            start_probe: false,
            end_probe: false,
            onset: None,
            barges: Vec::new(),
            failures: Vec::new(),
        }
    }

    fn rng(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    fn action(&mut self, tick: u64, total: u64, prev: Option<&Prev>) -> Action {
        loop {
            match self.phase {
                Phase::Gap { left } => {
                    if left == 0 {
                        self.turns = self.turns.saturating_add(1);
                        let words = PHRASES[(self.rng() as usize) % PHRASES.len()];
                        self.phase = Phase::User {
                            words,
                            index: 0,
                            holds: 0,
                        };
                        continue;
                    }
                    self.phase = Phase::Gap { left: left - 1 };
                    return silence(true);
                }
                Phase::User {
                    words,
                    index,
                    holds,
                } => {
                    if index < words.len() {
                        self.phase = Phase::User {
                            words,
                            index: index + 1,
                            holds,
                        };
                        return Action {
                            voiced: true,
                            reveal: true,
                            steady: false,
                            mark_onset: false,
                            words,
                        };
                    }
                    if holds < 2 {
                        self.phase = Phase::User {
                            words,
                            index,
                            holds: holds + 1,
                        };
                        return Action {
                            voiced: true,
                            reveal: false,
                            steady: true,
                            mark_onset: false,
                            words,
                        };
                    }
                    self.phase = Phase::Release { left: 40 };
                    return silence(false);
                }
                Phase::Release { left } => {
                    if prev.is_some_and(|frame| frame.me_audible) {
                        self.phase = Phase::Choose;
                        continue;
                    }
                    if left == 0 {
                        self.failures
                            .push(format!("turn {} did not reach the mouth", self.turns));
                        self.phase = Phase::Gap { left: 8 };
                        continue;
                    }
                    self.phase = Phase::Release { left: left - 1 };
                    return silence(false);
                }
                Phase::Choose => {
                    let interrupt = if !self.start_probe {
                        self.start_probe = true;
                        true
                    } else if !self.end_probe && tick.saturating_add(END_SLACK) >= total {
                        self.end_probe = true;
                        true
                    } else {
                        self.rng() % 5 == 0
                    };
                    if interrupt {
                        self.phase = Phase::Interrupt { sent: 0 };
                    } else if self.rng() % 3 == 0 {
                        self.phase = Phase::Backchannel { left: 3 };
                    } else {
                        self.phase = Phase::Drain { sent: 0 };
                    }
                    continue;
                }
                Phase::Backchannel { left } => {
                    if left == 0 {
                        self.phase = Phase::Drain { sent: 0 };
                        continue;
                    }
                    let reveal = left == 3;
                    self.phase = Phase::Backchannel { left: left - 1 };
                    return Action {
                        voiced: true,
                        reveal,
                        steady: false,
                        mark_onset: false,
                        words: YEAH,
                    };
                }
                Phase::Interrupt { sent } => {
                    if sent > 8 {
                        self.failures
                            .push(format!("turn {} did not barge", self.turns));
                        self.phase = Phase::Gap { left: 8 };
                        continue;
                    }
                    let mark_onset = sent == 0;
                    self.phase = Phase::Interrupt { sent: sent + 1 };
                    return Action {
                        voiced: true,
                        reveal: sent < 2,
                        steady: false,
                        mark_onset,
                        words: STOP,
                    };
                }
                Phase::AfterBarge { left } => {
                    if left == 0 {
                        let gap = 8 + (self.rng() % 24) as u32;
                        self.phase = Phase::Gap { left: gap };
                        continue;
                    }
                    self.phase = Phase::AfterBarge { left: left - 1 };
                    return silence(false);
                }
                Phase::Drain { sent } => {
                    let done = prev
                        .is_some_and(|frame| !frame.me_audible && frame.floor == Floor::Listening);
                    if done && sent > 0 {
                        let gap = 8 + (self.rng() % 24) as u32;
                        self.phase = Phase::Gap { left: gap };
                        continue;
                    }
                    if sent > 200 {
                        self.failures
                            .push(format!("turn {} did not finish speaking", self.turns));
                        self.phase = Phase::Gap { left: 8 };
                        continue;
                    }
                    self.phase = Phase::Drain { sent: sent + 1 };
                    return silence(true);
                }
            }
        }
    }
}

fn silence(steady: bool) -> Action {
    Action {
        voiced: false,
        reveal: false,
        steady,
        mark_onset: false,
        words: EMPTY,
    }
}

struct Prev {
    floor: Floor,
    me_audible: bool,
}

impl Prev {
    fn from_frame(frame: &FrameOut) -> Self {
        Self {
            floor: frame.floor,
            me_audible: frame.me_audible,
        }
    }
}

#[derive(Clone, Copy)]
enum Bucket {
    Idle,
    Play,
    Hold,
    Boundary,
}

fn bucket(action: &Action, frame: &FrameOut) -> Bucket {
    if !action.steady || action.reveal {
        return Bucket::Boundary;
    }
    if matches!(
        frame.event,
        LoopEvent::EagerEndOfTurn
            | LoopEvent::Committed
            | LoopEvent::FirstAudio
            | LoopEvent::BargeIn
            | LoopEvent::TurnResumed
            | LoopEvent::Backchannel
    ) {
        return Bucket::Boundary;
    }
    if action.voiced {
        return Bucket::Hold;
    }
    if frame.me_audible {
        // The frame that closes the utterance still clones the partial.
        // Echo residue can do that on a scripted silence. Steady playback
        // is the frames where that string is already gone.
        if !frame.partial.is_empty() {
            return Bucket::Boundary;
        }
        return Bucket::Play;
    }
    // The mouth queue can be empty while the floor is still hers. Those
    // frames still clone the committed clause, so they are not idle.
    if matches!(frame.floor, Floor::Speaking | Floor::Ducked) {
        return Bucket::Boundary;
    }
    if !frame.you_voiced {
        return Bucket::Idle;
    }
    Bucket::Boundary
}

struct Watch {
    min: [f32; AXES],
    max: [f32; AXES],
    non_finite: u64,
    out_of_range: u64,
}

impl Watch {
    fn new() -> Self {
        Self {
            min: [f32::INFINITY; AXES],
            max: [f32::NEG_INFINITY; AXES],
            non_finite: 0,
            out_of_range: 0,
        }
    }

    fn note(&mut self, moment: MomentState) {
        for axis in 0..AXES {
            let pos = moment.pos[axis];
            let vel = moment.vel[axis];
            if !pos.is_finite() || !vel.is_finite() {
                self.non_finite = self.non_finite.saturating_add(1);
                continue;
            }
            let (lo, hi) = axis_limits(axis);
            if pos < lo || pos > hi || vel.abs() > VEL_LIMIT {
                self.out_of_range = self.out_of_range.saturating_add(1);
            }
            self.min[axis] = self.min[axis].min(pos);
            self.max[axis] = self.max[axis].max(pos);
        }
    }
}

struct Probes {
    prosody: u64,
    emotion: u64,
    echo: u64,
    buffer: u64,
}

fn probes(allocs: &dyn Fn() -> u64) -> Probes {
    let tone = loud_frame();
    let mut sense = ProsodyEar::new();
    let before = allocs();
    let _ = sense.hear(&tone);
    let prosody = allocs().saturating_sub(before);

    let mut emotion = CoreAffect::samantha();
    let before = allocs();
    let _ = emotion.step(0);
    let emotion_n = allocs().saturating_sub(before);

    let mut echo = PlaybackEcho::new();
    let mut out = [0i16; FRAME_SAMPLES];
    let before = allocs();
    let _ = echo.cancel_into(&tone, &mut out);
    let echo_n = allocs().saturating_sub(before);

    let mut buffer = OutputBuffer::with_capacity_samples(FRAME_SAMPLES * 4);
    let before = allocs();
    buffer.enqueue(&tone);
    let mut scratch = [0i16; FRAME_SAMPLES];
    assert!(buffer.dequeue_frame(&mut scratch));
    let buffer_n = allocs().saturating_sub(before);

    Probes {
        prosody,
        emotion: emotion_n,
        echo: echo_n,
        buffer: buffer_n,
    }
}

fn axis_limits(axis: usize) -> (f32, f32) {
    match axis {
        AROUSAL | DOMINANCE => (0.0, 1.0),
        _ => (-1.0, 1.0),
    }
}

fn loud_frame() -> Vec<i16> {
    let mut pcm = vec![0i16; FRAME_SAMPLES];
    fill_sine(&mut pcm, true, &mut 0.0, 180.0);
    pcm
}

fn fill_sine(pcm: &mut [i16], voiced: bool, phase: &mut f32, hz: f32) {
    if !voiced {
        pcm.fill(0);
        return;
    }
    let step = hz / (SAMPLE_RATE as f32);
    for sample in pcm {
        let x = (*phase * std::f32::consts::TAU).sin();
        *sample = (x * 14_000.0) as i16;
        *phase += step;
        if *phase >= 1.0 {
            *phase -= 1.0;
        }
    }
}

pub fn ticks_for(minutes: u64) -> u64 {
    minutes.saturating_mul(60).saturating_mul(1_000) / 80
}

pub fn run(minutes: u64, allocs: &dyn Fn() -> u64) -> Report {
    let ticks = ticks_for(minutes);
    let probed = probes(allocs);
    let line = Rc::new(RefCell::new(EMPTY));
    let buffer = Rc::new(RefCell::new(OutputBuffer::with_capacity_samples(
        usize::try_from(SAMPLE_RATE)
            .unwrap_or(16_000)
            .saturating_mul(10),
    )));
    let mouth = SoakMouth::open(Rc::clone(&buffer));
    let mut duplex = Duplex::new(
        Parts {
            recognizer: Scripted::new(Rc::clone(&line)),
            emotion: CoreAffect::samantha(),
            breath: SpeakingBreath,
            persona: SamanthaPersona,
            mouth,
        },
        Kettle,
    )
    .expect("samantha");

    let mut machine = Machine::new();
    let mut watch = Watch::new();
    let mut prev: Option<Prev> = None;
    let mut pcm = vec![0i16; FRAME_SAMPLES];
    let mut phase = 0.0f32;
    let mut idle = AllocSpan::new();
    let mut play = AllocSpan::new();
    let mut hold = AllocSpan::new();
    let mut walls: Vec<u64> = Vec::with_capacity(WINDOW_TICKS as usize);
    let mut windows = Vec::new();
    let mut user_sent = 0u64;
    let mut user_heard = 0u64;
    let mut prosody_bad = 0u64;
    let rss_start = vm_rss_kib();
    let mut rss_warm: Option<Result<u64, String>> = None;
    let mut rss_peak = 0u64;

    for tick in 0..ticks {
        let action = machine.action(tick, ticks, prev.as_ref());
        *line.borrow_mut() = action.words;
        let hz = 180.0 + (machine.turns % 5) as f32 * 25.0;
        fill_sine(&mut pcm, action.voiced, &mut phase, hz);

        let before = allocs();
        let started = Instant::now();
        let frame = match duplex.tick(&pcm) {
            Ok(frame) => frame,
            Err(err) => {
                machine.failures.push(err.to_string());
                break;
            }
        };
        let spent = allocs().saturating_sub(before);
        let wall = u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX);
        walls.push(wall);

        if action.mark_onset {
            machine.onset = Some(frame.t_ms);
        }
        if matches!(machine.phase, Phase::Interrupt { .. }) && frame.event == LoopEvent::BargeIn {
            if let Some(onset) = machine.onset {
                machine.barges.push(frame.t_ms.saturating_sub(onset));
            }
            machine.onset = None;
            machine.phase = Phase::AfterBarge { left: 5 };
        }

        if action.voiced {
            user_sent = user_sent.saturating_add(1);
            if frame.you_voiced {
                user_heard = user_heard.saturating_add(1);
            }
        }
        let moment = duplex.emotion().moment();
        watch.note(moment);
        let cue = duplex.prosody();
        if !cue.enabled || !cue.rms_db.is_finite() || !cue.f0_hz.is_finite() {
            prosody_bad = prosody_bad.saturating_add(1);
        }
        if !frame.affect.valence.is_finite()
            || !frame.affect.arousal.is_finite()
            || !frame.affect.dominance.is_finite()
        {
            watch.non_finite = watch.non_finite.saturating_add(1);
        }

        if tick >= WARMUP_TICKS {
            match bucket(&action, &frame) {
                Bucket::Idle => idle.push(spent),
                Bucket::Play => play.push(spent),
                Bucket::Hold => hold.push(spent),
                Bucket::Boundary => {}
            }
        }
        if tick == WARMUP_TICKS {
            rss_warm = Some(vm_rss_kib());
        }
        if walls.len() == WINDOW_TICKS as usize {
            if let Ok(kib) = vm_rss_kib() {
                rss_peak = rss_peak.max(kib);
            }
            windows.push(close_window(windows.len() as u32, &mut walls));
        }
        prev = Some(Prev::from_frame(&frame));
    }

    if !walls.is_empty() {
        if let Ok(kib) = vm_rss_kib() {
            rss_peak = rss_peak.max(kib);
        }
        windows.push(close_window(windows.len() as u32, &mut walls));
    }
    let rss_end = vm_rss_kib();
    if !machine.end_probe {
        machine
            .failures
            .push("the closing barge probe did not run".to_string());
    }
    if !machine.start_probe {
        machine
            .failures
            .push("the opening barge probe did not run".to_string());
    }

    let mouth_name = duplex.mouth().id();
    let desync = duplex.mouth().desync();
    let mouth_label = if mouth_name == "flite" {
        "flite"
    } else {
        "native"
    };
    let buf = buffer.borrow();

    Report {
        minutes,
        ticks,
        seed: SEED,
        mouth: mouth_label,
        prosody: duplex.prosody_enabled(),
        aec: duplex.aec_enabled(),
        barges: machine.barges,
        idle,
        play,
        hold,
        echo_probe: probed.echo,
        prosody_probe: probed.prosody,
        emotion_probe: probed.emotion,
        buffer_probe: probed.buffer,
        windows,
        rss_start,
        rss_warm,
        rss_end,
        rss_peak,
        buffer_max: buf.max_depth(),
        buffer_end: buf.depth(),
        buffer_overflows: buf.overflows(),
        desync,
        emotion_non_finite: watch.non_finite,
        emotion_out_of_range: watch.out_of_range,
        emotion_min: watch.min,
        emotion_max: watch.max,
        user_sent,
        user_heard,
        prosody_bad,
        turns: machine.turns,
        failures: machine.failures,
    }
}

fn close_window(index: u32, walls: &mut Vec<u64>) -> Window {
    walls.sort_unstable();
    let window = crate::Window {
        index,
        ticks: walls.len() as u64,
        p50_ns: percentile(walls, 50.0),
        p99_ns: percentile(walls, 99.0),
        max_ns: walls.last().copied().unwrap_or(0),
    };
    walls.clear();
    window
}
