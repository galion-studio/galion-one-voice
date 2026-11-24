// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Scripted duplex run. The input is a temp WAV. The mouth is Piper, flite,
//! or an untuned tone. Only a real mouth is timed.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use galion_emotion_core::{EmotionCore, IntentMsg, AROUSAL, DOMINANCE, VALENCE};
use horbm::{CardHit, CardLookup};
use horbm_duplex::{
    Affect, Breath, BreathCue, BreathPhase, Duplex, Emotion, LoopError, LoopEvent, Parts, Persona,
    Style, DEV_PERSONA, FRAME_SAMPLES,
};
use horbm_voice::SAMPLE_RATE;

use crate::mouth::{self, EngineKind, LiveMouth};
use crate::percentile_nearest;
use crate::percentile_nearest_f64;
use crate::recognizer::{self, EarRec};
use crate::report::{self, Report};
use crate::stages;
use crate::vad::{self, Energy};
use crate::wav;

pub const SCRIPT: &str = "where is the kettle";
pub const TRIALS: u32 = 11;
const BARGE: &str = "stop now";
const LINES: [&str; 3] = [SCRIPT, "yeah", BARGE];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sense {
    On,
    Off,
}

pub struct Outcome {
    pub report: Report,
    pub mouth_real: bool,
    pub asr_real: bool,
    pub spoke_kettle: bool,
    pub wav_removed: bool,
    pub first_audio_p50_us: Option<u64>,
    pub first_audio_p95_us: Option<u64>,
    pub barge_stop_ns: Option<u64>,
    pub rtf: Option<f64>,
    pub conductor_barge_ms: Option<u64>,
    pub voiced_frames: u32,
}

struct TempTree {
    dir: PathBuf,
}

impl TempTree {
    fn new() -> Result<Self, String> {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("horbm-e2e-{}-{nanos}", std::process::id()));
        fs::create_dir_all(&dir).map_err(|err| format!("temp dir: {err}"))?;
        Ok(Self { dir })
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        if self.dir.starts_with(std::env::temp_dir()) {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }
}

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

struct CoreEmotion {
    core: EmotionCore,
}

impl Emotion for CoreEmotion {
    fn step(&mut self, t_ms: u32) -> Affect {
        let msg = IntentMsg::steady(t_ms);
        let moment = self.core.step(&msg).unwrap_or_else(|err| {
            panic!("emotion step at {t_ms} ms failed: {err:?}");
        });
        Affect {
            valence: moment.pos[VALENCE],
            arousal: moment.pos[AROUSAL],
            dominance: moment.pos[DOMINANCE],
        }
    }
}

struct QuietBreath;

impl Breath for QuietBreath {
    fn cue(&mut self, _t_ms: u32, speaking: bool) -> BreathCue {
        if speaking {
            BreathCue {
                phase: BreathPhase::Exhale,
                speaking: true,
            }
        } else {
            BreathCue {
                phase: BreathPhase::Inhale,
                speaking: false,
            }
        }
    }
}

struct Samantha;

impl Persona for Samantha {
    fn id(&self) -> &str {
        DEV_PERSONA
    }

    fn voice(&self) -> &str {
        DEV_PERSONA
    }

    fn style(&self) -> Style {
        Style::from_brevity(0.55, 0.8, 2)
    }

    fn unknown_line(&self) -> &str {
        horbm::UNKNOWN_REPLY
    }
}

struct Other;

impl Persona for Other {
    fn id(&self) -> &str {
        "aria"
    }

    fn voice(&self) -> &str {
        "aria"
    }

    fn style(&self) -> Style {
        Style::from_brevity(0.4, 0.7, 2)
    }

    fn unknown_line(&self) -> &str {
        horbm::UNKNOWN_REPLY
    }
}

struct Trial {
    first_audio_us: Option<u64>,
    barge_ns: Option<u64>,
    queued: usize,
    rtf: Option<f64>,
    conductor_barge_ms: Option<u64>,
    spoken: Vec<String>,
    hypothesis: Option<String>,
}

