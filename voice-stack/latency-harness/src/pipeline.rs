// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! One timed turn: end of speech, ASR final, reply, first TTS sample,
//! first heartbeat frame in the sink. The heartbeat play time is derived
//! from those arrivals. It is not a sleep.

use crate::clock::{
    heartbeat_play_us, input_samples_for_one_frame, resample_to_frame, FRAME_SAMPLES,
};
use crate::machine::{probe_machine, repo_root, Machine};
use crate::piper::PiperInfo;
use crate::report::{ClockKind, ColdClocks, Rep, Report, StageStat};
use crate::stats::percentile;
use crate::traits::{
    AsrStage, AudioOut, ReplyEngine, SpeechEnd, StageError, StageNames, TtsStage, PHRASE,
};
use std::time::{Duration, Instant};

const PULL_TIMEOUT: Duration = Duration::from_secs(15);
/// Below this peak the frame is treated as silence and the run fails.
const MIN_PEAK: f32 = 1.0e-3;

#[derive(Debug)]
pub enum HarnessError {
    /// The backend is not on this machine. No percentiles.
    Absent {
        reason: String,
    },
    Failed(StageError),
}

impl std::fmt::Display for HarnessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HarnessError::Absent { reason } => write!(f, "absent: {reason}"),
            HarnessError::Failed(e) => write!(f, "{e}"),
        }
    }
}

pub struct MeasureIn<'a> {
    pub pcm: &'a [i16],
    pub pcm_rate: u32,
    pub reps: usize,
    pub warmup: usize,
    pub backend: &'static str,
    pub claims_tts_measured: bool,
    pub voice: Option<String>,
    pub cold: Option<ColdClocks>,
    pub piper: Option<PiperInfo>,
    pub notes: Vec<String>,
}

pub fn measure<V, A, R, T, O>(
    vad: &mut V,
    asr: &mut A,
    reply: &mut R,
    tts: &mut T,
    audio_out: &mut O,
    input: MeasureIn<'_>,
) -> Result<Report, HarnessError>
where
    V: SpeechEnd,
    A: AsrStage,
    R: ReplyEngine,
    T: TtsStage,
    O: AudioOut,
{
    if input.reps == 0 {
        return Err(HarnessError::Failed(StageError::new(
            "harness",
            "reps must be > 0",
        )));
    }
    let names = StageNames {
        vad: vad.name().to_string(),
        asr: asr.name().to_string(),
        reply: reply.name().to_string(),
        tts: tts.name().to_string(),
        audio_out: audio_out.name().to_string(),
    };
    for i in 0..input.warmup {
        eprintln!("s12 {} warmup {}/{}", input.backend, i + 1, input.warmup);
        one_turn(vad, asr, reply, tts, audio_out, input.pcm, input.pcm_rate).map_err(|e| {
            HarnessError::Failed(StageError::new("warmup", format!("warmup {i} failed: {e}")))
        })?;
    }
    let mut reps = Vec::with_capacity(input.reps);
    let mut last_mood = None;
    for i in 0..input.reps {
        if i == 0 || (i + 1) % 10 == 0 || i + 1 == input.reps {
            eprintln!("s12 {} rep {}/{}", input.backend, i + 1, input.reps);
        }
        let rep =
            one_turn(vad, asr, reply, tts, audio_out, input.pcm, input.pcm_rate).map_err(|e| {
                HarnessError::Failed(StageError::new("rep", format!("rep {i} failed: {e}")))
            })?;
        last_mood = Some(rep.mood);
        reps.push(rep.rep);
    }
    let machine = probe_machine(&repo_root());
    Ok(Report::measured(ReportParts {
        backend: input.backend.to_string(),
        machine,
        names,
        reps,
        warmup: input.warmup,
        claims_tts_measured: input.claims_tts_measured,
        voice: input.voice,
        cold: input.cold,
        piper: input.piper,
        notes: input.notes,
        last_mood,
        phrase: PHRASE.to_string(),
    }))
}

struct TurnBits {
    rep: Rep,
    mood: crate::Pad,
}

