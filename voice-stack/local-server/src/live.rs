// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Mic to ear to HORBM cards to Piper.
//!
//! Silero runs in `vad-ear` (its own workspace, so `ort` stays out of this
//! one). A quiet gap of 640 ms ends one turn. Vosk stays on a worker thread
//! because its handles are not `Send`. The mouth is Piper when the LJ Speech
//! pack is on disk. The brain is [`crate::brain::HorbmMind`]: read-only cards,
//! and `I don't know.` when none match. `Brain::open` in the orchestrator
//! stays unlinked. This path does not open `horbm-memory`'s sqlite file.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Sender};
use std::sync::Arc;
#[cfg(test)]
use std::sync::Mutex;
use std::thread::JoinHandle;
use std::time::Duration;

use galion_sam_asr::{final_text, open_engine, Asr, EngineOpts};
use galion_sam_horbm::speakable_chunks;
use galion_sam_orch::{AudioFrame, EnergyVad, Vad, VadEvent};
use galion_tts_piper::{resample_linear, PiperTts, Tts};

use crate::audio::{self, rms, SPEAKER_RATE};
use crate::brain::HorbmMind;
use crate::pipeline::{PipeInfo, Pipeline, TurnProduct};
use crate::protocol::{dot_json, error_json, turn_json, vad_json};
use crate::reply::{may_speak_piper, ReplyEngine};

const FRAME: usize = 320;
const SPEECH_RMS: f32 = 0.02;
/// Silence that ends one question. 160 ms split a sentence into two turns.
const HANG_MS: f64 = 640.0;
const UTTERANCE_CAP: usize = 16_000 * 16;

pub fn live_factory() -> Arc<dyn Fn() -> Box<dyn crate::pipeline::Pipeline> + Send + Sync> {
    Arc::new(|| Box::new(LivePipeline::from_env()))
}

enum Front {
    Child(ChildVad),
    Energy(EnergyVad),
}

enum Ear {
    #[cfg(test)]
    Script(String),
    Missing,
    Vosk(Sender<Job>),
}

struct Job {
    pcm: Vec<i16>,
    back: Sender<Result<String, String>>,
}

trait Mouth: Send {
    fn speak(&mut self, text: &str, mood: crate::reply::Pad) -> Result<(Vec<i16>, u32), String>;
    fn name(&self) -> &'static str;
}

struct MissingMouth;

impl Mouth for MissingMouth {
    fn speak(&mut self, _: &str, _: crate::reply::Pad) -> Result<(Vec<i16>, u32), String> {
        Err("piper-missing".into())
    }
    fn name(&self) -> &'static str {
        "missing"
    }
}

struct PiperMouth {
    tts: PiperTts,
}

impl Mouth for PiperMouth {
    fn speak(&mut self, text: &str, mood: crate::reply::Pad) -> Result<(Vec<i16>, u32), String> {
        let pad = galion_tts_piper::Pad {
            arousal: mood.arousal,
            valence: mood.valence,
            dominance: mood.dominance,
        };
        let pieces = speakable_chunks(text);
        let pieces: Vec<String> = if pieces.is_empty() {
            vec![text.to_string()]
        } else {
            pieces.into_iter().map(|c| c.text).collect()
        };
        let mut pcm = Vec::new();
        for piece in pieces {
            let mut part = Vec::new();
            let mut rate = SPEAKER_RATE;
            self.tts
                .speak(&piece, pad, &mut |chunk| {
                    rate = chunk.sample_rate;
                    part.extend_from_slice(chunk.samples);
                })
                .map_err(|_| "piper-missing".to_string())?;
            pcm.extend(resample_linear(&part, rate, SPEAKER_RATE));
        }
        if pcm.is_empty() {
            return Err("piper-missing".into());
        }
        Ok((pcm, SPEAKER_RATE))
    }
    fn name(&self) -> &'static str {
        "piper"
    }
}

#[cfg(test)]
struct RecMouth {
    heard: Arc<Mutex<Vec<String>>>,
}

