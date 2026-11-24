// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Synthetic echo only. A tone, noise, and a mix of the two, through a
//! delay, a gain, and two short reverb taps. No recording and no voice.

use std::cell::Cell;
use std::rc::Rc;
#[cfg(feature = "aec")]
use std::time::Instant;

use horbm::{CardHit, CardLookup};
use horbm_duplex::{
    CoreAffect, Duplex, Hypothesis, HypothesisKind, LoopError, LoopEvent, Parts, PlaybackEcho,
    RecogError, Recognizer, SamanthaPersona, SpeakingBreath, FRAME_MS, FRAME_SAMPLES, SAMPLE_RATE,
};
#[cfg(feature = "aec")]
use horbm_duplex::{EarSense, ProsodyEar, AEC_CRATE, AEC_LICENSE, AEC_VERSION};
use horbm_voice::{AudioFrame, Mouth, MouthPull, SpokenLine};

const DELAY: usize = 160;
const GAIN: f32 = 0.08;
const TAPS: [(usize, f32); 2] = [(48, 0.30), (96, 0.12)];
/// Voiced echo frames at the start of adaptation. "yeah" keeps them from
/// arming a barge while the filter settles. Noise is quiet by 240 ms.
const GRACE_FRAMES: usize = 8;

#[derive(Clone, Copy)]
enum Kind {
    Tone,
    Noise,
    Mix,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Self::Tone => "tone",
            Self::Noise => "noise",
            Self::Mix => "mix",
        }
    }
}

struct NoCards;

impl CardLookup for NoCards {
    fn lookup(&self, _query: &str) -> Option<CardHit> {
        None
    }

    fn len(&self) -> usize {
        0
    }
}

fn unit_noise(index: usize) -> f32 {
    let mut x = (index as u32)
        .wrapping_mul(0x9E37_79B9)
        .wrapping_add(0xA5A5_1234);
    x ^= x >> 16;
    f32::from((x >> 16) as i16) / 32768.0
}

fn unit_tone(index: usize, hz: f32) -> f32 {
    (std::f32::consts::TAU * hz * index as f32 / SAMPLE_RATE as f32).sin()
}

fn far_unit(kind: Kind, index: usize) -> f32 {
    match kind {
        Kind::Tone => 0.5 * unit_tone(index, 440.0),
        Kind::Noise => 0.4 * unit_noise(index),
        Kind::Mix => 0.35 * unit_tone(index, 440.0) + 0.25 * unit_noise(index),
    }
}

fn near_unit(index: usize) -> f32 {
    0.25 * unit_tone(index, 180.0) + 0.1 * unit_tone(index, 360.0)
}

fn unit_to_pcm(sample: f32) -> i16 {
    let scaled = f64::from(sample) * 32768.0;
    if scaled >= 32767.0 {
        32767
    } else if scaled <= -32768.0 {
        -32768
    } else {
        scaled.round() as i16
    }
}

fn frame_of(kind: Kind, frame: usize) -> Vec<i16> {
    let origin = frame.saturating_mul(FRAME_SAMPLES);
    (0..FRAME_SAMPLES)
        .map(|index| unit_to_pcm(far_unit(kind, origin + index)))
        .collect()
}

fn user_tone() -> Vec<i16> {
    (0..FRAME_SAMPLES)
        .map(|index| unit_to_pcm(0.25 * unit_tone(index, 200.0)))
        .collect()
}

struct Room {
    hist: Vec<f32>,
}

impl Room {
    fn new() -> Self {
        Self { hist: Vec::new() }
    }

    fn render(&mut self, far: &[i16]) -> Vec<i16> {
        let start = self.hist.len();
        for sample in far {
            self.hist.push(f32::from(*sample) / 32768.0);
        }
        let mut mic = vec![0i16; far.len()];
        for (index, sample) in mic.iter_mut().enumerate() {
            let at = start + index;
            let mut echo = GAIN * tap(&self.hist, at, DELAY);
            for (lag, scale) in TAPS {
                echo += GAIN * scale * tap(&self.hist, at, DELAY + lag);
            }
            *sample = unit_to_pcm(echo);
        }
        mic
    }
}

fn tap(hist: &[f32], index: usize, lag: usize) -> f32 {
    if index >= lag {
        hist[index - lag]
    } else {
        0.0
    }
}

