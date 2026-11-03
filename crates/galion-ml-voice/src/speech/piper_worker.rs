// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! Persistent Piper A21 worker (V2-S6). Out of process. No in-tree ONNX Runtime.
//!
//! One child per [`PiperWorker`]: `piper --json-input --output_raw`. Each request
//! is one JSON line `{"text":"..."}` on stdin. Piper 2023.11.14-2 writes raw
//! s16le PCM to stdout and, after that write is flushed, logs
//! `Real-time factor:` on stderr. That log line is the utterance boundary.
//! PCM stays in RAM. Nothing is written to disk for warmth, and barge-in does
//! not kill this process (the owner drops queued clauses; [`Drop`] is only
//! lane teardown).
#![allow(unsafe_code)]

use crate::VoiceError;
use std::io::{Read, Write};
use std::os::unix::io::AsRawFd;
use std::path::Path;
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

/// Scales passed on the Piper command line when the worker starts.
/// Voice identity is the A21 pack, not these numbers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PiperProsody {
    pub length_scale: String,
    pub noise_scale: String,
    pub noise_w: String,
}

impl PiperProsody {
    /// Deterministic (`noise_scale=0`, `noise_w=0`) unless `GALION_A21_NOISE=natural`.
    pub fn from_env() -> Self {
        let natural = std::env::var("GALION_A21_NOISE").ok().as_deref() == Some("natural");
        if natural {
            Self { length_scale: "1.0".into(), noise_scale: "0.667".into(), noise_w: "0.8".into() }
        } else {
            Self { length_scale: "1.0".into(), noise_scale: "0".into(), noise_w: "0".into() }
        }
    }

    /// V2-S6 prosody hook. `Neutral` (phone default, consent deny) returns `self`.
    /// Non-neutral is not applied: a later consent-gated caller can change
    /// `length_scale` here without swapping the voice pack.
    pub fn with_emotion(&self, emotion: &str) -> Self {
        match emotion.trim() {
            "Neutral" | "neutral" | "" => self.clone(),
            _ => self.clone(),
        }
    }
}

/// One warm Piper process.
pub struct PiperWorker {
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    stdout: Option<ChildStdout>,
    stderr: Option<ChildStderr>,
    err_log: String,
    pcm_buf: Vec<u8>,
    pid: u32,
    calls: u32,
    cold_ms: Option<u32>,
    warm_ms: Option<u32>,
    first_chunk_ms: Option<u32>,
}

