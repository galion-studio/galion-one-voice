// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Piper mouth for the duplex loop. Glue around the warm worker in
//! `galion-ml-voice`. This module does not change that worker.
//!
//! One sentence is one JSON line. The first sentence is written as soon as
//! [`PiperMouth::start`] runs, so its audio is not held for the rest of the
//! line. Each finished sentence is shaped with the moment packet captured
//! when that sentence was sent ([`horbm_voice::shape_sentence`]), resampled
//! to the 16 kHz duplex clock, and sliced into 80 ms frames.
//!
//! The worker reads `length_scale`, `noise_scale`, and `noise_w` once, at
//! spawn. Those stay at the neutral scales. Pace, pitch, energy, pause, and
//! breath are applied to the sentence buffer. The latest noise scales stay
//! on [`PiperMouth::controls`] for a later spawn. Barge-in drops the queued
//! frames and discards the rest of the in-flight sentence. It does not kill
//! the process.
//!
//! The pipe wait inside the worker is Unix (`poll`). On other targets
//! [`PiperMouth::open`] returns an error and does not spawn. CI and the
//! runtime here are Linux. Paths come from the environment or the cache.
//! Nothing in this file is a hard-coded `.exe` or drive letter.
//!
//! The only voice this mouth will open is the open stock pack
//! `en_US-ljspeech`. Samantha, Aria, and any clone pack are refused.
//! The pack is not downloaded here.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use galion_emotion_core::{Breath, MomentState};
use galion_ml_voice::speech::piper_worker::{PiperProsody, PiperWorker};
use horbm::Pad;
use horbm_voice::{
    samantha_baseline, shape_sentence, AudioFrame, MomentVoice, Mouth, MouthPull, SpokenLine,
    VoiceControls, FRAME_SAMPLES, SAMPLE_RATE,
};

/// Open stock pack this mouth will load. Not a persona and not a clone.
pub const STOCK_VOICE: &str = "en_US-ljspeech-medium";

const BENCH_LINES: &[&str] = &[
    "The kettle is on the hob.",
    "I can hear you.",
    "One moment please.",
    "The door is open.",
    "Say that again.",
    "I am still here.",
    "Take your time.",
    "That is fine.",
    "Go ahead.",
    "I will wait.",
];

const BARGE_LINE: &str =
    "The kettle is on the hob. I will keep talking until you interrupt this sentence now.";

/// Where the warm process and the stock pack are.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PiperPaths {
    pub runtime: PathBuf,
    pub model: PathBuf,
    pub config: PathBuf,
    pub voice: String,
}

/// Wall-clock measurements of the warm CPU mouth. No samples.
#[derive(Clone, Debug, PartialEq)]
pub struct PiperStats {
    pub voice: String,
    pub ttfa_p50_ms: u32,
    pub ttfa_p95_ms: u32,
    pub barge_to_silence_ms: u32,
    pub rtf: f32,
    pub runs: u32,
    pub cpu: bool,
}

impl PiperStats {
    /// One JSON object. `cpu` is true because this mouth only starts the CPU worker.
    #[must_use]
    pub fn json(&self) -> String {
        format!(
            "{{\"event\":\"piper\",\"clock\":\"wall\",\"cpu\":{},\"voice\":\"{}\",\"ttfa_p50_ms\":{},\"ttfa_p95_ms\":{},\"barge_to_silence_ms\":{},\"rtf\":{},\"runs\":{},\"skipped\":null}}",
            if self.cpu { "true" } else { "false" },
            json_escape(&self.voice),
            self.ttfa_p50_ms,
            self.ttfa_p95_ms,
            self.barge_to_silence_ms,
            json_f32(self.rtf),
            self.runs,
        )
    }

    #[must_use]
    pub fn text(&self) -> String {
        format!(
            "duplex s34 piper cpu={} voice={} ttfa_p50_ms={} ttfa_p95_ms={} barge_to_silence_ms={} rtf={} runs={}",
            if self.cpu { 1 } else { 0 },
            self.voice,
            self.ttfa_p50_ms,
            self.ttfa_p95_ms,
            self.barge_to_silence_ms,
            json_f32(self.rtf),
            self.runs,
        )
    }
}

/// JSON object used when the feature is compiled and the stock pack is absent.
#[must_use]
pub fn piper_skipped_json() -> String {
    "{\"event\":\"piper\",\"clock\":\"wall\",\"cpu\":true,\"voice\":null,\"ttfa_p50_ms\":null,\"ttfa_p95_ms\":null,\"barge_to_silence_ms\":null,\"rtf\":null,\"runs\":0,\"skipped\":\"stock voice absent\"}".to_string()
}