#[cfg(test)]
impl Mouth for RecMouth {
    fn speak(&mut self, text: &str, _: crate::reply::Pad) -> Result<(Vec<i16>, u32), String> {
        if let Ok(mut guard) = self.heard.lock() {
            guard.push(text.to_string());
        }
        Ok((vec![1000, -1000, 1000, -1000], SPEAKER_RATE))
    }
    fn name(&self) -> &'static str {
        "sample"
    }
}

pub struct LivePipeline {
    front: Front,
    ear: Ear,
    mind: HorbmMind,
    mouth: Box<dyn Mouth>,
    vad_backend: &'static str,
    vad_ready: bool,
    ear_name: &'static str,
    phase: &'static str,
    level: f32,
    speech: Vec<i16>,
    in_speech: bool,
    utterance: Vec<i16>,
    _worker: Option<JoinHandle<()>>,
}

impl LivePipeline {
    pub fn from_env() -> Self {
        let (front, backend, ready) = open_vad();
        let (ear, ear_name, worker) = open_ear();
        let mouth = open_mouth();
        Self {
            front,
            ear,
            mind: HorbmMind::from_env(),
            mouth,
            vad_backend: backend,
            vad_ready: ready,
            ear_name,
            phase: "idle",
            level: 0.0,
            speech: Vec::new(),
            in_speech: false,
            utterance: Vec::new(),
            _worker: worker,
        }
    }

    #[cfg(test)]
    fn for_test(
        front: Front,
        backend: &'static str,
        ready: bool,
        ear: Ear,
        ear_name: &'static str,
        mind: HorbmMind,
        mouth: Box<dyn Mouth>,
    ) -> Self {
        Self {
            front,
            ear,
            mind,
            mouth,
            vad_backend: backend,
            vad_ready: ready,
            ear_name,
            phase: "idle",
            level: 0.0,
            speech: Vec::new(),
            in_speech: false,
            utterance: Vec::new(),
            _worker: None,
        }
    }

    fn fail(&mut self, code: &str) -> TurnProduct {
        self.phase = "idle";
        TurnProduct::events(vec![error_json(code), dot_json("idle", self.level)])
    }

    fn hear(&mut self, text: &str, t_ms: u64, preface: Vec<String>) -> TurnProduct {
        let started = std::time::Instant::now();
        let mut out = match self.mind.turn(text) {
            Ok(out) => out,
            Err(code) => return self.fail(&code),
        };
        if out.took_us == 0 {
            out.took_us = u32::try_from(started.elapsed().as_micros()).unwrap_or(u32::MAX);
        }
        out.mood = out.mood.finite_or_resting();
        let reply = out.reply.clone();
        let mut events = preface;
        events.push(turn_json(&out));
        if !may_speak_piper(&reply, out.violations) {
            self.phase = "idle";
            events.push(dot_json("idle", self.level));
            return TurnProduct {
                events,
                pcm: Vec::new(),
                rate: SPEAKER_RATE,
                reply,
            };
        }
        match self.mouth.speak(&reply, out.mood) {
            Ok((pcm, rate)) => {
                self.phase = "idle";
                events.push(dot_json("speaking", self.level));
                TurnProduct {
                    events,
                    pcm,
                    rate,
                    reply,
                }
            }
            Err(code) => {
                let _ = t_ms;
                self.fail(&code)
            }
        }
    }

    fn push_frame(&mut self, frame: &[i16]) -> Option<&'static str> {
        self.level = rms(frame).clamp(0.0, 1.0);
        match &mut self.front {
            Front::Energy(vad) => {
                let event = vad.push(&AudioFrame {
                    pcm: frame.to_vec(),
                    rate_hz: 16_000,
                    ms: 20,
                });
                match event {
                    VadEvent::Start => Some("start"),
                    VadEvent::End => Some("end"),
                    VadEvent::None => None,
                }
            }
            Front::Child(child) => match child.push(frame) {
                Some("start") => Some("start"),
                Some("end") => Some("end"),
                _ => None,
            },
        }
    }
}

