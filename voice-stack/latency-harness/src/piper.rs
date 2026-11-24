// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Piper mouth. One persistent child, the same `--json-input --output_raw`
//! protocol as `archive/2026-11-24/crates/galion-v2-live/src/piper.rs` and
//! `scripts/eval/piper_stream_ttfa.py`.
//!
//! The binary and the `en_US-ljspeech-medium` pack are located the way
//! `crates/galion-ml-voice/src/speech/weights.rs` locates them:
//! `$GALION_A21_RUNTIME`, else `galion-piper-rt` when it is installed, else
//! `weights/runtime/piper/piper`. `$GALION_PIPER_LEGACY=1` keeps the legacy
//! CLI. The model is `$GALION_TTS_MODEL` or
//! `weights/tts/en_US-ljspeech-medium.onnx`.
//!
//! This voice is LJSpeech via Piper. It is not Samantha. No reference clip
//! is read. `GALION_SAMANTHA_REF` is not consulted.
//!
//! A missing pack is [`crate::HarnessError::Absent`]. The caller writes
//! `measured: false` and does not invent a latency.

use crate::clock::input_samples_for_one_frame;
use crate::fake::{fixture_pcm, EnergyVad, FakeAsr, FakeReply, FrameSink};
use crate::machine::repo_root;
use crate::pipeline::{measure, MeasureIn};
use crate::report::{ColdClocks, Report};
use crate::sha256::sha256_file;
use crate::traits::{Pad, PcmChunk, StageError, TtsStage, PHRASE};
use crate::HarnessError;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Pin from `crates/galion-ml-voice/src/speech/weights.rs` `PINNED`.
pub const PINNED_ONNX: &str = "6f52a751e2349abe7a76735eb09dc1875298c77ea2342ffd2fef79ff81b87f22";
/// Pin for the sibling `.onnx.json`.
pub const PINNED_JSON: &str = "141d612cc0a95ed7efc1ca936b845c2364967f2e9217c5dbfcf69fc4d6c65860";
pub const VOICE: &str = "en_US-ljspeech-medium";
const WARMUP_PHRASE: &str = "Hi.";

#[derive(Clone, Debug)]
pub struct PiperPaths {
    pub bin: PathBuf,
    pub model: PathBuf,
    pub config: PathBuf,
}

#[derive(Clone, Debug)]
pub struct PiperInfo {
    pub runtime: String,
    pub model: String,
    pub sample_rate: u32,
    pub sha256: String,
    pub sha256_pinned: bool,
    pub json_sha256: String,
    pub json_sha256_pinned: bool,
    pub length_scale: String,
    pub prosody: String,
    pub args: Vec<String>,
}

pub fn locate_piper() -> Result<PiperPaths, String> {
    let root = repo_root();
    let model = match std::env::var("GALION_TTS_MODEL") {
        Ok(p) if !p.trim().is_empty() => PathBuf::from(p.trim()),
        _ => root.join("weights/tts").join(format!("{VOICE}.onnx")),
    };
    let config = PathBuf::from(format!("{}.json", model.display()));
    if !model.is_file() || !config.is_file() {
        return Err(format!(
            "A21 Piper pack is not on disk ({}, {}). Run scripts/a21/fetch-a21.sh on Linux x86_64, or set GALION_TTS_MODEL.",
            model.display(),
            config.display()
        ));
    }
    let bin = runtime_path(&root)?;
    if !bin.is_file() {
        return Err(format!(
            "Piper runtime is not on disk ({}). Run scripts/a21/fetch-a21.sh or set GALION_A21_RUNTIME.",
            bin.display()
        ));
    }
    Ok(PiperPaths { bin, model, config })
}

fn runtime_path(root: &Path) -> Result<PathBuf, String> {
    if let Ok(p) = std::env::var("GALION_A21_RUNTIME") {
        let p = p.trim();
        if !p.is_empty() {
            let path = PathBuf::from(p);
            if path.is_file() {
                return Ok(path);
            }
            return Err(format!("GALION_A21_RUNTIME is set but not a file ({p})"));
        }
    }
    let legacy = root.join("weights/runtime/piper/piper");
    let rt = root.join("weights/runtime/piper-rt/galion-piper-rt");
    if legacy_forced() {
        return Ok(legacy);
    }
    if rt.is_file() {
        Ok(rt)
    } else {
        Ok(legacy)
    }
}