fn mix_near(mic: &mut [i16], origin: usize) {
    for (index, sample) in mic.iter_mut().enumerate() {
        let mixed = f32::from(*sample) / 32768.0 + near_unit(origin + index);
        *sample = unit_to_pcm(mixed);
    }
}

#[cfg(feature = "aec")]
fn power(frame: &[i16]) -> f64 {
    frame
        .iter()
        .map(|sample| {
            let x = f64::from(*sample) / 32768.0;
            x * x
        })
        .sum()
}

#[cfg(feature = "aec")]
fn erle_db(echo: f64, residual: f64) -> f64 {
    10.0 * (echo / residual.max(1.0e-20)).log10()
}

#[cfg(feature = "aec")]
struct Direct {
    erle_db: f64,
    near_ratio_db: f64,
    cpu_us: f64,
}

#[cfg(feature = "aec")]
fn measure(kind: Kind) -> Direct {
    let mut echo = PlaybackEcho::new();
    assert!(echo.enabled(), "{}", kind.name());
    let mut room = Room::new();
    let mut playing = vec![0i16; FRAME_SAMPLES];
    let mut echo_power = 0.0f64;
    let mut out_power = 0.0f64;
    let mut near_power = 0.0f64;
    let mut kept_power = 0.0f64;
    let mut cpu = std::time::Duration::ZERO;
    let mut timed = 0u32;
    let mut checksum = 0i64;
    for frame in 0usize..28 {
        let mut mic = room.render(&playing);
        let origin = frame.saturating_mul(FRAME_SAMPLES);
        let near_on = frame >= 20;
        if near_on {
            mix_near(&mut mic, origin);
        }
        let started = Instant::now();
        let out = echo.cancel(&mic).expect("frame");
        cpu += started.elapsed();
        timed = timed.saturating_add(1);
        checksum = checksum.wrapping_add(i64::from(out[0]));
        if (10..18).contains(&frame) {
            echo_power += power(&mic);
            out_power += power(&out);
        }
        if (22..28).contains(&frame) {
            let near: Vec<i16> = (0..FRAME_SAMPLES)
                .map(|index| unit_to_pcm(near_unit(origin + index)))
                .collect();
            near_power += power(&near);
            kept_power += power(&out);
            let mut sense = ProsodyEar::new();
            assert!(
                sense.hear(&out).voiced,
                "{} near-end frame {frame} was quiet after cancellation",
                kind.name()
            );
        }
        let next = frame_of(kind, frame);
        echo.note_playback(&next);
        playing = next;
    }
    assert_ne!(checksum, 0, "{}", kind.name());
    let cpu_us = cpu.as_secs_f64() * 1.0e6 / f64::from(timed);
    Direct {
        erle_db: erle_db(echo_power, out_power),
        near_ratio_db: erle_db(near_power, kept_power),
        cpu_us,
    }
}

struct Schedule {
    phase: Rc<Cell<u8>>,
    shown: usize,
    open: bool,
    frames: u32,
}

impl Schedule {
    fn new(phase: Rc<Cell<u8>>) -> Self {
        Self {
            phase,
            shown: 0,
            open: false,
            frames: 0,
        }
    }

    fn text(&mut self) -> String {
        self.shown = self.shown.saturating_add(1);
        match self.phase.get() {
            0 => {
                const WORDS: &[&str] = &["where", "is", "the", "kettle"];
                let count = self.shown.min(WORDS.len());
                WORDS[..count].join(" ")
            }
            1 => "yeah".to_string(),
            _ => {
                if self.shown == 1 {
                    "stop".to_string()
                } else {
                    "stop now".to_string()
                }
            }
        }
    }
}

impl Recognizer for Schedule {
    fn push_frame(&mut self, pcm: &[i16]) -> Result<Vec<Hypothesis>, RecogError> {
        if pcm.len() != FRAME_SAMPLES {
            return Err(RecogError::Frame {
                got: pcm.len(),
                expected: FRAME_SAMPLES,
            });
        }
        if !self.open {
            self.shown = 0;
            self.open = true;
        }
        self.frames = self.frames.saturating_add(1);
        Ok(vec![Hypothesis {
            kind: HypothesisKind::Partial,
            text: self.text(),
            start_ms: 0,
            end_ms: self.audio_ms(),
        }])
    }

