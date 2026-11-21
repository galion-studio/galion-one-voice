// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! One Piper process, the same command the A21 worker uses.
//!
//! `crates/galion-ml-voice/src/speech/piper_worker.rs` spawns
//! `piper --model --output_raw` and reads raw s16le until stderr logs
//! `Real-time factor:`. Noise scales are 0 so the round-trip is the
//! deterministic V1 path (`PiperProsody::from_env` when
//! `GALION_A21_NOISE` is unset). This is the LJSpeech pack when that is
//! the file on disk. It is not a Samantha clone and it does not write a wav.

use std::io::{Read, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub struct Spoken {
    pub pcm: Vec<u8>,
    pub first_audio_ms: Option<f64>,
    pub wall_ms: f64,
}

struct Chunk {
    at_ms_from: Instant,
    bytes: Vec<u8>,
}

pub struct Session {
    child: Child,
    stdin: Option<std::process::ChildStdin>,
    chunks: Arc<Mutex<Vec<Chunk>>>,
    stderr: Arc<Mutex<String>>,
    rtf: Arc<AtomicU32>,
    _stdout: JoinHandle<()>,
    _stderr: JoinHandle<()>,
    pub model_load_ms: Option<f64>,
}

impl Session {
    pub fn spawn(runtime: &Path, model: &Path) -> Result<Self, String> {
        let mut cmd = Command::new(runtime);
        cmd.arg("--model")
            .arg(model)
            .arg("--output_raw")
            .arg("--noise_scale")
            .arg("0")
            .arg("--noise_w")
            .arg("0")
            .arg("--length_scale")
            .arg("1.0")
            .arg("--sentence_silence")
            .arg("0")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("piper spawn {}: {e}", runtime.display()))?;
        let stdin = child.stdin.take();
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let (Some(stdin), Some(mut stdout), Some(mut stderr)) = (stdin, stdout, stderr) else {
            let _ = child.kill();
            return Err("piper pipes missing".into());
        };
        let chunks = Arc::new(Mutex::new(Vec::new()));
        let chunks_t = Arc::clone(&chunks);
        let stderr_buf = Arc::new(Mutex::new(String::new()));
        let stderr_t = Arc::clone(&stderr_buf);
        let rtf = Arc::new(AtomicU32::new(0));
        let rtf_t = Arc::clone(&rtf);
        let stdout_thread = thread::spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match stdout.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let mut guard = chunks_t.lock().expect("pcm lock");
                        guard.push(Chunk {
                            at_ms_from: Instant::now(),
                            bytes: buf[..n].to_vec(),
                        });
                    }
                }
            }
        });
        let stderr_thread = thread::spawn(move || {
            let mut buf = [0u8; 1024];
            loop {
                match stderr.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let piece = String::from_utf8_lossy(&buf[..n]);
                        let mut guard = stderr_t.lock().expect("stderr lock");
                        guard.push_str(&piece);
                        let count = guard.matches("Real-time factor").count() as u32;
                        rtf_t.store(count, Ordering::SeqCst);
                    }
                }
            }
        });
        let started = Instant::now();
        let model_load_ms = wait_for(&stderr_buf, "Initialized piper", Duration::from_secs(30))?;
        let load = model_load_ms.map(|_| elapsed_ms(started));
        Ok(Self {
            child,
            stdin: Some(stdin),
            chunks,
            stderr: stderr_buf,
            rtf,
            _stdout: stdout_thread,
            _stderr: stderr_thread,
            model_load_ms: load,
        })
    }

    pub fn say(&mut self, text: &str) -> Result<Spoken, String> {
        let stdin = self.stdin.as_mut().ok_or("piper stdin is closed")?;
        let before = self.rtf.load(Ordering::SeqCst);
        let t0 = Instant::now();
        stdin
            .write_all(text.trim().as_bytes())
            .and_then(|_| stdin.write_all(b"\n"))
            .and_then(|_| stdin.flush())
            .map_err(|e| format!("piper write: {e}"))?;
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if self.rtf.load(Ordering::SeqCst) > before {
                let n = self.chunks.lock().expect("pcm lock").len();
                thread::sleep(Duration::from_millis(40));
                if self.chunks.lock().expect("pcm lock").len() == n {
                    break;
                }
            }
            if Instant::now() > deadline {
                let tail = tail(&self.stderr.lock().expect("stderr lock"));
                return Err(format!(
                    "piper did not log Real-time factor within 30s. {tail}"
                ));
            }
            thread::sleep(Duration::from_millis(5));
        }
        let chunks = self.chunks.lock().expect("pcm lock");
        let mut pcm = Vec::new();
        let mut first = None;
        for chunk in chunks.iter() {
            if chunk.at_ms_from < t0 {
                continue;
            }
            if first.is_none() && !chunk.bytes.is_empty() {
                first = Some(chunk.at_ms_from.saturating_duration_since(t0).as_secs_f64() * 1000.0);
            }
            pcm.extend_from_slice(&chunk.bytes);
        }
        Ok(Spoken {
            pcm,
            first_audio_ms: first,
            wall_ms: elapsed_ms(t0),
        })
    }

    pub fn stderr_tail(&self) -> String {
        tail(&self.stderr.lock().expect("stderr lock"))
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.stdin.take();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn elapsed_ms(t0: Instant) -> f64 {
    t0.elapsed().as_secs_f64() * 1000.0
}

fn wait_for(buf: &Mutex<String>, needle: &str, limit: Duration) -> Result<Option<()>, String> {
    let start = Instant::now();
    loop {
        if buf.lock().expect("stderr lock").contains(needle) {
            return Ok(Some(()));
        }
        if start.elapsed() > limit {
            let tail = tail(&buf.lock().expect("stderr lock"));
            return Err(format!(
                "piper did not log \"{needle}\" within {}s. {tail}",
                limit.as_secs()
            ));
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn tail(text: &str) -> String {
    let t = text.trim();
    if t.len() <= 400 {
        t.to_string()
    } else {
        t[t.len() - 400..].to_string()
    }
}
