// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! Parent side of the Vosk worker. Spawns a process, writes one utterance,
//! reads partials until a final or an error. Never substitutes a sentence.
#![allow(unsafe_code)]

use crate::protocol::{
    encode_utterance, model_dir_ok, parse_event, Word, WorkerEvent, DEFAULT_LIB, DEFAULT_MODEL_DIR,
    SAMPLE_RATE,
};
use std::io::{Read, Write};
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

/// What one utterance came back as. `text` is the worker's final line, not a guess.
#[derive(Debug, Clone, PartialEq)]
pub struct RecogResult {
    pub text: String,
    pub partials: Vec<String>,
    pub words: Vec<Word>,
}

/// One warm `galion-asr-vosk` process (or a protocol peer in tests).
pub struct VoskClient {
    child: Child,
    stdin: ChildStdin,
    stdout: ChildStdout,
    buf: Vec<u8>,
}

impl VoskClient {
    /// Production spawn: model directory must already be on disk.
    pub fn open() -> Result<Self, String> {
        let model = resolve_model()?;
        let bin = resolve_bin()?;
        let lib = resolve_lib();
        let mut args = vec!["--model".to_string(), model.display().to_string()];
        if let Some(ref lib) = lib {
            args.push("--lib".into());
            args.push(lib.display().to_string());
        }
        let mut cmd = Command::new(&bin);
        cmd.args(&args);
        apply_cpu_env(&mut cmd);
        if let Some(dir) = lib.as_ref().and_then(|p| p.parent()) {
            prepend_ld(&mut cmd, dir);
        }
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        Self::from_command(cmd)
    }

    /// Spawn a protocol peer. Tests use this with `galion-asr-protocol-double`.
    pub fn from_bin(bin: &Path, args: &[String]) -> Result<Self, String> {
        let mut cmd = Command::new(bin);
        cmd.args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        Self::from_command(cmd)
    }

    fn from_command(mut cmd: Command) -> Result<Self, String> {
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("spawn vosk worker: {e}. No transcript was invented."))?;
        let stdin = match child.stdin.take() {
            Some(s) => s,
            None => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("vosk worker stdin missing".into());
            }
        };
        let stdout = match child.stdout.take() {
            Some(s) => s,
            None => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("vosk worker stdout missing".into());
            }
        };
        if let Err(e) = set_nonblock(stdout.as_raw_fd()) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("vosk worker nonblock: {e}"));
        }
        let mut client = Self {
            child,
            stdin,
            stdout,
            buf: Vec::new(),
        };
        match client.read_event(Duration::from_secs(90)) {
            Ok(WorkerEvent::Ready { .. }) => Ok(client),
            Ok(WorkerEvent::Error { error }) => Err(error),
            Ok(other) => Err(format!("vosk worker handshake was {other:?}")),
            Err(e) => Err(e),
        }
    }

    /// Send one utterance. `grammar` is forwarded as a JSON phrase list when `Some`.
    /// The returned text is exactly the worker's final line.
    pub fn recognize(
        &mut self,
        pcm: &[i16],
        grammar: Option<&[String]>,
    ) -> Result<RecogResult, String> {
        if pcm.is_empty() {
            return Err("empty audio".into());
        }
        if pcm.len() > crate::protocol::MAX_UTTERANCE_SAMPLES {
            return Err("utterance too long. No transcript was invented.".into());
        }
        let line = encode_utterance(SAMPLE_RATE, pcm, grammar);
        self.stdin
            .write_all(line.as_bytes())
            .map_err(|e| format!("vosk worker stdin: {e}"))?;
        self.stdin
            .write_all(b"\n")
            .map_err(|e| format!("vosk worker stdin: {e}"))?;
        self.stdin
            .flush()
            .map_err(|e| format!("vosk worker stdin flush: {e}"))?;
        let mut partials = Vec::new();
        loop {
            match self.read_event(Duration::from_secs(60))? {
                WorkerEvent::Partial { text, .. } => partials.push(text),
                WorkerEvent::Final { text, words, .. } => {
                    return Ok(RecogResult {
                        text,
                        partials,
                        words,
                    });
                }
                WorkerEvent::Error { error } => return Err(error),
                WorkerEvent::Ready { .. } => return Err("unexpected ready mid-utterance".into()),
            }
        }
    }

    fn read_event(&mut self, timeout: Duration) -> Result<WorkerEvent, String> {
        let line = self.read_line(timeout)?;
        parse_event(&line).map_err(|e| format!("vosk worker protocol: {e} ({})", clip(&line)))
    }

    fn read_line(&mut self, timeout: Duration) -> Result<String, String> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(i) = self.buf.iter().position(|&b| b == b'\n') {
                let raw: Vec<u8> = self.buf.drain(..=i).collect();
                let line = trim_line(&raw);
                if line.is_empty() {
                    continue;
                }
                return Ok(line);
            }
            if Instant::now() >= deadline {
                let exited = self.child.try_wait().ok().flatten();
                return Err(match exited {
                    Some(st) => format!("vosk worker exited ({st}) before a transcript. No transcript was invented."),
                    None => "vosk worker timed out before a transcript. No transcript was invented.".into(),
                });
            }
            let wait_ms = deadline
                .saturating_duration_since(Instant::now())
                .as_millis()
                .min(i32::MAX as u128) as i32;
            let mut fds = [PollFd {
                fd: self.stdout.as_raw_fd(),
                events: POLLIN | POLLHUP,
                revents: 0,
            }];
            // SAFETY: fds points at one live pipe fd owned by this client; nfds is 1.
            let rc = unsafe { poll(fds.as_mut_ptr(), 1, wait_ms) };
            if rc < 0 {
                let err = std::io::Error::last_os_error();
                if err.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(format!("vosk worker poll: {err}"));
            }
            if rc == 0 {
                continue;
            }
            let mut tmp = [0u8; 8192];
            match self.stdout.read(&mut tmp) {
                Ok(0) => {
                    let _ = self.child.try_wait();
                    if self.buf.is_empty() {
                        return Err("vosk worker closed stdout before a transcript. No transcript was invented.".into());
                    }
                    let line = trim_line(&std::mem::take(&mut self.buf));
                    if line.is_empty() {
                        return Err("vosk worker closed stdout before a transcript. No transcript was invented.".into());
                    }
                    return Ok(line);
                }
                Ok(n) => self.buf.extend_from_slice(&tmp[..n]),
                Err(e)
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(e) => return Err(format!("vosk worker read: {e}")),
            }
        }
    }
}