pub fn run(sense: Sense) -> Result<Outcome, String> {
    let tree = TempTree::new()?;
    let kind = mouth::detect();
    let question = utterance(kind, SCRIPT)?;
    let barge = utterance(kind, BARGE)?.unwrap_or_else(|| tone_words(2));
    let question = question.unwrap_or_else(|| tone_words(4));
    let pcm = with_tail(question);
    let path = tree.dir.join("input.wav");
    wav::write_wav(&path, SAMPLE_RATE, &pcm)?;
    let (rate, read) = wav::read_wav(&path)?;
    if rate != SAMPLE_RATE || read != pcm {
        return Err("input wav did not round-trip".into());
    }
    let counts = vad::count_frames(&read);
    if counts.voiced == 0 || counts.quiet == 0 {
        return Err("vad did not hear both speech and silence in the input wav".into());
    }
    let question_frames = gate_frames(&read);
    let stop_frames = take_voiced(&barge, 3);
    let yeah = loud_frame();
    let want_asr = recognizer::asr_available();

    let mut first_audio = Vec::new();
    let mut barge_ns = Vec::new();
    let mut rtfs = Vec::new();
    let mut conductor = Vec::new();
    let mut spoke_kettle = false;
    let mut hypothesis = None;
    let mut queued = 0usize;
    let mut asr_real = false;

    for trial in 0..TRIALS {
        let one = drive(kind, want_asr, &question_frames, &yeah, &stop_frames)?;
        if trial == 0 {
            hypothesis = one.hypothesis.clone();
            asr_real = want_asr;
        }
        if one
            .spoken
            .iter()
            .any(|line| line.to_ascii_lowercase().contains("kettle"))
        {
            spoke_kettle = true;
        }
        if let Some(us) = one.first_audio_us {
            first_audio.push(us);
        }
        if let Some(ns) = one.barge_ns {
            barge_ns.push(ns);
            queued = queued.max(one.queued);
        }
        if let Some(rtf) = one.rtf {
            rtfs.push(rtf);
        }
        if let Some(ms) = one.conductor_barge_ms {
            conductor.push(ms);
        }
    }

    let mouth_real = kind != EngineKind::Stub;
    if mouth_real && (first_audio.len() != TRIALS as usize || barge_ns.len() != TRIALS as usize) {
        return Err("a real mouth did not time first audio and barge-in on every trial".into());
    }
    if !mouth_real && (!first_audio.is_empty() || !barge_ns.is_empty()) {
        return Err("a stub mouth was timed".into());
    }
    if !want_asr && conductor.len() != TRIALS as usize {
        return Err("the conductor did not barge on a scripted trial".into());
    }

    let wav_removed = remove_tree(&tree.dir);
    let mut report = Report::new();
    report.push("persona", DEV_PERSONA);
    report.push(
        "profile",
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        },
    );
    report.push(
        "prosody_sense",
        if sense == Sense::On { "on" } else { "off" },
    );
    report.push("trials", TRIALS.to_string());
    report.push("vad", "energy");
    report.push("vad_voiced_frames", counts.voiced.to_string());
    report.push("vad_quiet_frames", counts.quiet.to_string());
    report.push("asr", if asr_real { "vosk" } else { "stub" });
    report.push("prosody", "stub");
    report.push("end_of_turn", "stub");
    report.push("brain", "real");
    report.push("brain_spoke_kettle", if spoke_kettle { "1" } else { "0" });
    report.push("dialogue", "stub");
    report.push("person_memory", "stub");
    report.push("listener_feeling", "stub");
    report.push("emotion", "real");
    report.push("mood_into_voice", "stub");
    report.push("breath", "stub");
    report.push("mouth", kind.name());
    report.push("piper_duplex", "stub");
    if mouth_real {
        report.push("output_buffer", "loopback");
        report.push("output_buffer_samples", queued.to_string());
    } else {
        report.push("output_buffer", "stub");
    }
    push_audio(&mut report, mouth_real, &first_audio, &barge_ns, &rtfs);
    report.push("wer", report::format_wer(SCRIPT, hypothesis.as_deref()));
    match percentile_nearest(&sorted(&conductor), 0.50) {
        Some(ms) => {
            report.push("conductor_barge_ms", ms.to_string());
            report.push("conductor_barge_clock", "virtual");
        }
        None => report.push("conductor_barge_ms", "stub"),
    }
    stages::fill(&mut report, sense, SCRIPT)?;

    Ok(Outcome {
        report,
        mouth_real,
        asr_real,
        spoke_kettle,
        wav_removed,
        first_audio_p50_us: stat(&first_audio, 0.50),
        first_audio_p95_us: stat(&first_audio, 0.95),
        barge_stop_ns: stat(&barge_ns, 0.50),
        rtf: percentile_nearest_f64(rtfs.clone(), 0.50),
        conductor_barge_ms: stat(&conductor, 0.50),
        voiced_frames: counts.voiced,
    })
}

