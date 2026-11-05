// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! Warm Piper (A21 pin, out of process, CPU). One persistent child per
//! voice setting; text goes in as one JSON line per clause; raw s16le PCM
//! comes back on stdout and is timestamped by a reader thread the moment
//! the bytes arrive. No stub path: a missing binary is an error.
use crate::now_ms;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};

pub struct PiperPaths { pub bin: String, pub model: String }
impl PiperPaths {
    pub fn from_repo() -> Result<Self, String> {
        let root = crate::repo_root();
        // lat240 wt-14: prefer the in-tree Piper runtime (galion-piper-rt, same Piper
        // ONNX voice, ORT thread count + pre-warm), as galion-ml-voice already does.
        // Measured warm TTFA 29/33 ms vs 43/61 ms for the C++ CLI (n=20, BENCH.lock).
        // GALION_PIPER_LEGACY=1 keeps the C++ piper CLI.
        let rt = root.join("weights/runtime/piper-rt/galion-piper-rt");
        let legacy = std::env::var("GALION_PIPER_LEGACY").map(|v| v == "1").unwrap_or(false);
        let default_bin = if !legacy && rt.is_file() { rt } else { root.join("weights/runtime/piper/piper") };
        let bin = std::env::var("GALION_A21_RUNTIME").unwrap_or_else(|_| default_bin.display().to_string());
        let model = std::env::var("GALION_TTS_MODEL").unwrap_or_else(|_| root.join("weights/tts/en_US-ljspeech-medium.onnx").display().to_string());
        if !std::path::Path::new(&bin).is_file() || !std::path::Path::new(&model).is_file() {
            return Err(format!("A21 Piper pack missing ({bin}, {model}); run scripts/a21/fetch-a21.sh"));
        }
        Ok(Self { bin, model })
    }
}
pub const PIPER_HZ: u32 = 22_050;

/// A chunk of PCM with the wall time (ms since process start) it arrived.
pub struct Chunk { pub t_ms: f64, pub pcm: Vec<i16> }

pub struct Piper {
    child: Child,
    stdin: ChildStdin,
    pub rx: Receiver<Chunk>,
    /// Count of "Real-time factor" lines (one per synthesized sentence).
    pub done: Arc<AtomicUsize>,
    pub spawned_ms: f64,
    pub length_scale: f32,
}

impl Piper {
    pub fn spawn(p: &PiperPaths, length_scale: f32, noise_scale: f32) -> Result<Self, String> {
        let spawned_ms = now_ms();
        let mut child = Command::new(&p.bin)
            .args(["--model", &p.model, "--json-input", "--output_raw", "--sentence_silence", "0",
                   "--length_scale", &format!("{length_scale}"), "--noise_scale", &format!("{noise_scale}"), "--noise_w", "0.6"])
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
            .spawn().map_err(|e| format!("spawn piper: {e}"))?;
        let stdin = child.stdin.take().unwrap();
        let mut out = child.stdout.take().unwrap();
        let err = child.stderr.take().unwrap();
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let mut buf = vec![0u8; 4096];
            let mut carry: Option<u8> = None;
            loop {
                let n = match out.read(&mut buf) { Ok(0) | Err(_) => break, Ok(n) => n };
                let t = now_ms();
                let mut bytes = Vec::with_capacity(n + 1);
                if let Some(c) = carry.take() { bytes.push(c); }
                bytes.extend_from_slice(&buf[..n]);
                if bytes.len() % 2 == 1 { carry = bytes.pop(); }
                if tx.send(Chunk { t_ms: t, pcm: crate::audio::bytes_to_i16(&bytes) }).is_err() { break; }
            }
        });
        let done = Arc::new(AtomicUsize::new(0));
        let d2 = done.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(err).lines().map_while(Result::ok) {
                if line.contains("Real-time factor") { d2.fetch_add(1, Ordering::SeqCst); }
            }
        });
        Ok(Self { child, stdin, rx, done, spawned_ms, length_scale })
    }
    /// Send one clause. Returns the send time (ms).
    pub fn say(&mut self, text: &str) -> Result<f64, String> {
        let line = serde_json::json!({ "text": text }).to_string();
        let t = now_ms();
        self.stdin.write_all(line.as_bytes()).and_then(|_| self.stdin.write_all(b"\n")).and_then(|_| self.stdin.flush())
            .map_err(|e| format!("piper stdin: {e}"))?;
        Ok(t)
    }
    /// Drain everything already queued (used between benchmark runs).
    pub fn drain(&self) { while self.rx.try_recv().is_ok() {} }
    /// Block until the first PCM chunk; returns its arrival time.
    pub fn first_pcm(&self, timeout_ms: u64) -> Option<Chunk> {
        self.rx.recv_timeout(std::time::Duration::from_millis(timeout_ms)).ok()
    }
    /// Collect until `sentences` more RTF lines were seen and the pipe is idle 30 ms.
    pub fn collect_until_done(&self, before: usize, sentences: usize, timeout_ms: u64) -> Vec<Chunk> {
        let mut v = Vec::new();
        let t0 = now_ms();
        loop {
            match self.rx.recv_timeout(std::time::Duration::from_millis(30)) {
                Ok(c) => v.push(c),
                Err(_) => { if self.done.load(Ordering::SeqCst) >= before + sentences { break; } }
            }
            if now_ms() - t0 > timeout_ms as f64 { break; }
        }
        v
    }
}
impl Drop for Piper { fn drop(&mut self) { let _ = self.child.kill(); let _ = self.child.wait(); } }