impl Drop for VoskClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub fn resolve_model() -> Result<PathBuf, String> {
    let raw = std::env::var("GALION_VOSK_MODEL").unwrap_or_else(|_| DEFAULT_MODEL_DIR.to_string());
    let path = PathBuf::from(&raw);
    if model_dir_ok(&path) {
        Ok(path)
    } else {
        Err(format!(
            "Vosk model not found at {} (need am/final.mdl and conf/model.conf). Download with scripts/asr/fetch-vosk.sh. Typed text still works. No transcript was invented. See docs/asr/VOSK.md",
            path.display()
        ))
    }
}

pub fn resolve_bin() -> Result<PathBuf, String> {
    if let Ok(p) = std::env::var("GALION_VOSK_BIN") {
        let path = PathBuf::from(&p);
        if path.is_file() {
            return Ok(path);
        }
        return Err(format!(
            "GALION_VOSK_BIN={p} is not a file. No transcript was invented. See docs/asr/VOSK.md"
        ));
    }
    let mut cands = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            cands.push(dir.join("galion-asr-vosk"));
            if let Some(parent) = dir.parent() {
                cands.push(parent.join("galion-asr-vosk"));
            }
        }
    }
    cands.push(PathBuf::from("target/debug/galion-asr-vosk"));
    cands.push(PathBuf::from("target/release/galion-asr-vosk"));
    for c in &cands {
        if c.is_file() {
            return Ok(c.clone());
        }
    }
    Err("galion-asr-vosk binary not found. Build it with `cargo build -p galion-asr-vosk --bin galion-asr-vosk`. No transcript was invented. See docs/asr/VOSK.md".into())
}

fn resolve_lib() -> Option<PathBuf> {
    let mut cands = Vec::new();
    if let Ok(p) = std::env::var("GALION_VOSK_LIB") {
        cands.push(PathBuf::from(p));
    }
    cands.push(PathBuf::from(DEFAULT_LIB));
    cands.into_iter().find(|p| p.is_file())
}

fn apply_cpu_env(cmd: &mut Command) {
    let n = std::env::var("GALION_VOSK_THREADS").unwrap_or_else(|_| "2".into());
    if std::env::var_os("OMP_NUM_THREADS").is_none() {
        cmd.env("OMP_NUM_THREADS", &n);
    }
    if std::env::var_os("OPENBLAS_NUM_THREADS").is_none() {
        cmd.env("OPENBLAS_NUM_THREADS", &n);
    }
}

fn prepend_ld(cmd: &mut Command, dir: &Path) {
    let dir = dir.display().to_string();
    let mut ld = std::env::var("LD_LIBRARY_PATH").unwrap_or_default();
    if !ld.split(':').any(|p| p == dir) {
        if !ld.is_empty() {
            ld.insert(0, ':');
        }
        ld.insert_str(0, &dir);
    }
    cmd.env("LD_LIBRARY_PATH", ld);
}

fn trim_line(bytes: &[u8]) -> String {
    let mut s = String::from_utf8_lossy(bytes).into_owned();
    while s.ends_with(['\n', '\r']) {
        s.pop();
    }
    s.trim().to_string()
}

fn clip(s: &str) -> String {
    let mut t: String = s.chars().take(180).collect();
    if s.chars().count() > 180 {
        t.push('…');
    }
    t
}

#[repr(C)]
struct PollFd {
    fd: i32,
    events: i16,
    revents: i16,
}

const POLLIN: i16 = 0x001;
const POLLHUP: i16 = 0x010;
const F_GETFL: i32 = 3;
const F_SETFL: i32 = 4;
const O_NONBLOCK: i32 = 0o4000;

extern "C" {
    fn poll(fds: *mut PollFd, nfds: u64, timeout: i32) -> i32;
    fn fcntl(fd: i32, cmd: i32, arg: i32) -> i32;
}

fn set_nonblock(fd: i32) -> std::io::Result<()> {
    // SAFETY: fd is a pipe this process owns; F_GETFL/F_SETFL take an int flag.
    let flags = unsafe { fcntl(fd, F_GETFL, 0) };
    if flags < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let rc = unsafe { fcntl(fd, F_SETFL, flags | O_NONBLOCK) };
    if rc < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}