fn push_audio(report: &mut Report, mouth_real: bool, first: &[u64], barge: &[u64], rtfs: &[f64]) {
    if !mouth_real {
        report.push("first_audio_p50_ms", "stub");
        report.push("first_audio_p95_ms", "stub");
        report.push("barge_in_stop_ns", "stub");
        report.push("rtf", "stub");
        return;
    }
    let p50 = stat(first, 0.50).unwrap_or(0);
    let p95 = stat(first, 0.95).unwrap_or(0);
    report.push("first_audio_p50_ms", format!("{:.3}", p50 as f64 / 1000.0));
    report.push("first_audio_p95_ms", format!("{:.3}", p95 as f64 / 1000.0));
    report.push(
        "barge_in_stop_ns",
        stat(barge, 0.50).unwrap_or(0).to_string(),
    );
    let rtf = percentile_nearest_f64(rtfs.to_vec(), 0.50).unwrap_or(0.0);
    report.push("rtf", format!("{rtf:.4}"));
    report.push("rtf_def", "wall/audio");
}

fn stat(samples: &[u64], quantile: f64) -> Option<u64> {
    percentile_nearest(&sorted(samples), quantile)
}

fn sorted(samples: &[u64]) -> Vec<u64> {
    let mut copy = samples.to_vec();
    copy.sort_unstable();
    copy
}

fn drive(
    kind: EngineKind,
    want_asr: bool,
    question: &[Vec<i16>],
    yeah: &[i16],
    stop: &[Vec<i16>],
) -> Result<Trial, String> {
    let recognizer = EarRec::new(&LINES, want_asr);
    if want_asr && !recognizer.using_asr() {
        return Err("vosk opened once and then did not".into());
    }
    let finals = recognizer.final_handle();
    let mut duplex = Duplex::new(
        Parts {
            recognizer,
            emotion: CoreEmotion {
                core: EmotionCore::at_rest(),
            },
            breath: QuietBreath,
            persona: Samantha,
            mouth: LiveMouth::open(kind)?,
        },
        Kettle,
    )
    .map_err(|err| err.to_string())?;

    let mut speech_end = None;
    let mut heard_voice = false;
    let mut first_audio = None;
    let mut playing = false;
    let mut barge_onset = None;
    let mut conductor_barge_ms = None;

    for frame in question {
        note_end(frame, &mut heard_voice, &mut speech_end);
        let out = duplex.tick(frame).map_err(|err| err.to_string())?;
        note_audio(&mut first_audio, speech_end, duplex.mouth().buffered_at());
        if out.me_first || out.event == LoopEvent::FirstAudio {
            playing = true;
        }
        note_barge(&out, &mut barge_onset, &mut conductor_barge_ms);
    }
    // A stub mouth never stamps a wall-clock buffer time. Waiting on that
    // stamp pulls the tone dry before the barge. Stop once a mouth frame
    // has actually played.
    let mut extra = 0u32;
    while !playing && extra < 8 {
        let silence = vec![0i16; FRAME_SAMPLES];
        note_end(&silence, &mut heard_voice, &mut speech_end);
        let out = duplex.tick(&silence).map_err(|err| err.to_string())?;
        note_audio(&mut first_audio, speech_end, duplex.mouth().buffered_at());
        if out.me_first || out.event == LoopEvent::FirstAudio {
            playing = true;
        }
        note_barge(&out, &mut barge_onset, &mut conductor_barge_ms);
        extra += 1;
    }
    for _ in 0..6 {
        let out = duplex.tick(yeah).map_err(|err| err.to_string())?;
        note_barge(&out, &mut barge_onset, &mut conductor_barge_ms);
    }
    let silence = vec![0i16; FRAME_SAMPLES];
    let out = duplex.tick(&silence).map_err(|err| err.to_string())?;
    note_barge(&out, &mut barge_onset, &mut conductor_barge_ms);
    for frame in stop {
        let out = duplex.tick(frame).map_err(|err| err.to_string())?;
        note_barge(&out, &mut barge_onset, &mut conductor_barge_ms);
    }

    if let Some(err) = duplex.mouth().error() {
        return Err(err.to_string());
    }
    let mouth_real = duplex.mouth().real();
    if !mouth_real {
        first_audio = None;
    }
    let hypothesis = finals.borrow().clone();
    Ok(Trial {
        first_audio_us: first_audio,
        barge_ns: if mouth_real {
            duplex.mouth().stop_ns()
        } else {
            None
        },
        queued: duplex.mouth().queued_at_barge(),
        rtf: if mouth_real {
            duplex.mouth().rtf()
        } else {
            None
        },
        conductor_barge_ms,
        spoken: duplex.mouth().spoken().to_vec(),
        hypothesis,
    })
}