    fn finish(&mut self) -> Result<Vec<Hypothesis>, RecogError> {
        self.open = false;
        self.shown = 0;
        Ok(Vec::new())
    }

    fn cancel(&mut self) -> Result<(), RecogError> {
        self.open = false;
        self.shown = 0;
        Ok(())
    }

    fn audio_ms(&self) -> u32 {
        self.frames.saturating_mul(FRAME_MS)
    }

    fn name(&self) -> &str {
        "schedule"
    }
}

struct FarMouth {
    kind: Kind,
    left: u32,
    index: usize,
    pending_first: bool,
    last: Vec<i16>,
}

impl FarMouth {
    fn new(kind: Kind) -> Self {
        Self {
            kind,
            left: 0,
            index: 0,
            pending_first: false,
            last: Vec::new(),
        }
    }

    fn last(&self) -> &[i16] {
        &self.last
    }
}

impl Mouth for FarMouth {
    fn id(&self) -> &str {
        "far"
    }

    fn start(&mut self, line: &SpokenLine) {
        self.last.clear();
        self.index = 0;
        if line.text.trim().is_empty() {
            self.left = 0;
            self.pending_first = false;
            return;
        }
        self.left = 64;
        self.pending_first = true;
    }

    fn pull(&mut self) -> MouthPull {
        if self.left == 0 {
            return MouthPull::Done;
        }
        self.left -= 1;
        let first = self.pending_first;
        self.pending_first = false;
        let samples = frame_of(self.kind, self.index);
        self.index = self.index.saturating_add(1);
        self.last.clone_from(&samples);
        MouthPull::Frame(AudioFrame {
            samples,
            sample_rate: SAMPLE_RATE,
            first,
        })
    }

    fn cancel(&mut self) {
        self.left = 0;
        self.pending_first = false;
        self.last.clear();
    }

    fn speaking(&self) -> bool {
        self.left > 0
    }
}

struct Scene {
    barges_before_near: u32,
    barge_ms: Option<u64>,
    voiced_before_near: u32,
}

fn scene(kind: Kind, aec: bool, near_after: Option<usize>) -> Scene {
    let phase = Rc::new(Cell::new(0u8));
    let mut duplex = Duplex::new(
        Parts {
            recognizer: Schedule::new(Rc::clone(&phase)),
            emotion: CoreAffect::samantha(),
            breath: SpeakingBreath,
            persona: SamanthaPersona,
            mouth: FarMouth::new(kind),
        },
        NoCards,
    )
    .expect("samantha");
    duplex.set_aec(aec);
    let mut saw_audio = false;
    for step in 0..16 {
        let pcm = if step < 5 {
            user_tone()
        } else {
            vec![0i16; FRAME_SAMPLES]
        };
        let frame = duplex.tick(&pcm).expect("opening");
        if step < 5 {
            assert!(frame.you_voiced, "opening frame {step} was quiet");
        }
        if frame.event == LoopEvent::FirstAudio {
            saw_audio = true;
            break;
        }
    }
    assert!(saw_audio, "{} never started", kind.name());
    let mut playing = duplex.mouth().last().to_vec();
    assert_eq!(playing.len(), FRAME_SAMPLES);
    let mut room = Room::new();
    phase.set(1);
    for _ in 0..GRACE_FRAMES {
        let mic = room.render(&playing);
        let frame = duplex.tick(&mic).expect("grace");
        assert_ne!(
            frame.event,
            LoopEvent::BargeIn,
            "{} barged during adaptation",
            kind.name()
        );
        playing = if frame.me_audible {
            duplex.mouth().last().to_vec()
        } else {
            vec![0i16; FRAME_SAMPLES]
        };
    }
    phase.set(2);
    let mut barges_before_near = 0u32;
    let mut voiced_before_near = 0u32;
    let mut barge_ms = None;
    let mut onset = None;
    let quiet_prefix = near_after.unwrap_or(8);
    for index in 0usize..(quiet_prefix + 6) {
        let mut mic = room.render(&playing);
        let with_near = near_after.is_some_and(|start| index >= start);
        if with_near {
            let origin = index.saturating_mul(FRAME_SAMPLES);
            mix_near(&mut mic, origin);
        }
        let frame = duplex.tick(&mic).expect("echo");
        if !with_near && frame.you_voiced {
            voiced_before_near = voiced_before_near.saturating_add(1);
        }
        if frame.you_voiced && onset.is_none() {
            onset = Some(frame.t_ms);
        }
        if !with_near && frame.event == LoopEvent::BargeIn {
            barges_before_near = barges_before_near.saturating_add(1);
        }
        if frame.event == LoopEvent::BargeIn {
            if let Some(start) = onset {
                barge_ms = Some(frame.t_ms.saturating_sub(start));
            }
            break;
        }
        playing = if frame.me_audible {
            duplex.mouth().last().to_vec()
        } else {
            vec![0i16; FRAME_SAMPLES]
        };
    }
    Scene {
        barges_before_near,
        barge_ms,
        voiced_before_near,
    }
}