impl PiperWorker {
    /// Spawn Piper. The model loads inside the child before it reads stdin.
    /// The first [`synthesize`](Self::synthesize) therefore pays load + inference.
    pub fn spawn(runtime: &Path, model: &Path, config: &Path, prosody: &PiperProsody) -> Result<Self, VoiceError> {
        let mut cmd = Command::new(runtime);
        cmd.arg("--model").arg(model)
            .arg("--config").arg(config)
            .arg("--output_raw")
            .arg("--json-input")
            .arg("--noise_scale").arg(&prosody.noise_scale)
            .arg("--noise_w").arg(&prosody.noise_w)
            .arg("--length_scale").arg(&prosody.length_scale)
            .arg("--sentence_silence").arg("0")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // lat240: optional core pinning (`GALION_PIPER_CPUS=6-7`). The pinned Piper
        // CLI starts one ONNX intra-op thread per core and exposes no thread
        // setting, so affinity is the only CPU lever it has. Unset = inherit.
        if let Some(cpus) = std::env::var("GALION_PIPER_CPUS").ok().and_then(|v| parse_cpus(&v)) {
            use std::os::unix::process::CommandExt;
            // SAFETY: pre_exec runs in the forked child before exec; sched_setaffinity
            // is async-signal-safe and only touches the child's own mask.
            unsafe { cmd.pre_exec(move || set_affinity(&cpus)); }
        }
        let mut child = cmd
            .spawn()
            .map_err(|e| VoiceError::Backend(format!("A21 piper spawn {}: {e}", runtime.display())))?;
        let pid = child.id();
        let stdin = child.stdin.take();
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let (Some(stdin), Some(stdout), Some(stderr)) = (stdin, stdout, stderr) else {
            let _ = child.kill();
            let _ = child.wait();
            return Err(VoiceError::Backend("A21 piper pipes missing".into()));
        };
        if let Err(e) = set_nonblock(stdout.as_raw_fd()).and_then(|_| set_nonblock(stderr.as_raw_fd())) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(VoiceError::Backend(format!("A21 piper nonblock: {e}")));
        }
        Ok(Self {
            child: Some(child), stdin: Some(stdin), stdout: Some(stdout), stderr: Some(stderr),
            err_log: String::new(), pcm_buf: Vec::new(), pid, calls: 0, cold_ms: None, warm_ms: None, first_chunk_ms: None,
        })
    }

    pub fn pid(&self) -> u32 { self.pid }
    pub fn calls(&self) -> u32 { self.calls }
    pub fn cold_ms(&self) -> Option<u32> { self.cold_ms }
    pub fn warm_ms(&self) -> Option<u32> { self.warm_ms }

    pub fn is_alive(&mut self) -> bool {
        match self.child.as_mut().map(|c| c.try_wait()) {
            Some(Ok(None)) => true,
            _ => false,
        }
    }

    /// One clause in, one PCM buffer out. Same process on every call.
    pub fn synthesize(&mut self, text: &str) -> Result<Vec<i16>, VoiceError> {
        let mut all = Vec::new();
        self.synthesize_stream(text, &mut |pcm: &[i16]| all.extend_from_slice(pcm))?;
        if all.is_empty() {
            return Err(VoiceError::Backend(format!("A21 piper returned no PCM ({})", tail(&self.err_log))));
        }
        Ok(all)
    }

    /// Streaming form: `on_pcm` gets each s16 chunk the moment it is read off
    /// Piper's stdout (Piper flushes once per sentence), not after the
    /// end-of-utterance marker. The first call is the first audio.
    pub fn synthesize_stream(&mut self, text: &str, on_pcm: &mut dyn FnMut(&[i16])) -> Result<(), VoiceError> {
        let t0 = Instant::now();
        let line = json_line(text);
        {
            let stdin = self.stdin.as_mut().ok_or_else(|| VoiceError::Backend("A21 piper stdin closed".into()))?;
            stdin.write_all(line.as_bytes()).map_err(|e| VoiceError::Backend(format!("A21 piper stdin: {e}")))?;
            stdin.flush().map_err(|e| VoiceError::Backend(format!("A21 piper stdin flush: {e}")))?;
        }
        self.pcm_buf.clear();
        self.first_chunk_ms = None;
        let mut first: Option<u32> = None;
        let mut emit = |buf: &mut Vec<u8>| {
            let whole = buf.len() & !1;
            if whole == 0 { return; }
            if first.is_none() { first = Some(elapsed_ms(t0)); }
            let pcm: Vec<i16> = buf[..whole].chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]])).collect();
            buf.drain(..whole);
            on_pcm(&pcm);
        };
        self.read_utterance(Duration::from_secs(60), &mut emit)?;
        self.first_chunk_ms = first;
        if first.is_none() {
            return Err(VoiceError::Backend(format!("A21 piper returned no PCM ({})", tail(&self.err_log))));
        }
        let ms = first.unwrap_or_else(|| elapsed_ms(t0));
        self.calls += 1;
        if self.calls == 1 { self.cold_ms = Some(ms); }
        else if self.calls == 2 { self.warm_ms = Some(ms); }
        if self.err_log.len() > 8192 {
            let mut cut = self.err_log.len() - 2048;
            while cut < self.err_log.len() && !self.err_log.is_char_boundary(cut) { cut += 1; }
            self.err_log.drain(..cut);
        }
        Ok(())
    }

    /// Request write -> first PCM byte of the last utterance.
    pub fn first_chunk_ms(&self) -> Option<u32> { self.first_chunk_ms }

    fn read_utterance(&mut self, timeout: Duration, emit: &mut dyn FnMut(&mut Vec<u8>)) -> Result<(), VoiceError> {
        let deadline = Instant::now() + timeout;
        let mark = self.err_log.len();
        loop {
            if Instant::now() >= deadline {
                return Err(VoiceError::Backend(format!("A21 piper timed out ({})", tail(&self.err_log))));
            }
            let remain = deadline.saturating_duration_since(Instant::now());
            let wait_ms = remain.as_millis().min(i32::MAX as u128) as i32;
            let stdout_fd = self.stdout.as_ref().unwrap().as_raw_fd();
            let stderr_fd = self.stderr.as_ref().unwrap().as_raw_fd();
            let mut fds = [
                PollFd { fd: stdout_fd, events: POLLIN, revents: 0 },
                PollFd { fd: stderr_fd, events: POLLIN, revents: 0 },
            ];
            // SAFETY: fds points at two live pipe fds owned by this worker; nfds is 2.
            let rc = unsafe { poll(fds.as_mut_ptr(), 2, wait_ms) };
            if rc < 0 {
                let err = std::io::Error::last_os_error();
                if err.kind() == std::io::ErrorKind::Interrupted { continue; }
                return Err(VoiceError::Backend(format!("A21 piper poll: {err}")));
            }
            let mut progress = false;
            if rc > 0 {
                if fds[0].revents & (POLLIN | POLLHUP) != 0 {
                    self.drain(Pipe::Out)?;
                    emit(&mut self.pcm_buf);
                    progress = true;
                }
                if fds[1].revents & (POLLIN | POLLHUP) != 0 {
                    self.drain(Pipe::Err)?;
                    progress = true;
                }
            }
            if self.err_log[mark..].contains("Real-time factor:") {
                if self.stdout.as_ref().is_some() {
                    // The marker is logged after stdout is flushed. Take any bytes still queued.
                    let _ = self.drain(Pipe::Out);
                    emit(&mut self.pcm_buf);
                }
                break;
            }
            let exited = matches!(self.child.as_mut().map(|c| c.try_wait()), Some(Ok(Some(_))));
            if exited {
                let _ = self.drain(Pipe::Out);
                emit(&mut self.pcm_buf);
                let _ = self.drain(Pipe::Err);
                if self.err_log[mark..].contains("Real-time factor:") { break; }
                return Err(VoiceError::Backend(format!("A21 piper exited ({})", tail(&self.err_log))));
            }
            if !progress {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        self.pcm_buf.clear();
        Ok(())
    }

    fn drain(&mut self, pipe: Pipe) -> Result<(), VoiceError> {
        let mut buf = [0u8; 8192];
        loop {
            let read = match pipe {
                Pipe::Out => self.stdout.as_mut().unwrap().read(&mut buf),
                Pipe::Err => self.stderr.as_mut().unwrap().read(&mut buf),
            };
            match read {
                Ok(0) => return Ok(()),
                Ok(n) => match pipe {
                    Pipe::Out => self.pcm_buf.extend_from_slice(&buf[..n]),
                    Pipe::Err => self.err_log.push_str(&String::from_utf8_lossy(&buf[..n])),
                },
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(VoiceError::Backend(format!("A21 piper read: {e}"))),
            }
        }
    }
}

impl Drop for PiperWorker {
    fn drop(&mut self) {
        // Lane teardown only. Barge-in must not reach here.
        self.stdin.take();
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[derive(Clone, Copy)]
enum Pipe { Out, Err }

fn json_line(text: &str) -> String {
    let mut s = String::with_capacity(text.len() + 16);
    s.push_str("{\"text\":\"");
    for c in text.chars() {
        match c {
            '"' => s.push_str("\\\""),
            '\\' => s.push_str("\\\\"),
            '\n' | '\r' | '\t' => s.push(' '),
            c if (c as u32) < 0x20 => s.push(' '),
            c => s.push(c),
        }
    }
    s.push_str("\"}\n");
    s
}

fn tail(s: &str) -> String {
    let t = s.trim();
    if t.len() <= 300 { t.to_string() } else { t[t.len() - 300..].to_string() }
}

fn elapsed_ms(t0: Instant) -> u32 { t0.elapsed().as_millis().min(u32::MAX as u128) as u32 }

#[repr(C)]
struct PollFd { fd: i32, events: i16, revents: i16 }

const POLLIN: i16 = 0x001;
const POLLHUP: i16 = 0x010;
const F_GETFL: i32 = 3;
const F_SETFL: i32 = 4;
const O_NONBLOCK: i32 = 0o4000;

extern "C" {
    fn poll(fds: *mut PollFd, nfds: u64, timeout: i32) -> i32;
    fn fcntl(fd: i32, cmd: i32, arg: i32) -> i32;
}

/// `"6-7"`, `"7"`, `"0,2-3"` -> cpu list. `None` on any parse error.
pub fn parse_cpus(s: &str) -> Option<Vec<usize>> {
    let mut v = Vec::new();
    for part in s.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let (lo, hi) = part.split_once('-').unwrap_or((part, part));
        let (lo, hi): (usize, usize) = (lo.trim().parse().ok()?, hi.trim().parse().ok()?);
        if hi < lo || hi >= 1024 { return None; }
        v.extend(lo..=hi);
    }
    if v.is_empty() { None } else { Some(v) }
}

extern "C" {
    fn sched_setaffinity(pid: i32, cpusetsize: usize, mask: *const u64) -> i32;
}

fn set_affinity(cpus: &[usize]) -> std::io::Result<()> {
    let mut mask = [0u64; 16]; // 1024 cpus, glibc cpu_set_t size
    for &c in cpus { mask[c / 64] |= 1u64 << (c % 64); }
    // SAFETY: mask is a 128-byte bitset that outlives the call; pid 0 = calling (child) process.
    let rc = unsafe { sched_setaffinity(0, std::mem::size_of_val(&mask), mask.as_ptr()) };
    if rc != 0 { return Err(std::io::Error::last_os_error()); }
    Ok(())
}

fn set_nonblock(fd: i32) -> std::io::Result<()> {
    // SAFETY: fd is a pipe this process owns; F_GETFL/F_SETFL take an int flag.
    let flags = unsafe { fcntl(fd, F_GETFL, 0) };
    if flags < 0 { return Err(std::io::Error::last_os_error()); }
    let rc = unsafe { fcntl(fd, F_SETFL, flags | O_NONBLOCK) };
    if rc < 0 { return Err(std::io::Error::last_os_error()); }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn neutral_prosody_keeps_scales() {
        let p = PiperProsody::from_env();
        assert_eq!(p.with_emotion("Neutral"), p);
        assert_eq!(p.with_emotion("neutral"), p);
        assert_eq!(p.with_emotion(""), p);
        // Voice identity is not a prosody field.
        assert!(p.length_scale == "1.0" || p.length_scale == "1");
    }

    #[test]
    fn cpu_lists_parse() {
        assert_eq!(parse_cpus("6-7"), Some(vec![6, 7]));
        assert_eq!(parse_cpus("0,2-3"), Some(vec![0, 2, 3]));
        assert_eq!(parse_cpus("7"), Some(vec![7]));
        assert_eq!(parse_cpus("3-1"), None);
        assert_eq!(parse_cpus("x"), None);
        assert_eq!(parse_cpus(""), None);
    }

    #[test]
    fn json_line_escapes() {
        let s = json_line("say \"hi\" \\ now");
        assert_eq!(s, "{\"text\":\"say \\\"hi\\\" \\\\ now\"}\n");
        assert!(!s[..s.len() - 1].contains('\n'));
    }

    /// Two syntheses, one process. The fake runtime speaks the same protocol
    /// as Piper 2023.11.14-2: raw s16le, then a `Real-time factor:` line.
    #[test]
    fn two_turns_reuse_one_piper_process() {
        let dir = std::env::temp_dir().join(format!("galion-piper-worker-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("fake-piper");
        std::fs::write(&script, FAKE_PIPER).unwrap();
        let mut perm = std::fs::metadata(&script).unwrap().permissions();
        perm.set_mode(0o755);
        std::fs::set_permissions(&script, perm).unwrap();
        let model = dir.join("voice.onnx");
        let config = dir.join("voice.onnx.json");
        std::fs::write(&model, b"not-a-model").unwrap();
        std::fs::write(&config, b"{}").unwrap();
        let before = entries(&dir);

        let mut w = PiperWorker::spawn(&script, &model, &config, &PiperProsody {
            length_scale: "1.0".into(), noise_scale: "0".into(), noise_w: "0".into(),
        }).unwrap();
        let pid = w.pid();
        let a = w.synthesize("turn one").unwrap();
        let b = w.synthesize("turn two says hi").unwrap();
        assert_eq!(w.pid(), pid, "pid changed across turns");
        assert!(w.is_alive());
        assert_eq!(w.calls(), 2);
        assert!(w.cold_ms().is_some() && w.warm_ms().is_some());
        assert_ne!(a[0], b[0], "second turn must not replay the first PCM");
        // Streaming form: chunks arrive through the callback, same samples as the buffered form.
        let mut chunks = 0usize;
        let mut got = Vec::new();
        w.synthesize_stream("turn one", &mut |p: &[i16]| { chunks += 1; got.extend_from_slice(p); }).unwrap();
        assert!(chunks >= 1);
        assert_eq!(got, a, "streamed PCM differs from buffered PCM");
        assert!(w.first_chunk_ms().is_some());
        assert!(a.iter().any(|&s| s != 0));
        // No PCM file appeared beside the fake runtime.
        let after = entries(&dir);
        assert_eq!(before, after, "warmth must not cache PCM on disk: {after:?}");
        drop(w);
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn entries(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        v.sort();
        v
    }

    const FAKE_PIPER: &str = r#"#!/usr/bin/env python3
import json, struct, sys
while True:
    line = sys.stdin.buffer.readline()
    if not line:
        break
    text = json.loads(line.decode()).get("text", "")
    n = max(1, min(len(text), 32767))
    pcm = struct.pack("<h", n) + struct.pack("<200h", *([1000 + (n % 50)] * 200))
    sys.stdout.buffer.write(pcm)
    sys.stdout.buffer.flush()
    sys.stderr.write("Real-time factor: 0.10 (infer=0.01 sec, audio=0.20 sec)\n")
    sys.stderr.flush()
"#;
}