/// Cache directory for a stock pack fetched by the tests. Not a repo path.
#[must_use]
pub fn piper_cache_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("HORBM_PIPER_CACHE") {
        return PathBuf::from(dir);
    }
    if let Some(dir) = std::env::var_os("XDG_CACHE_HOME") {
        return PathBuf::from(dir).join("horbm-piper");
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join(".cache").join("horbm-piper");
    }
    std::env::temp_dir().join("horbm-piper")
}

/// A stock pack already on disk. Does not download and does not spawn.
#[must_use]
pub fn discover_piper() -> Option<PiperPaths> {
    let model = std::env::var_os("HORBM_PIPER_MODEL")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("GALION_TTS_WEIGHTS").map(PathBuf::from))
        .or_else(|| {
            let cached = piper_cache_dir()
                .join("tts")
                .join(format!("{STOCK_VOICE}.onnx"));
            cached.is_file().then_some(cached)
        })
        .or_else(|| {
            let beside = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../../weights/tts")
                .join(format!("{STOCK_VOICE}.onnx"));
            beside.is_file().then_some(beside)
        })
        .or_else(|| {
            let cwd = PathBuf::from("weights/tts").join(format!("{STOCK_VOICE}.onnx"));
            cwd.is_file().then_some(cwd)
        })?;
    if !is_stock_model(&model) {
        return None;
    }
    let config = config_beside(&model);
    if !config.is_file() {
        return None;
    }
    let runtime = std::env::var_os("HORBM_PIPER_RUNTIME")
        .map(PathBuf::from)
        .filter(|path| path.is_file())
        .or_else(|| {
            std::env::var_os("GALION_A21_RUNTIME")
                .map(PathBuf::from)
                .filter(|path| path.is_file())
        })
        .or_else(|| first_runtime(&piper_cache_dir().join("runtime").join("piper")))
        .or_else(|| {
            first_runtime(
                &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../weights/runtime/piper"),
            )
        })
        .or_else(|| first_runtime(Path::new("weights/runtime/piper")))?;
    let voice = model
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| STOCK_VOICE.to_string());
    Some(PiperPaths {
        runtime,
        model,
        config,
        voice,
    })
}

/// Warm the worker, then measure ten sentences and one barge-in.
///
/// Time to first audio is the wall time from [`PiperMouth::start`] to the
/// first audible 80 ms frame, after one discarded warmup. The real-time
/// factor is synthesis wall time divided by audio duration, on this CPU
/// process. Barge-in time is [`PiperMouth::cancel`] until the mouth is silent.
pub fn measure_piper(paths: &PiperPaths) -> Result<PiperStats, String> {
    let mut mouth = PiperMouth::open(paths)?;
    mouth.prepare()?;
    let controls = rest_controls();
    mouth.feel(controls);
    let mut ttfa = Vec::with_capacity(BENCH_LINES.len());
    let mut wall_ns = 0u128;
    let mut samples = 0usize;
    for line in BENCH_LINES {
        let started = Instant::now();
        mouth.start(&bench_line(line));
        let mut saw = false;
        let deadline = Instant::now() + Duration::from_secs(45);
        loop {
            if Instant::now() > deadline {
                return Err(timeout_message(&mouth));
            }
            match mouth.pull() {
                MouthPull::Frame(frame) => {
                    if !saw && audible(&frame.samples) {
                        ttfa.push(millis(started.elapsed()));
                        saw = true;
                    }
                    samples = samples.saturating_add(frame.samples.len());
                }
                MouthPull::Idle => std::thread::yield_now(),
                MouthPull::Done => break,
            }
        }
        wall_ns = wall_ns.saturating_add(started.elapsed().as_nanos());
        if !saw {
            return Err("piper sentence produced no audio".into());
        }
        if let Some(err) = mouth.last_error() {
            return Err(err.to_string());
        }
    }
    mouth.feel(controls);
    mouth.start(&bench_line(BARGE_LINE));
    let deadline = Instant::now() + Duration::from_secs(45);
    let mut heard = false;
    loop {
        if Instant::now() > deadline {
            return Err(timeout_message(&mouth));
        }
        match mouth.pull() {
            MouthPull::Frame(frame) if audible(&frame.samples) => {
                heard = true;
                break;
            }
            MouthPull::Frame(_) | MouthPull::Idle => std::thread::yield_now(),
            MouthPull::Done => break,
        }
    }
    if !heard {
        return Err("barge setup produced no audio".into());
    }
    let barge_at = Instant::now();
    mouth.cancel();
    let mut leaked = false;
    for _ in 0..4 {
        if let MouthPull::Frame(frame) = mouth.pull() {
            if audible(&frame.samples) {
                leaked = true;
            }
        }
    }
    let barge_ms = millis(barge_at.elapsed());
    if leaked {
        return Err("barge-in still had audible samples".into());
    }
    if mouth.speaking() {
        return Err("barge-in left the mouth speaking".into());
    }
    let drain_deadline = Instant::now() + Duration::from_secs(45);
    while mouth.busy() {
        if Instant::now() > drain_deadline {
            return Err("piper did not finish the cancelled sentence".into());
        }
        mouth.feel(controls);
        std::thread::sleep(Duration::from_millis(5));
    }
    let audio_ns = (samples as u128).saturating_mul(1_000_000_000) / u128::from(SAMPLE_RATE);
    if audio_ns == 0 {
        return Err("piper measure produced no samples".into());
    }
    let rtf = (wall_ns as f64 / audio_ns as f64) as f32;
    if !rtf.is_finite() || rtf <= 0.0 {
        return Err("piper real-time factor was not a positive number".into());
    }
    let mut sorted = ttfa;
    let ttfa_p50_ms = percentile(&mut sorted, 50);
    let ttfa_p95_ms = percentile(&mut sorted, 95);
    let runs = u32::try_from(sorted.len()).unwrap_or(u32::MAX);
    Ok(PiperStats {
        voice: paths.voice.clone(),
        ttfa_p50_ms,
        ttfa_p95_ms,
        barge_to_silence_ms: barge_ms,
        rtf,
        runs,
        cpu: true,
    })
}