impl Pipeline for LivePipeline {
    fn push_mic(&mut self, pcm: &[i16], rate: u32, t_ms: u64) -> TurnProduct {
        if rate != 16_000 && rate != 24_000 {
            return self.fail("bad-rate");
        }
        let frame_n = audio::samples_per_frame(rate, 20);
        if frame_n == 0 {
            return self.fail("bad-frame");
        }
        self.speech.extend_from_slice(pcm);
        let mut events = Vec::new();
        let mut buf = std::mem::take(&mut self.speech);
        while buf.len() >= frame_n {
            let frame: Vec<i16> = buf.drain(..frame_n).collect();
            let kind = self.push_frame(&frame);
            let piece = frame_16k(&frame, rate);
            match kind {
                Some("start") => {
                    self.in_speech = true;
                    self.utterance.clear();
                    self.utterance.extend_from_slice(&piece);
                    self.phase = "listening";
                    events.push(vad_json("start", t_ms));
                    events.push(dot_json("listening", self.level));
                }
                Some("end") => {
                    if self.in_speech {
                        self.utterance.extend_from_slice(&piece);
                    }
                    self.in_speech = false;
                    events.push(vad_json("end", t_ms));
                    let heard_pcm = std::mem::take(&mut self.utterance);
                    self.speech = buf;
                    if heard_pcm.is_empty() {
                        self.phase = "idle";
                        events.push(dot_json("idle", self.level));
                        return TurnProduct::events(events);
                    }
                    let heard = match self.recognise(&heard_pcm) {
                        Ok(text) => text,
                        Err(code) => {
                            let mut fail = self.fail(&code);
                            events.append(&mut fail.events);
                            fail.events = events;
                            return fail;
                        }
                    };
                    return self.hear(&heard, t_ms, events);
                }
                _ => {
                    if self.in_speech {
                        self.utterance.extend_from_slice(&piece);
                        if self.utterance.len() > UTTERANCE_CAP {
                            let drop_n = self.utterance.len() - UTTERANCE_CAP;
                            self.utterance.drain(..drop_n);
                        }
                    }
                }
            }
        }
        self.speech = buf;
        TurnProduct::events(events)
    }

    fn commit_pcm(&mut self, pcm: &[i16], rate: u32, t_ms: u64) -> TurnProduct {
        if !self.vad_ready {
            return self.fail("vad-unready");
        }
        let samples = if rate == 16_000 {
            pcm.to_vec()
        } else if rate == 24_000 {
            downsample_24_to_16(pcm)
        } else {
            return self.fail("bad-rate");
        };
        self.speech.clear();
        let mut preface = Vec::new();
        let mut saw_speech = rms(&samples) >= SPEECH_RMS;
        let mut off = 0;
        while off + FRAME <= samples.len() {
            let frame = &samples[off..off + FRAME];
            off += FRAME;
            if let Some(kind) = self.push_frame(frame) {
                preface.push(vad_json(kind, t_ms));
                if kind == "start" {
                    saw_speech = true;
                    self.phase = "listening";
                    preface.push(dot_json("listening", self.level));
                }
            }
        }
        self.speech.clear();
        if !saw_speech {
            self.phase = "idle";
            preface.push(dot_json("idle", self.level));
            return TurnProduct::events(preface);
        }
        let heard = match self.recognise(&samples) {
            Ok(text) => text,
            Err(code) => return self.fail(&code),
        };
        self.hear(&heard, t_ms, preface)
    }

    fn push_text(&mut self, text: &str, t_ms: u64) -> TurnProduct {
        let text = text.trim();
        if text.is_empty() {
            return self.fail("empty-text");
        }
        self.phase = "thinking";
        self.hear(text, t_ms, vec![dot_json("thinking", self.level)])
    }

    fn stop(&mut self) {
        self.speech.clear();
        self.utterance.clear();
        self.in_speech = false;
        self.phase = "idle";
        if let Front::Energy(vad) = &mut self.front {
            vad.reset_turn();
        }
        if let Front::Child(child) = &mut self.front {
            child.reset();
        }
    }