fn legacy_forced() -> bool {
    matches!(
        std::env::var("GALION_PIPER_LEGACY")
            .ok()
            .as_deref()
            .map(str::trim),
        Some("1" | "true" | "yes")
    )
}

pub fn parse_sample_rate(json: &str) -> Option<u32> {
    let key = "\"sample_rate\"";
    let i = json.find(key)?;
    let rest = &json[i + key.len()..];
    let mut digits = String::new();
    let mut started = false;
    for c in rest.chars() {
        if c.is_ascii_digit() {
            started = true;
            digits.push(c);
        } else if started {
            break;
        }
    }
    digits.parse().ok()
}

struct PiperTts {
    child: Child,
    stdin: Option<ChildStdin>,
    rx: Receiver<PcmChunk>,
    rtf: Arc<AtomicUsize>,
    last_audio: Arc<Mutex<Instant>>,
    sample_rate: u32,
    /// Chunks stamped before this instant belong to the previous utterance.
    started: Instant,
    rtf_mark: usize,
    info: PiperInfo,
}

impl PiperTts {
    fn open(paths: &PiperPaths) -> Result<(Self, ColdClocks), StageError> {
        let model_sha = sha256_file(&paths.model).map_err(|e| StageError::new("tts", e))?;
        let json_sha = sha256_file(&paths.config).map_err(|e| StageError::new("tts", e))?;
        let config_text = std::fs::read_to_string(&paths.config)
            .map_err(|e| StageError::new("tts", format!("read config: {e}")))?;
        let sample_rate = parse_sample_rate(&config_text)
            .ok_or_else(|| StageError::new("tts", "config has no sample_rate"))?;
        let onnx_name = paths
            .model
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        let pinned_name = onnx_name == format!("{VOICE}.onnx");
        let sha256_pinned = pinned_name && model_sha == PINNED_ONNX;
        let json_sha256_pinned = pinned_name && json_sha == PINNED_JSON;
        if pinned_name && (model_sha != PINNED_ONNX || json_sha != PINNED_JSON) {
            return Err(StageError::new(
                "tts",
                format!("sha256 mismatch for {VOICE} (model {model_sha}, config {json_sha})"),
            ));
        }
        let root = repo_root();
        let args = vec![
            "--model".into(),
            paths.model.display().to_string(),
            "--json-input".into(),
            "--output_raw".into(),
            "--sentence_silence".into(),
            "0".into(),
            "--noise_scale".into(),
            "0".into(),
            "--noise_w".into(),
            "0".into(),
            "--length_scale".into(),
            "1.0".into(),
        ];
        let spawned = Instant::now();
        let mut child = Command::new(&paths.bin)
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| StageError::new("tts", format!("spawn piper: {e}")))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| StageError::new("tts", "piper stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| StageError::new("tts", "piper stdout"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| StageError::new("tts", "piper stderr"))?;
        let (tx, rx) = mpsc::channel();
        let last_audio = Arc::new(Mutex::new(Instant::now()));
        let last_r = last_audio.clone();
        std::thread::spawn(move || read_pcm(stdout, tx, last_r, sample_rate));
        let rtf = Arc::new(AtomicUsize::new(0));
        let rtf_r = rtf.clone();
        std::thread::spawn(move || read_rtf(stderr, rtf_r));
        let info = PiperInfo {
            runtime: rel(&root, &paths.bin),
            model: rel(&root, &paths.model),
            sample_rate,
            sha256: model_sha,
            sha256_pinned,
            json_sha256: json_sha,
            json_sha256_pinned,
            length_scale: "1.0".into(),
            prosody: "fixed-resting. length_scale 1.0, noise_scale 0, noise_w 0. The reply mood is recorded and is not applied to this child.".into(),
            args,
        };
        let mut mouth = Self {
            child,
            stdin: Some(stdin),
            rx,
            rtf,
            last_audio,
            sample_rate,
            started: spawned,
            rtf_mark: 0,
            info,
        };
        let write_at = Instant::now();
        mouth.begin(WARMUP_PHRASE, &Pad::RESTING)?;
        let first = mouth.pull_frame()?;
        mouth.finish()?;
        let cold = ColdClocks {
            spawn_to_first_pcm_us: micros(spawned, first)?,
            warmup_write_to_first_pcm_us: micros(write_at, first)?,
            warmup_phrase: WARMUP_PHRASE.to_string(),
        };
        Ok((mouth, cold))
    }