struct Queued {
    samples: Vec<i16>,
    first: bool,
}

/// Sentence stream in front of one warm [`PiperWorker`].
pub struct PiperMouth {
    worker: Option<PiperWorker>,
    pid: u32,
    native_rate: u32,
    spawn_length: String,
    spawn_noise: String,
    spawn_noise_w: String,
    queue: VecDeque<Queued>,
    carry: Vec<i16>,
    last: Vec<i16>,
    sentences: VecDeque<String>,
    controls: VoiceControls,
    sentence_controls: VoiceControls,
    inflight: bool,
    discard: bool,
    marked_first: bool,
    sentences_started: u32,
    error: Option<String>,
}

impl PiperMouth {
    /// Spawn the worker. Neutral scales, so a later moment does not respawn it.
    pub fn open(paths: &PiperPaths) -> Result<Self, String> {
        open_worker(paths)
    }

    /// One short line, discarded, so the next sentence is not the cold start.
    pub fn prepare(&mut self) -> Result<(), String> {
        if self.inflight {
            return Err("piper is already speaking".into());
        }
        let worker = self.worker.as_mut().ok_or("piper worker is closed")?;
        worker.synthesize("Okay.").map_err(|err| err.to_string())?;
        Ok(())
    }

    #[must_use]
    pub fn last(&self) -> &[i16] {
        &self.last
    }

    #[must_use]
    pub fn last_error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Packet from the latest [`Mouth::feel`]. Noise on this packet is not
    /// the noise the warm process was spawned with.
    #[must_use]
    pub fn controls(&self) -> VoiceControls {
        self.controls
    }

    /// Scales passed on the command line at spawn. They do not move.
    #[must_use]
    pub fn spawn_noise(&self) -> &str {
        &self.spawn_noise
    }

    #[must_use]
    pub fn spawn_length(&self) -> &str {
        &self.spawn_length
    }

    #[must_use]
    pub fn spawn_noise_w(&self) -> &str {
        &self.spawn_noise_w
    }

    #[must_use]
    pub fn sentences_started(&self) -> u32 {
        self.sentences_started
    }

    /// True while a sentence is still in the worker, including one being discarded.
    #[must_use]
    pub fn busy(&self) -> bool {
        self.inflight
    }

    #[must_use]
    pub fn pid(&self) -> u32 {
        self.pid
    }

    pub fn process_alive(&mut self) -> bool {
        self.worker.as_mut().is_some_and(PiperWorker::is_alive)
    }