    fn tick_mood(&mut self) -> (crate::reply::Pad, &'static str) {
        match self.mind.tick() {
            Some(pad) => (pad.finite_or_resting(), "brain"),
            None => (crate::reply::Pad::RESTING, "resting"),
        }
    }

    fn phase(&self) -> &'static str {
        self.phase
    }

    fn level(&self) -> f32 {
        self.level
    }

    fn info(&self) -> PipeInfo {
        PipeInfo {
            ear: self.ear_name,
            mouth: self.mouth.name(),
            mind: "horbm".into(),
            vad_backend: self.vad_backend,
            vad_ready: self.vad_ready,
        }
    }
}

impl LivePipeline {
    fn recognise(&mut self, pcm: &[i16]) -> Result<String, String> {
        match &self.ear {
            Ear::Missing => Err("asr-unready".into()),
            #[cfg(test)]
            Ear::Script(line) => {
                let line = line.trim().to_string();
                if line.is_empty() {
                    Err("asr-empty".into())
                } else {
                    Ok(line)
                }
            }
            Ear::Vosk(tx) => {
                let (back_tx, back_rx) = mpsc::channel();
                tx.send(Job {
                    pcm: pcm.to_vec(),
                    back: back_tx,
                })
                .map_err(|_| "asr-unready".to_string())?;
                back_rx
                    .recv_timeout(Duration::from_secs(30))
                    .unwrap_or(Err("asr-unready".into()))
            }
        }
    }
}

fn open_vad() -> (Front, &'static str, bool) {
    let require = std::env::var("GALION_VAD_REQUIRE").ok().as_deref() == Some("silero");
    if let Ok(bin) = std::env::var("GALION_VAD_BIN") {
        let path = PathBuf::from(bin.trim());
        if path.is_file() {
            if let Ok(child) = ChildVad::spawn(&path) {
                let backend = child.backend;
                let ready = !require || backend == "silero";
                return (Front::Child(child), backend, ready);
            }
        }
    }
    let ready = !require;
    (Front::Energy(EnergyVad::new(HANG_MS)), "energy", ready)
}

fn open_ear() -> (Ear, &'static str, Option<JoinHandle<()>>) {
    if galion_sam_asr::find_vosk_model().is_err() || galion_sam_asr::find_vosk_lib().is_none() {
        return (Ear::Missing, "missing", None);
    }
    let (tx, rx) = mpsc::channel::<Job>();
    let worker = std::thread::spawn(move || {
        let opened = open_engine(&EngineOpts::backend("vosk")).ok();
        let mut engine = opened.map(|(engine, _, _)| engine);
        while let Ok(job) = rx.recv() {
            let result = match engine.as_mut() {
                None => Err("asr-unready".into()),
                Some(engine) => transcribe(engine.as_mut(), &job.pcm),
            };
            let _ = job.back.send(result);
        }
    });
    (Ear::Vosk(tx), "vosk", Some(worker))
}

fn transcribe(engine: &mut dyn Asr, pcm: &[i16]) -> Result<String, String> {
    engine.reset().map_err(|_| "asr-unready".to_string())?;
    engine
        .push_pcm(pcm)
        .map_err(|_| "asr-unready".to_string())?;
    let events = engine.finish().map_err(|_| "asr-unready".to_string())?;
    let text = final_text(&events);
    let text = text.trim();
    if text.is_empty() {
        Err("asr-empty".into())
    } else {
        Ok(text.to_string())
    }
}

fn open_mouth() -> Box<dyn Mouth> {
    match PiperTts::open_resolved() {
        Ok(tts) => Box::new(PiperMouth { tts }),
        Err(_) => Box::new(MissingMouth),
    }
}

struct ChildVad {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
    backend: &'static str,
}