#[test]
fn bypass_copies_the_microphone_and_a_short_frame_is_refused() {
    let mut echo = PlaybackEcho::new();
    echo.set_enabled(false);
    assert!(!echo.enabled());
    let mic: Vec<i16> = (0..FRAME_SAMPLES as i16)
        .map(|sample| sample.wrapping_mul(3))
        .collect();
    let out = echo.cancel(&mic).expect("copy");
    assert_eq!(out, mic);
    let err = echo.cancel(&[0i16; 4]).expect_err("short");
    assert_eq!(
        err,
        LoopError::Frame {
            got: 4,
            expected: FRAME_SAMPLES,
        }
    );
}

#[test]
fn self_echo_barges_when_the_canceller_is_bypassed() {
    for kind in [Kind::Tone, Kind::Noise, Kind::Mix] {
        let scene = scene(kind, false, None);
        assert!(
            scene.barges_before_near > 0,
            "{} bypass voiced {} frames and barged {}",
            kind.name(),
            scene.voiced_before_near,
            scene.barges_before_near
        );
        let barge_ms = scene.barge_ms.expect("bypass did not time the barge");
        assert!(
            barge_ms <= u64::from(FRAME_MS) * 2,
            "{} bypass barge {barge_ms} ms",
            kind.name()
        );
    }
}

#[cfg(feature = "aec")]
#[test]
fn synthetic_echo_is_removed_before_the_ear() {
    assert!(PlaybackEcho::compiled());
    assert!(ProsodyEar::compiled(), "barge-in follows the prosody clamp");
    assert_eq!(AEC_CRATE, "fdaf-aec");
    assert_eq!(AEC_LICENSE, "MIT");
    assert_eq!(AEC_VERSION, "0.1.0");
    let mut line = format!(
        "duplex s33 aec={AEC_CRATE} version={AEC_VERSION} license={AEC_LICENSE} delay_samples={DELAY} gain={GAIN} reverb_taps=48+96"
    );
    let mut slowest_us = 0.0f64;
    for kind in [Kind::Tone, Kind::Noise, Kind::Mix] {
        let measured = measure(kind);
        slowest_us = slowest_us.max(measured.cpu_us);
        let floor = match kind {
            Kind::Tone => 40.0,
            Kind::Noise => 20.0,
            Kind::Mix => 20.0,
        };
        assert!(
            measured.erle_db >= floor,
            "{} ERLE {:.2} dB, floor {floor}",
            kind.name(),
            measured.erle_db
        );
        assert!(
            measured.near_ratio_db.abs() <= 6.0,
            "{} near-end moved {:.2} dB",
            kind.name(),
            measured.near_ratio_db
        );
        assert!(
            measured.cpu_us < 20_000.0,
            "{} frame cost {:.1} us",
            kind.name(),
            measured.cpu_us
        );
        let heard = scene(kind, true, Some(4));
        assert_eq!(
            heard.barges_before_near,
            0,
            "{} false barges, voiced frames {}",
            kind.name(),
            heard.voiced_before_near
        );
        let barge_ms = heard.barge_ms.expect("near-end did not barge");
        assert!(barge_ms <= 160, "{} barge {barge_ms} ms", kind.name());
        line.push_str(&format!(
            " {name}_erle_db={erle:.2} {name}_near_delta_db={near:.2} {name}_false_barge=0 {name}_barge_ms={barge_ms} {name}_cpu_us={cpu:.1}",
            name = kind.name(),
            erle = measured.erle_db,
            near = measured.near_ratio_db,
            cpu = measured.cpu_us,
        ));
    }
    line.push_str(&format!(" cpu_us_max={slowest_us:.1}"));
    println!("{line}");
}