    fn pump(&mut self) -> Result<(), String> {
        if self.worker.is_none() {
            return Ok(());
        }
        if self.inflight {
            self.poll_some()?;
            if self.utterance_ready()? {
                let pcm = self.take_pcm()?;
                let discard = self.discard;
                self.inflight = false;
                self.discard = false;
                if !discard {
                    self.enqueue_sentence(&pcm);
                }
            }
        }
        if !self.inflight {
            self.kick()?;
        }
        Ok(())
    }

    fn poll_some(&mut self) -> Result<(), String> {
        let worker = self.worker.as_mut().ok_or("piper worker is closed")?;
        for _ in 0..8 {
            worker.poll_pipes().map_err(|err| err.to_string())?;
        }
        Ok(())
    }

    fn utterance_ready(&mut self) -> Result<bool, String> {
        let done = self
            .worker
            .as_ref()
            .ok_or("piper worker is closed")?
            .utterance_done();
        if done {
            return Ok(true);
        }
        let alive = self
            .worker
            .as_mut()
            .ok_or("piper worker is closed")?
            .is_alive();
        if alive {
            return Ok(false);
        }
        self.poll_some()?;
        if self
            .worker
            .as_ref()
            .ok_or("piper worker is closed")?
            .utterance_done()
        {
            Ok(true)
        } else {
            Err("piper exited before the sentence finished".into())
        }
    }

    fn take_pcm(&mut self) -> Result<Vec<i16>, String> {
        let worker = self.worker.as_mut().ok_or("piper worker is closed")?;
        let mut last = worker.copy_i16().len();
        for _ in 0..32 {
            worker.poll_pipes().map_err(|err| err.to_string())?;
            let now = worker.copy_i16().len();
            if now == last {
                break;
            }
            last = now;
        }
        Ok(worker.copy_i16())
    }

    fn kick(&mut self) -> Result<(), String> {
        if self.inflight || self.discard {
            return Ok(());
        }
        let Some(sentence) = self.sentences.pop_front() else {
            return Ok(());
        };
        self.sentence_controls = self.controls;
        self.sentences_started = self.sentences_started.saturating_add(1);
        let worker = self.worker.as_mut().ok_or("piper worker is closed")?;
        worker
            .write_text(&sentence)
            .map_err(|err| err.to_string())?;
        self.inflight = true;
        Ok(())
    }

    fn enqueue_sentence(&mut self, pcm: &[i16]) {
        if pcm.is_empty() {
            return;
        }
        let shaped = shape_sentence(pcm, self.native_rate, self.sentence_controls);
        let at_clock = resample(&shaped, self.native_rate, SAMPLE_RATE);
        self.push_samples(&at_clock);
        self.finish_carry();
    }

    fn push_samples(&mut self, pcm: &[i16]) {
        self.carry.extend_from_slice(pcm);
        while self.carry.len() >= FRAME_SAMPLES {
            let frame: Vec<i16> = self.carry.drain(..FRAME_SAMPLES).collect();
            self.push_frame(frame);
        }
    }

    fn finish_carry(&mut self) {
        if self.carry.is_empty() {
            return;
        }
        self.carry.resize(FRAME_SAMPLES, 0);
        let frame = std::mem::take(&mut self.carry);
        self.push_frame(frame);
    }

    fn push_frame(&mut self, samples: Vec<i16>) {
        let first = audible(&samples) && !self.marked_first;
        if first {
            self.marked_first = true;
        }
        self.queue.push_back(Queued { samples, first });
    }

    fn fail(&mut self, err: String) {
        self.error = Some(err);
        self.queue.clear();
        self.carry.clear();
        self.sentences.clear();
        self.inflight = false;
        self.discard = false;
        self.last.clear();
    }
}

impl Mouth for PiperMouth {
    fn id(&self) -> &str {
        "piper"
    }

    fn start(&mut self, line: &SpokenLine) {
        self.queue.clear();
        self.carry.clear();
        self.last.clear();
        self.marked_first = false;
        self.error = None;
        self.sentences = sentences(&line.text).into();
        if self.sentences.is_empty() {
            if self.inflight {
                self.discard = true;
            }
            return;
        }
        if self.inflight {
            self.discard = true;
            return;
        }
        if let Err(err) = self.kick() {
            self.fail(err);
        }
    }