impl ChildVad {
    fn spawn(bin: &Path) -> Result<Self, ()> {
        let mut cmd = if bin.extension().and_then(|s| s.to_str()) == Some("py") {
            let mut cmd = Command::new("python3");
            cmd.arg(bin);
            cmd
        } else {
            Command::new(bin)
        };
        let hang = std::env::var("GALION_VAD_HANG_MS").unwrap_or_else(|_| "640".into());
        let commit = std::env::var("GALION_VAD_COMMIT_MS").unwrap_or_else(|_| "640".into());
        cmd.env("GALION_VAD_HANG_MS", hang)
            .env("GALION_VAD_COMMIT_MS", commit)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = cmd.spawn().map_err(|_| ())?;
        let stdin = child.stdin.take().ok_or(())?;
        let stdout = child.stdout.take().ok_or(())?;
        let mut stdout = BufReader::new(stdout);
        let mut line = String::new();
        stdout.read_line(&mut line).map_err(|_| ())?;
        let backend = parse_ready(&line).ok_or(())?;
        Ok(Self {
            child,
            stdin,
            stdout,
            backend,
        })
    }

    fn push(&mut self, frame: &[i16]) -> Option<&'static str> {
        let hex = encode_frame(frame);
        if writeln!(self.stdin, "frame {hex}").is_err() || self.stdin.flush().is_err() {
            return None;
        }
        let mut line = String::new();
        if self.stdout.read_line(&mut line).is_err() {
            return None;
        }
        parse_score(&line).map(|score| score.event)
    }

    fn reset(&mut self) {
        let _ = writeln!(self.stdin, "reset");
        let _ = self.stdin.flush();
        let mut line = String::new();
        let _ = self.stdout.read_line(&mut line);
    }
}

impl Drop for ChildVad {
    fn drop(&mut self) {
        let _ = writeln!(self.stdin, "quit");
        let _ = self.stdin.flush();
        let _ = self.child.kill();
    }
}

fn encode_frame(frame: &[i16]) -> String {
    let mut out = String::with_capacity(frame.len() * 4);
    for sample in frame {
        for byte in sample.to_le_bytes() {
            out.push_str(&format!("{byte:02x}"));
        }
    }
    out
}

fn parse_ready(line: &str) -> Option<&'static str> {
    let rest = line.trim().strip_prefix("ready backend=")?;
    match rest.split_whitespace().next()? {
        "silero" => Some("silero"),
        "energy" => Some("energy"),
        _ => None,
    }
}

struct Score {
    event: &'static str,
}

fn parse_score(line: &str) -> Option<Score> {
    let line = line.trim();
    if !line.starts_with("score ") {
        return None;
    }
    let mut event = None;
    for part in line.split_whitespace().skip(1) {
        if let Some(value) = part.strip_prefix("event=") {
            event = Some(match value {
                "start" => "start",
                "end" => "end",
                _ => "none",
            });
        }
    }
    Some(Score { event: event? })
}

fn frame_16k(frame: &[i16], rate: u32) -> Vec<i16> {
    if rate == 24_000 {
        downsample_24_to_16(frame)
    } else {
        frame.to_vec()
    }
}

