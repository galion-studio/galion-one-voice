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

/// Dropped from the ranked tick series. Allocation buckets and the RSS
/// warmup read already use this cutoff. Ticks `0..WARMUP_TICKS` are not
/// ranked. Tick `WARMUP_TICKS` is the first ranked tick.
const WARMUP_TICKS: u64 = 250;
const WINDOW_TICKS: u64 = 3_750;
const END_SLACK: u64 = 800;

/// Clock around [`run`]'s `Duplex::tick`.
///
/// [`TickClock::Wall`] is [`std::time::Instant`]. The gate sample and the
/// wall sample are that one reading.
///
/// [`TickClock::ThreadCpu`] reads a monotonic nanosecond clock the caller
/// supplies. On Unix that clock is `CLOCK_THREAD_CPUTIME_ID`. Wall time is
/// still [`std::time::Instant`] around the same call, and it is not a gate.
/// This crate stays free of `unsafe`. The integration test owns the syscall.
#[derive(Clone, Copy)]
pub enum TickClock {
    Wall,
    ThreadCpu(fn() -> u64),
}

impl TickClock {
    /// `wall` or `thread-cpu`. Printed on the soak line.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            TickClock::Wall => "wall",
            TickClock::ThreadCpu(_) => "thread-cpu",
        }
    }
}

fn elapsed_ns(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX)
}

/// Ranked tick samples. Warmup ticks are counted, then left out of every
/// window. A window still closes on the 3,750-tick block boundary, so the
/// first window is short by the warmup and the later windows stay 3,750.
struct Series {
    gate: Vec<u64>,
    wall: Vec<u64>,
    windows: Vec<Window>,
    warmup_ticks: u64,
    warmup_gate_max: u64,
    warmup_wall_max: u64,
}

impl Series {
    fn new() -> Self {
        Self {
            gate: Vec::with_capacity(WINDOW_TICKS as usize),
            wall: Vec::with_capacity(WINDOW_TICKS as usize),
            windows: Vec::new(),
            warmup_ticks: 0,
            warmup_gate_max: 0,
            warmup_wall_max: 0,
        }
    }

    /// Records one tick. Returns whether this tick closed a block.
    fn on_tick(&mut self, tick: u64, gate_ns: u64, wall_ns: u64) -> bool {
        if tick < WARMUP_TICKS {
            self.warmup_ticks = self.warmup_ticks.saturating_add(1);
            self.warmup_gate_max = self.warmup_gate_max.max(gate_ns);
            self.warmup_wall_max = self.warmup_wall_max.max(wall_ns);
        } else {
            self.gate.push(gate_ns);
            self.wall.push(wall_ns);
        }
        if tick % WINDOW_TICKS == WINDOW_TICKS - 1 {
            self.close();
            true
        } else {
            false
        }
    }

    /// Closes a partial window when the call stopped off a block boundary.
    fn finish(&mut self) -> bool {
        if self.gate.is_empty() {
            return false;
        }
        self.close();
        true
    }

    fn close(&mut self) {
        if self.gate.is_empty() {
            return;
        }
        let index = self.windows.len() as u32;
        self.windows
            .push(close_window(index, &mut self.gate, &mut self.wall));
    }
}

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

pub fn run(minutes: u64, allocs: &dyn Fn() -> u64, clock: TickClock) -> Report {
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
    let mut series = Series::new();
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
        let wall_at = Instant::now();
        let cpu_at = match clock {
            TickClock::ThreadCpu(read) => read(),
            TickClock::Wall => 0,
        };
        let frame = match duplex.tick(&pcm) {
            Ok(frame) => frame,
            Err(err) => {
                machine.failures.push(err.to_string());
                break;
            }
        };
        let gate_ns = match clock {
            TickClock::ThreadCpu(read) => read().saturating_sub(cpu_at),
            TickClock::Wall => elapsed_ns(wall_at),
        };
        let wall_ns = match clock {
            TickClock::Wall => gate_ns,
            TickClock::ThreadCpu(_) => elapsed_ns(wall_at),
        };
        let spent = allocs().saturating_sub(before);

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
        let moment = duplex.emotion().view().moment;
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
        if series.on_tick(tick, gate_ns, wall_ns) {
            if let Ok(kib) = vm_rss_kib() {
                rss_peak = rss_peak.max(kib);
            }
        }
        prev = Some(Prev::from_frame(&frame));
    }

    if series.finish() {
        if let Ok(kib) = vm_rss_kib() {
            rss_peak = rss_peak.max(kib);
        }
    }
    let clock_name = clock.name();
    let Series {
        windows,
        warmup_ticks,
        warmup_gate_max,
        warmup_wall_max,
        ..
    } = series;
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
        clock: clock_name,
        warmup_ticks,
        warmup_gate_max_ns: warmup_gate_max,
        warmup_wall_max_ns: warmup_wall_max,
    }
}