    fn pull(&mut self) -> MouthPull {
        if let Err(err) = self.pump() {
            self.fail(err);
            return MouthPull::Done;
        }
        let Some(frame) = self.queue.pop_front() else {
            if self.speaking() {
                return MouthPull::Idle;
            }
            self.last.clear();
            return MouthPull::Done;
        };
        self.last.clone_from(&frame.samples);
        MouthPull::Frame(AudioFrame {
            samples: frame.samples,
            sample_rate: SAMPLE_RATE,
            first: frame.first,
        })
    }

    fn cancel(&mut self) {
        self.queue.clear();
        self.carry.clear();
        self.sentences.clear();
        self.last.clear();
        self.marked_first = false;
        if self.inflight {
            self.discard = true;
        }
    }

    fn speaking(&self) -> bool {
        if self.discard && self.sentences.is_empty() {
            return false;
        }
        !self.queue.is_empty() || self.inflight || !self.sentences.is_empty()
    }

    fn feel(&mut self, controls: VoiceControls) {
        self.controls = controls;
        if let Err(err) = self.pump() {
            self.fail(err);
        }
    }
}

#[cfg(unix)]
fn open_worker(paths: &PiperPaths) -> Result<PiperMouth, String> {
    if !is_stock_model(&paths.model) {
        return Err(format!(
            "refusing {} ; the duplex mouth only opens the stock {} pack",
            paths.model.display(),
            STOCK_VOICE
        ));
    }
    if !paths.runtime.is_file() {
        return Err(format!(
            "piper runtime missing at {}",
            paths.runtime.display()
        ));
    }
    let native_rate = read_sample_rate(&paths.config)?;
    let prosody = neutral_prosody();
    let worker = PiperWorker::spawn(&paths.runtime, &paths.model, &paths.config, &prosody)
        .map_err(|err| err.to_string())?;
    let pid = worker.pid();
    Ok(PiperMouth {
        worker: Some(worker),
        pid,
        native_rate,
        spawn_length: prosody.length_scale,
        spawn_noise: prosody.noise_scale,
        spawn_noise_w: prosody.noise_w,
        queue: VecDeque::new(),
        carry: Vec::new(),
        last: Vec::new(),
        sentences: VecDeque::new(),
        controls: VoiceControls::unsteered(),
        sentence_controls: VoiceControls::unsteered(),
        inflight: false,
        discard: false,
        marked_first: false,
        sentences_started: 0,
        error: None,
    })
}

#[cfg(not(unix))]
fn open_worker(paths: &PiperPaths) -> Result<PiperMouth, String> {
    let _ = paths;
    Err("piper mouth waits on worker pipes with poll, and this build only does that on unix".into())
}

fn neutral_prosody() -> PiperProsody {
    PiperProsody {
        length_scale: "1.00".into(),
        noise_scale: "0.00".into(),
        noise_w: "0.00".into(),
        sentence_silence: "0.00".into(),
    }
}

fn is_stock_model(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let name = name.to_ascii_lowercase();
    if name.contains("samantha")
        || name.contains("aria")
        || name.contains("clone")
        || name.contains("jarvis")
    {
        return false;
    }
    name.starts_with("en_us-ljspeech") && name.ends_with(".onnx") && !name.ends_with(".onnx.json")
}

fn config_beside(model: &Path) -> PathBuf {
    PathBuf::from(format!("{}.json", model.display()))
}

fn first_runtime(dir: &Path) -> Option<PathBuf> {
    let named = dir.join(format!("piper{}", std::env::consts::EXE_SUFFIX));
    if named.is_file() {
        return Some(named);
    }
    let plain = dir.join("piper");
    plain.is_file().then_some(plain)
}

fn read_sample_rate(config: &Path) -> Result<u32, String> {
    let text =
        std::fs::read_to_string(config).map_err(|err| format!("{}: {err}", config.display()))?;
    sample_rate_in(&text).ok_or_else(|| format!("{}: no sample_rate", config.display()))
}

fn sample_rate_in(text: &str) -> Option<u32> {
    let key = "\"sample_rate\"";
    let rest = text.split(key).nth(1)?;
    let rest = rest.trim_start().strip_prefix(':')?.trim_start();
    let digits: String = rest.chars().take_while(|ch| ch.is_ascii_digit()).collect();
    let rate = digits.parse::<u32>().ok()?;
    (rate > 0).then_some(rate)
}