fn one_turn<V, A, R, T, O>(
    vad: &mut V,
    asr: &mut A,
    reply: &mut R,
    tts: &mut T,
    audio_out: &mut O,
    pcm: &[i16],
    pcm_rate: u32,
) -> Result<TurnBits, StageError>
where
    V: SpeechEnd,
    A: AsrStage,
    R: ReplyEngine,
    T: TtsStage,
    O: AudioOut,
{
    let t0 = Instant::now();
    let end = vad.end_sample(pcm, pcm_rate)?;
    let eos = Instant::now();
    if end == 0 || end > pcm.len() {
        return Err(StageError::new("vad", "end sample out of range"));
    }
    let heard = asr.finalize(&pcm[..end], pcm_rate)?;
    let asr_at = Instant::now();
    let elapsed = us_between(eos, asr_at)?;
    reply.on_heartbeat(elapsed);
    let turn = reply.turn(&heard)?;
    let reply_at = Instant::now();
    reply.on_heartbeat(us_between(eos, reply_at)?);
    if turn.reply.trim().is_empty() {
        return Err(StageError::new("reply", "empty reply"));
    }

    tts.begin(&turn.reply, &turn.mood)?;
    let mut buf: Vec<i16> = Vec::new();
    let mut rate = 0u32;
    let mut first_at = None;
    let mut need = 0usize;
    let frame_at = loop {
        match tts.pull(PULL_TIMEOUT)? {
            None => {
                return Err(StageError::new(
                    "tts",
                    "utterance ended before one heartbeat frame",
                ));
            }
            Some(chunk) => {
                if chunk.samples.is_empty() {
                    continue;
                }
                if rate == 0 {
                    rate = chunk.sample_rate;
                    need = input_samples_for_one_frame(rate);
                    if need == 0 {
                        return Err(StageError::new("tts", "bad sample rate"));
                    }
                } else if chunk.sample_rate != rate {
                    return Err(StageError::new("tts", "sample rate changed mid-utterance"));
                }
                if first_at.is_none() {
                    first_at = Some(chunk.at);
                }
                let at = chunk.at;
                buf.extend_from_slice(&chunk.samples);
                if buf.len() >= need {
                    break at;
                }
            }
        }
    };
    let first_at = first_at.ok_or_else(|| StageError::new("tts", "no pcm"))?;
    let reply_to_first_slice_us = us_between(reply_at, frame_at)?;
    let mut frame = [0.0f32; FRAME_SAMPLES];
    resample_to_frame(&buf[..need], rate, &mut frame);
    audio_out.emit_frame(&frame)?;
    let out_at = Instant::now();
    let peak = frame.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    if peak < MIN_PEAK {
        return Err(StageError::new("audio_out", "first frame was silent"));
    }
    // Drain sits outside the clocks.
    tts.finish()?;

    let vad_us = us_between(t0, eos)?;
    let eos_to_asr_us = us_between(eos, asr_at)?;
    let asr_to_reply_us = us_between(asr_at, reply_at)?;
    let reply_to_tts_us = us_between(reply_at, first_at)?;
    let tts_to_audio_out_us = us_between(first_at, out_at)?;
    let eos_to_audio_out_us = us_between(eos, out_at)?;
    let first_pcm_us = us_between(eos, first_at)?;
    let frame_arrived_us = us_between(eos, frame_at)?;
    let eos_to_heartbeat_play_us = heartbeat_play_us(frame_arrived_us, first_pcm_us);
    let sink_peak_milli = (peak * 1000.0) as u32;

    Ok(TurnBits {
        mood: turn.mood,
        rep: Rep {
            vad_us,
            eos_to_asr_us,
            asr_to_reply_us,
            reply_to_tts_us,
            reply_to_first_slice_us,
            tts_to_audio_out_us,
            eos_to_audio_out_us,
            eos_to_heartbeat_play_us,
            mood_ticks: (us_between(eos, reply_at)? / (crate::FRAME_MS as u64 * 1000)) as u32,
            sink_peak_milli,
        },
    })
}

fn us_between(earlier: Instant, later: Instant) -> Result<u64, StageError> {
    match later.checked_duration_since(earlier) {
        Some(d) => Ok(u64::try_from(d.as_micros()).unwrap_or(u64::MAX)),
        None => Err(StageError::new(
            "clock",
            "a later stamp is before an earlier one",
        )),
    }
}

pub struct ReportParts {
    pub backend: String,
    pub machine: Machine,
    pub names: StageNames,
    pub reps: Vec<Rep>,
    pub warmup: usize,
    pub claims_tts_measured: bool,
    pub voice: Option<String>,
    pub cold: Option<ColdClocks>,
    pub piper: Option<PiperInfo>,
    pub notes: Vec<String>,
    pub last_mood: Option<crate::Pad>,
    pub phrase: String,
}

pub fn summarize(reps: &[Rep]) -> Vec<StageStat> {
    fn col(reps: &[Rep], f: impl Fn(&Rep) -> u64) -> Vec<u64> {
        reps.iter().map(f).collect()
    }
    let specs: [(&str, &str, ClockKind, fn(&Rep) -> u64); 8] = [
        ("vad_us", "vad (offline buffer)", ClockKind::Measured, |r| {
            r.vad_us
        }),
        (
            "eos_to_asr_us",
            "end of speech to ASR final",
            ClockKind::Measured,
            |r| r.eos_to_asr_us,
        ),
        (
            "asr_to_reply_us",
            "ASR final to reply ready",
            ClockKind::Measured,
            |r| r.asr_to_reply_us,
        ),
        (
            "reply_to_tts_us",
            "reply ready to first TTS audio",
            ClockKind::Measured,
            |r| r.reply_to_tts_us,
        ),
        (
            "reply_to_first_slice_us",
            "reply ready to first slice",
            ClockKind::Measured,
            |r| r.reply_to_first_slice_us,
        ),
        (
            "tts_to_audio_out_us",
            "first TTS audio to first audio out",
            ClockKind::Measured,
            |r| r.tts_to_audio_out_us,
        ),
        (
            "eos_to_audio_out_us",
            "end of speech to first audio out",
            ClockKind::Measured,
            |r| r.eos_to_audio_out_us,
        ),
        (
            "eos_to_heartbeat_play_us",
            "end of speech to heartbeat play",
            ClockKind::Derived,
            |r| r.eos_to_heartbeat_play_us,
        ),
    ];
    specs
        .into_iter()
        .map(|(key, title, clock, f)| stat(key, title, clock, &col(reps, f)))
        .collect()
}

fn stat(key: &'static str, title: &'static str, clock: ClockKind, samples: &[u64]) -> StageStat {
    let mut v = samples.to_vec();
    v.sort_unstable();
    StageStat {
        key,
        title,
        clock,
        n: v.len(),
        min_us: v[0],
        p50_us: percentile(&v, 50, 100),
        p95_us: percentile(&v, 95, 100),
        p99_us: percentile(&v, 99, 100),
        max_us: *v.last().unwrap(),
    }
}