fn close_window(index: u32, gate: &mut Vec<u64>, wall: &mut Vec<u64>) -> Window {
    debug_assert_eq!(gate.len(), wall.len());
    gate.sort_unstable();
    wall.sort_unstable();
    let window = crate::Window {
        index,
        ticks: gate.len() as u64,
        p50_ns: percentile(gate, 50.0),
        p99_ns: percentile(gate, 99.0),
        max_ns: gate.last().copied().unwrap_or(0),
        wall_p50_ns: percentile(wall, 50.0),
        wall_p99_ns: percentile(wall, 99.0),
        wall_max_ns: wall.last().copied().unwrap_or(0),
    };
    gate.clear();
    wall.clear();
    window
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The laptop wall sample that missed the 90 ms max once and passed on rerun.
    const REPORTED_WALL_SPIKE_NS: u64 = 94_500_000;

    #[test]
    fn warmup_is_excluded_and_the_block_stays_3750() {
        let mut series = Series::new();
        for tick in 0..ticks_for(5) {
            let ns = if tick == 10 {
                REPORTED_WALL_SPIKE_NS
            } else {
                1_000_000
            };
            series.on_tick(tick, ns, ns);
        }
        assert!(!series.finish());
        assert_eq!(series.warmup_ticks, WARMUP_TICKS);
        assert_eq!(series.warmup_gate_max, REPORTED_WALL_SPIKE_NS);
        assert_eq!(series.windows.len(), 1);
        assert_eq!(series.windows[0].ticks, WINDOW_TICKS - WARMUP_TICKS);
        assert_eq!(series.windows[0].p50_ns, 1_000_000);
        assert_eq!(series.windows[0].p99_ns, 1_000_000);
        assert_eq!(series.windows[0].max_ns, 1_000_000);
    }

    #[test]
    fn one_ranked_spike_sets_max_and_not_p99() {
        let mut series = Series::new();
        for tick in 0..ticks_for(5) {
            let gate = if tick == WARMUP_TICKS {
                REPORTED_WALL_SPIKE_NS
            } else {
                1_000_000
            };
            let wall = gate.saturating_add(5_000_000);
            series.on_tick(tick, gate, wall);
        }
        assert!(!series.finish());
        let window = &series.windows[0];
        assert_eq!(window.ticks, 3_500);
        assert_eq!(window.p99_ns, 1_000_000);
        assert_eq!(window.max_ns, REPORTED_WALL_SPIKE_NS);
        assert!(window.wall_max_ns > window.max_ns);
        assert!(window.p99_ns < 50_000_000);
    }

    #[test]
    fn an_hour_is_twelve_windows_and_only_the_first_is_short() {
        let mut series = Series::new();
        for tick in 0..ticks_for(60) {
            let closed = series.on_tick(tick, 1_000, 2_000);
            if tick == WINDOW_TICKS - 1 || tick == ticks_for(60) - 1 {
                assert!(closed);
            }
        }
        assert!(!series.finish());
        assert_eq!(series.windows.len(), 12);
        assert_eq!(series.windows[0].ticks, 3_500);
        assert!(series.windows[1..]
            .iter()
            .all(|window| window.ticks == 3_750));
        let ranked: u64 = series.windows.iter().map(|window| window.ticks).sum();
        assert_eq!(ranked, ticks_for(60) - WARMUP_TICKS);
    }
}