/// Split on `.` `!` `?` when the next character is whitespace or the end.
fn sentences(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut cur = String::new();
    for (index, ch) in chars.iter().enumerate() {
        cur.push(*ch);
        let boundary = matches!(ch, '.' | '!' | '?')
            && chars
                .get(index + 1)
                .map(|next| next.is_whitespace())
                .unwrap_or(true);
        if boundary {
            push_sentence(&mut out, &mut cur);
        }
    }
    push_sentence(&mut out, &mut cur);
    out
}

fn push_sentence(out: &mut Vec<String>, cur: &mut String) {
    let trimmed = cur.trim();
    if !trimmed.is_empty() {
        out.push(trimmed.to_string());
    }
    cur.clear();
}

fn resample(input: &[i16], from: u32, to: u32) -> Vec<i16> {
    if input.is_empty() || from == 0 || to == 0 || from == to {
        return input.to_vec();
    }
    let out_len = ((input.len() as u64) * u64::from(to) / u64::from(from)).max(1) as usize;
    if input.len() == 1 {
        return vec![input[0]; out_len];
    }
    let last = input.len() - 1;
    let mut out = Vec::with_capacity(out_len);
    let denom = (out_len - 1).max(1) as f32;
    for index in 0..out_len {
        let pos = (index as f32) * (last as f32) / denom;
        let at = pos.floor() as usize;
        let frac = pos - at as f32;
        let a = f32::from(input[at.min(last)]);
        let b = f32::from(input[(at + 1).min(last)]);
        out.push(to_i16(a + (b - a) * frac));
    }
    out
}

fn to_i16(x: f32) -> i16 {
    let y = if x.is_finite() { x.round() } else { 0.0 };
    if y >= 32767.0 {
        32767
    } else if y <= -32768.0 {
        -32768
    } else {
        y as i16
    }
}

fn audible(samples: &[i16]) -> bool {
    samples.iter().any(|sample| *sample != 0)
}

fn rest_controls() -> VoiceControls {
    let baseline = samantha_baseline();
    let mut voice = MomentVoice::new(baseline);
    let rest = MomentState::at(baseline.pos);
    voice.tick(rest, Breath::rest());
    voice.tick(rest, Breath::rest());
    voice.applied()
}

fn bench_line(text: &str) -> SpokenLine {
    SpokenLine {
        text: text.to_string(),
        mood: Pad::RESTING,
        rate: 1.0,
        energy: 1.0,
        pitch_st: 0.0,
        pause_ms: 0.0,
    }
}

fn millis(elapsed: Duration) -> u32 {
    u32::try_from(elapsed.as_millis()).unwrap_or(u32::MAX)
}

fn percentile(samples: &mut [u32], pct: usize) -> u32 {
    if samples.is_empty() {
        return 0;
    }
    samples.sort_unstable();
    let rank = (pct * samples.len() + 99) / 100;
    let rank = rank.clamp(1, samples.len());
    samples[rank - 1]
}

fn timeout_message(mouth: &PiperMouth) -> String {
    match mouth.last_error() {
        Some(err) => format!("piper timed out ({err})"),
        None => "piper timed out".into(),
    }
}

fn json_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

fn json_f32(value: f32) -> String {
    if !value.is_finite() {
        return "null".into();
    }
    format!("{value:.4}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sentences_split_on_the_boundary_and_keep_a_decimal() {
        assert_eq!(
            sentences("Hello there. Second line!"),
            vec!["Hello there.".to_string(), "Second line!".to_string()]
        );
        assert_eq!(sentences("3.14 is pi."), vec!["3.14 is pi.".to_string()]);
        assert!(sentences("   ").is_empty());
    }

    #[test]
    fn sample_rate_is_read_from_the_pack_json() {
        let text = r#"{"audio":{"sample_rate":22050,"quality":"medium"}}"#;
        assert_eq!(sample_rate_in(text), Some(22050));
        assert_eq!(sample_rate_in("{}"), None);
    }

    #[test]
    fn a_persona_pack_is_not_the_stock_voice() {
        assert!(is_stock_model(Path::new("en_US-ljspeech-medium.onnx")));
        assert!(!is_stock_model(Path::new("samantha.onnx")));
        assert!(!is_stock_model(Path::new("aria-parked.onnx")));
        assert!(!is_stock_model(Path::new("en_US-clone.onnx")));
    }

    #[test]
    fn resample_keeps_the_length_ratio() {
        let input = vec![0, 1000, 0, -1000];
        let out = resample(&input, 16_000, 16_000);
        assert_eq!(out, input);
        let up = resample(&input, 8_000, 16_000);
        assert_eq!(up.len(), 8);
    }
}