    fn pull_frame(&mut self) -> Result<Instant, StageError> {
        let need = input_samples_for_one_frame(self.sample_rate);
        let mut got = 0usize;
        let mut first = None;
        loop {
            match self.pull(Duration::from_secs(20))? {
                None => {
                    return Err(StageError::new(
                        "tts",
                        "piper closed before one heartbeat frame",
                    ))
                }
                Some(chunk) => {
                    if first.is_none() {
                        first = Some(chunk.at);
                    }
                    got += chunk.samples.len();
                    if got >= need {
                        return first.ok_or_else(|| StageError::new("tts", "no pcm"));
                    }
                }
            }
        }
    }
}

impl Drop for PiperTts {
    fn drop(&mut self) {
        self.stdin.take();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl TtsStage for PiperTts {
    fn name(&self) -> &str {
        "piper"
    }

    fn begin(&mut self, text: &str, _mood: &Pad) -> Result<(), StageError> {
        let text = sanitize(text)?;
        self.started = Instant::now();
        self.rtf_mark = self.rtf.load(Ordering::SeqCst);
        let line = format!("{{\"text\":{}}}\n", crate::report::json_str(&text));
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| StageError::new("tts", "piper stdin closed"))?;
        stdin
            .write_all(line.as_bytes())
            .and_then(|_| stdin.flush())
            .map_err(|e| StageError::new("tts", format!("piper stdin: {e}")))?;
        Ok(())
    }

    fn pull(&mut self, timeout: Duration) -> Result<Option<PcmChunk>, StageError> {
        let deadline = Instant::now() + timeout;
        loop {
            let now = Instant::now();
            if now >= deadline {
                return Err(StageError::new("tts", "timed out waiting for piper pcm"));
            }
            match self
                .rx
                .recv_timeout(deadline.saturating_duration_since(now))
            {
                Ok(chunk) => {
                    if chunk.at < self.started {
                        continue;
                    }
                    return Ok(Some(chunk));
                }
                Err(RecvTimeoutError::Timeout) => {
                    return Err(StageError::new("tts", "timed out waiting for piper pcm"));
                }
                Err(RecvTimeoutError::Disconnected) => return Ok(None),
            }
        }
    }

    fn finish(&mut self) -> Result<(), StageError> {
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut stable = 0u8;
        loop {
            while self.rx.try_recv().is_ok() {}
            let rtf_ok = self.rtf.load(Ordering::SeqCst) > self.rtf_mark;
            let idle = self
                .last_audio
                .lock()
                .map(|t| t.elapsed() >= Duration::from_millis(40))
                .unwrap_or(false);
            let empty = matches!(
                self.rx.try_recv(),
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected)
            );
            if rtf_ok && idle && empty {
                stable = stable.saturating_add(1);
                if stable >= 2 {
                    return Ok(());
                }
            } else {
                stable = 0;
            }
            if Instant::now() > deadline {
                return Err(StageError::new("tts", "piper did not finish the utterance"));
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

fn read_pcm(
    mut stdout: impl Read,
    tx: mpsc::Sender<PcmChunk>,
    last: Arc<Mutex<Instant>>,
    sample_rate: u32,
) {
    let mut buf = [0u8; 8192];
    let mut carry: Option<u8> = None;
    loop {
        let n = match stdout.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        let at = Instant::now();
        let mut bytes = Vec::with_capacity(n + 1);
        if let Some(c) = carry.take() {
            bytes.push(c);
        }
        bytes.extend_from_slice(&buf[..n]);
        if bytes.len() % 2 == 1 {
            carry = bytes.pop();
        }
        if bytes.is_empty() {
            continue;
        }
        let mut samples = Vec::with_capacity(bytes.len() / 2);
        for ch in bytes.chunks_exact(2) {
            samples.push(i16::from_le_bytes([ch[0], ch[1]]));
        }
        if samples.is_empty() {
            continue;
        }
        if let Ok(mut slot) = last.lock() {
            *slot = at;
        }
        if tx
            .send(PcmChunk {
                at,
                samples,
                sample_rate,
            })
            .is_err()
        {
            break;
        }
    }
}

fn read_rtf(stderr: impl Read, rtf: Arc<AtomicUsize>) {
    let reader = BufReader::new(stderr);
    for line in reader.lines() {
        let Ok(line) = line else { break };
        if line.contains("Real-time factor") {
            rtf.fetch_add(1, Ordering::SeqCst);
        }
    }
}

fn micros(earlier: Instant, later: Instant) -> Result<u64, StageError> {
    later
        .checked_duration_since(earlier)
        .map(|d| u64::try_from(d.as_micros()).unwrap_or(u64::MAX))
        .ok_or_else(|| StageError::new("clock", "piper stamp went backwards"))
}

fn sanitize(text: &str) -> Result<String, StageError> {
    let t = text.trim();
    if t.is_empty() || t.len() > 200 {
        return Err(StageError::new("tts", "piper text length"));
    }
    if !t
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | ',' | '.' | '\'' | '?' | '!'))
    {
        return Err(StageError::new(
            "tts",
            "piper text has a character this harness will not send",
        ));
    }
    Ok(t.to_string())
}

fn rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

pub fn run_piper(reps: usize, warmup: usize) -> Result<Report, HarnessError> {
    let paths = locate_piper().map_err(|reason| HarnessError::Absent { reason })?;
    let (mut mouth, cold) = PiperTts::open(&paths).map_err(|e| {
        if e.message.contains("sha256 mismatch") || e.message.contains("not on disk") {
            HarnessError::Absent { reason: e.message }
        } else {
            HarnessError::Failed(e)
        }
    })?;
    let pinned = mouth.info.sha256_pinned && mouth.info.json_sha256_pinned;
    let info = mouth.info.clone();
    let pcm = fixture_pcm(16_000);
    let mut vad = EnergyVad::default();
    let mut asr = FakeAsr;
    let mut reply = FakeReply::resting();
    let mut sink = FrameSink::default();
    let voice = if pinned {
        Some(VOICE.to_string())
    } else {
        None
    };
    let mut notes = vec![
        "TTS is Piper en_US-ljspeech-medium (LJSpeech, public-domain data, MIT pack) on this CPU. It is not Samantha and not a clone. GALION_SAMANTHA_REF was not read.".to_string(),
        "VAD, ASR, and the reply are the fake stages, the same ones as the fake backend. Those lines are not Vosk and not HORBM.".to_string(),
        "First TTS audio is the first PCM byte from the warm child. Reply ready to first slice is the reply stamp until one 80 ms frame of the first speakable piece has been produced. first_slice_grain is clause or sentence. This phrase is one sentence. First audio out is the wall time when one resampled 24 kHz heartbeat frame is in the sink. The heartbeat play row is derived from those arrivals on the 80 ms grid (origin = first PCM byte), the same rule as scripts/eval/piper_stream_ttfa.py. The rest of the utterance is drained after the clock stops.".to_string(),
        "Prosody is fixed at rest (length_scale 1.0, noise_scale 0, noise_w 0) so the distribution is the CPU, not a noise draw. The reply still carries a Pad. That Pad is not applied to this Piper child.".to_string(),
        format!("Spoken phrase: \"{PHRASE}\". Cores were not pinned."),
    ];
    if !pinned {
        notes.push("The model sha256 did not match the A21 pin. tts_pinned_voice is false.".into());
    }
    let report = measure(
        &mut vad,
        &mut asr,
        &mut reply,
        &mut mouth,
        &mut sink,
        MeasureIn {
            pcm: &pcm,
            pcm_rate: 16_000,
            reps,
            warmup,
            backend: "piper",
            claims_tts_measured: pinned,
            voice,
            cold: Some(cold),
            piper: Some(info),
            notes,
        },
    )?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_rate_parses_the_pack_shape() {
        let json =
            "{\n  \"audio\": {\n    \"sample_rate\": 22050,\n    \"quality\": \"medium\"\n  }\n}\n";
        assert_eq!(parse_sample_rate(json), Some(22_050));
        assert_eq!(parse_sample_rate("{}"), None);
    }

    #[test]
    fn pins_are_64_hex_chars() {
        assert_eq!(PINNED_ONNX.len(), 64);
        assert_eq!(PINNED_JSON.len(), 64);
        assert!(PINNED_ONNX.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(PINNED_JSON.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