fn note_end(frame: &[i16], heard_voice: &mut bool, speech_end: &mut Option<Instant>) {
    let voiced = frame.iter().any(|sample| *sample != 0);
    if voiced {
        *heard_voice = true;
    } else if *heard_voice && speech_end.is_none() {
        *speech_end = Some(Instant::now());
    }
}

fn note_audio(first: &mut Option<u64>, speech_end: Option<Instant>, buffered: Option<Instant>) {
    if first.is_some() {
        return;
    }
    if let (Some(start), Some(at)) = (speech_end, buffered) {
        *first = Some(
            u64::try_from(at.saturating_duration_since(start).as_micros()).unwrap_or(u64::MAX),
        );
    }
}

fn note_barge(out: &horbm_duplex::FrameOut, onset: &mut Option<u64>, barge: &mut Option<u64>) {
    if onset.is_none()
        && out.you_voiced
        && out.me_audible
        && out
            .partial
            .split_whitespace()
            .any(|word| word.eq_ignore_ascii_case("stop"))
    {
        *onset = Some(out.t_ms);
    }
    if out.event == LoopEvent::BargeIn {
        if let Some(start) = *onset {
            *barge = Some(out.t_ms.saturating_sub(start));
        }
    }
}

fn utterance(kind: EngineKind, text: &str) -> Result<Option<Vec<i16>>, String> {
    mouth::speak_16k(kind, text)
}

fn with_tail(mut pcm: Vec<i16>) -> Vec<i16> {
    let tail = usize::try_from(SAMPLE_RATE).unwrap_or(16_000) * 800 / 1000;
    pcm.extend(std::iter::repeat(0).take(tail));
    pcm
}

fn gate_frames(pcm: &[i16]) -> Vec<Vec<i16>> {
    let mut energy = Energy::default();
    let mut frames = Vec::new();
    let mut offset = 0;
    while offset < pcm.len() {
        let mut frame = vec![0i16; FRAME_SAMPLES];
        let end = (offset + FRAME_SAMPLES).min(pcm.len());
        frame[..end - offset].copy_from_slice(&pcm[offset..end]);
        if !vad::eighty_ms_voiced(&mut energy, &frame) {
            frame.fill(0);
        }
        frames.push(frame);
        offset += FRAME_SAMPLES;
    }
    frames
}

fn take_voiced(pcm: &[i16], count: usize) -> Vec<Vec<i16>> {
    let voiced: Vec<Vec<i16>> = gate_frames(pcm)
        .into_iter()
        .filter(|frame| frame.iter().any(|sample| *sample != 0))
        .collect();
    (0..count)
        .map(|index| {
            if voiced.is_empty() {
                loud_frame()
            } else {
                voiced[index % voiced.len()].clone()
            }
        })
        .collect()
}

fn tone_words(words: usize) -> Vec<i16> {
    let mut out = Vec::new();
    let rate = usize::try_from(SAMPLE_RATE).unwrap_or(16_000);
    for _ in 0..words {
        let n = rate * 180 / 1000;
        for index in 0..n {
            let t = index as f32 / rate as f32;
            let sample = (t * 220.0 * std::f32::consts::TAU).sin();
            out.push((sample * 8_000.0) as i16);
        }
        out.extend(std::iter::repeat(0).take(rate * 40 / 1000));
    }
    out
}

fn loud_frame() -> Vec<i16> {
    let mut frame = vec![0i16; FRAME_SAMPLES];
    let rate = SAMPLE_RATE as f32;
    for (index, sample) in frame.iter_mut().enumerate() {
        let t = index as f32 / rate;
        *sample = ((t * 180.0 * std::f32::consts::TAU).sin() * 8_000.0) as i16;
    }
    frame
}

fn remove_tree(dir: &Path) -> bool {
    if dir.starts_with(std::env::temp_dir()) {
        let _ = fs::remove_dir_all(dir);
    }
    !dir.exists()
}

pub fn refuse_other_persona() -> Result<(), String> {
    match Duplex::new(
        Parts {
            recognizer: EarRec::scripted(&[]),
            emotion: CoreEmotion {
                core: EmotionCore::at_rest(),
            },
            breath: QuietBreath,
            persona: Other,
            mouth: LiveMouth::open(EngineKind::Stub)?,
        },
        Kettle,
    ) {
        Ok(_) => Err("a persona other than samantha was accepted".into()),
        Err(LoopError::Persona(_)) => Ok(()),
        Err(err) => Err(err.to_string()),
    }
}