fn downsample_24_to_16(pcm: &[i16]) -> Vec<i16> {
    if pcm.is_empty() {
        return Vec::new();
    }
    let out_len = pcm.len() * 2 / 3;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let pos = i as f64 * 1.5;
        let i0 = pos.floor() as usize;
        let i1 = (i0 + 1).min(pcm.len() - 1);
        let frac = pos - i0 as f64;
        let s = f64::from(pcm[i0]) * (1.0 - frac) + f64::from(pcm[i1]) * frac;
        out.push(s.round().clamp(-32768.0, 32767.0) as i16);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn ready_and_score_lines() {
        assert_eq!(parse_ready("ready backend=silero\n"), Some("silero"));
        assert_eq!(parse_ready("ready backend=energy\n"), Some("energy"));
        assert!(parse_ready("ready backend=other").is_none());
        assert_eq!(
            parse_score("score prob=0.800 speaking=1 event=start")
                .unwrap()
                .event,
            "start"
        );
        assert_eq!(
            parse_score("score prob=0.100 speaking=0 event=end")
                .unwrap()
                .event,
            "end"
        );
    }

    #[test]
    fn text_miss_is_i_dont_know_and_the_mouth_is_called() {
        let heard = Arc::new(Mutex::new(Vec::new()));
        let mut pipe = test_pipe(heard.clone(), Ear::Script("unused".into()), "script");
        let out = pipe.push_text("who won the match", 0);
        assert!(out.events.iter().all(|e| !e.contains("brain-unlinked")));
        assert_eq!(out.reply, "I don't know.");
        assert!(!out.pcm.is_empty());
        assert_eq!(heard.lock().unwrap().as_slice(), ["I don't know."]);
        let kettle = pipe.push_text("where is the kettle", 1);
        assert_eq!(kettle.reply, "The kettle is on the stove.");
        assert!(kettle.sources_ok());
    }

    #[test]
    fn mic_commit_runs_vad_then_the_card() {
        let heard = Arc::new(Mutex::new(Vec::new()));
        let mut pipe = test_pipe(
            heard.clone(),
            Ear::Script("where is the kettle".into()),
            "script",
        );
        assert!(pipe.vad_ready);
        assert_eq!(pipe.vad_backend, "energy");
        let pcm = vec![8_000i16; FRAME * 8];
        let out = pipe.commit_pcm(&pcm, 16_000, 0);
        assert!(out.events.iter().any(|e| e.contains("\"type\":\"vad\"")));
        assert_eq!(out.reply, "The kettle is on the stove.");
        assert!(!out.pcm.is_empty());
        assert!(out.events.iter().all(|e| !e.contains("brain-unlinked")));
    }

    #[test]
    fn streaming_mic_waits_640_ms_then_speaks() {
        let heard = Arc::new(Mutex::new(Vec::new()));
        let mut pipe = test_pipe(
            heard.clone(),
            Ear::Script("where is the kettle".into()),
            "script",
        );
        let loud = vec![8_000i16; FRAME];
        let quiet = vec![0i16; FRAME];
        for _ in 0..4 {
            let out = pipe.push_mic(&loud, 16_000, 0);
            assert!(out.pcm.is_empty());
        }
        for _ in 0..31 {
            let mid = pipe.push_mic(&quiet, 16_000, 0);
            assert!(mid.pcm.is_empty(), "{}", mid.events.join("\n"));
            assert!(mid.events.iter().all(|e| !e.contains("\"event\":\"end\"")));
        }
        let end = pipe.push_mic(&quiet, 16_000, 0);
        assert!(end.events.iter().any(|e| e.contains("\"event\":\"end\"")));
        assert_eq!(end.reply, "The kettle is on the stove.");
        assert!(!end.pcm.is_empty());
        assert!(end.events.iter().all(|e| !e.contains("brain-unlinked")));
        assert_eq!(
            heard.lock().unwrap().as_slice(),
            ["The kettle is on the stove."]
        );
    }

    #[test]
    fn missing_vosk_does_not_invent_a_line() {
        let heard = Arc::new(Mutex::new(Vec::new()));
        let mut pipe = test_pipe(heard.clone(), Ear::Missing, "missing");
        let pcm = vec![8_000i16; FRAME * 8];
        let out = pipe.commit_pcm(&pcm, 16_000, 0);
        assert!(out.pcm.is_empty());
        assert!(out.events.iter().any(|e| e.contains("asr-unready")));
        assert!(heard.lock().unwrap().is_empty());
    }

    #[test]
    fn vad_child_reports_silero_ready() {
        let path = write_ear_script();
        let child = ChildVad::spawn(&path).expect("ear");
        assert_eq!(child.backend, "silero");
        let mut child = child;
        let event = child.push(&[1_000i16; FRAME]);
        assert_eq!(event, Some("start"));
        drop(child);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn windows_script_stays_on_the_g_drive() {
        let text = include_str!("../../../scripts/windows/Start-GalionOne.ps1");
        for needle in [
            r"G:\galion\work\galion-one-voice",
            r"G:\galion\ci\toolchains\cargo",
            r"G:\galion\ci\toolchains\rustup",
            r"G:\galion\work\target-voice",
            r"G:\galion\tmp\win",
            r"G:\galion\work\logs",
            "CARGO_HOME",
            "RUSTUP_HOME",
            "CARGO_TARGET_DIR",
            "HF_HOME",
            "PIP_CACHE_DIR",
            "NPM_CONFIG_CACHE",
            "127.0.0.1:7881",
            "silero_vad.onnx",
            "vosk-model-small-en-us-0.15",
            "en_US-ljspeech-medium",
            r"G:\galion\models\asr\vosk-model-small-en-us-0.15",
            r"G:\galion\models\asr\vosk-win64-0.3.45",
            r"voice-stack\vad\models\silero_vad.onnx",
            r"voice-stack\tts-piper-stream\var",
            r"G:\galion\toolchains\onnxruntime\onnxruntime-win-x64-cpu-1.31.1",
            r"G:\galion\tmp\cache\ort.pyke.io",
            "ORT_DYLIB_PATH",
            "ORT_CACHE_DIR",
            "GALION_VAD_HANG_MS",
            "640",
        ] {
            assert!(text.contains(needle), "missing {needle}");
        }
        assert!(!text.contains("%USERPROFILE%"));
        assert!(!text.contains("%APPDATA%"));
        assert!(!text.contains("%LOCALAPPDATA%"));
        assert!(!text.contains("%TEMP%"));
        assert!(!text.contains("C:\\Users"));
        assert!(!text.to_ascii_lowercase().contains("ollama"));
        assert!(!text.to_ascii_lowercase().contains("openai"));
        let cmd = include_str!("../../../scripts/windows/START-SAMANTHA-7881.cmd");
        for needle in [
            r"G:\galion\models\asr\vosk-model-small-en-us-0.15",
            r"G:\galion\models\asr\vosk-win64-0.3.45",
            r"voice-stack\vad\models\silero_vad.onnx",
            r"voice-stack\tts-piper-stream\var",
            r"onnxruntime-win-x64-cpu-1.31.1",
            r"G:\galion\tmp\cache\ort.pyke.io",
            "ORT_DYLIB_PATH",
            "GALION_VAD_HANG_MS=640",
            "127.0.0.1:7881",
            "I don't know.",
        ] {
            assert!(cmd.contains(needle), "cmd missing {needle}");
        }
        assert!(!cmd.contains("%USERPROFILE%"));
        assert!(!cmd.contains("%APPDATA%"));
        assert!(!cmd.contains("%LOCALAPPDATA%"));
        assert!(!cmd.contains("%TEMP%"));
        assert!(!cmd.contains("C:\\Users"));
        assert!(!cmd.to_ascii_lowercase().contains("ollama"));
        assert!(!cmd.to_ascii_lowercase().contains("openai"));
    }

    fn test_pipe(heard: Arc<Mutex<Vec<String>>>, ear: Ear, ear_name: &'static str) -> LivePipeline {
        LivePipeline::for_test(
            Front::Energy(EnergyVad::new(HANG_MS)),
            "energy",
            true,
            ear,
            ear_name,
            HorbmMind::open(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("knowledge")),
            Box::new(RecMouth { heard }),
        )
    }

    fn write_ear_script() -> PathBuf {
        let path = std::env::temp_dir().join(format!("galion-vad-ear-{}.py", std::process::id()));
        let mut file = std::fs::File::create(&path).unwrap();
        write!(
            file,
            "#!/usr/bin/env python3\nimport sys\nprint('ready backend=silero', flush=True)\n\
for line in sys.stdin:\n    line=line.strip()\n    \
if line.startswith('frame'):\n        print('score prob=0.800 speaking=1 event=start', flush=True)\n    \
elif line=='quit':\n        break\n"
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = file.metadata().unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&path, perms).unwrap();
        }
        path
    }

    impl TurnProduct {
        fn sources_ok(&self) -> bool {
            self.events.iter().any(|e| e.contains("card:kettle"))
        }
    }
}
