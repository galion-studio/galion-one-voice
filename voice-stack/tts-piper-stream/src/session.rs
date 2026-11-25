// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! One warm Piper child. Linux reads stdout and stderr with `poll`, the same
//! loop as `galion-ml-voice` `PiperWorker`. Other targets use a reader thread
//! and a 2 ms drain confirm so a flush that lands in the other thread is not
//! cut off. `GALION_PIPER_IO=thread` is not used; tests pass [`IoMode::Thread`].
#![allow(unsafe_code)]

use crate::mood::{shape_pcm, Prosody};
use crate::piper::{IoMode, PiperVoice};
use crate::protocol::{json_line, parse_rtf, tail, Rtf};
use crate::tts::{ms, ms_since, PcmChunk, SentenceStat, StreamEvent, TtsError};
use std::io::{Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const PHASE_IN_READ: u8 = 1;
const PHASE_HOLDING: u8 = 2;
pub(crate) const THREAD_DRAIN_CONFIRM_MS: u64 = 2;

enum Msg {
    Out(Vec<u8>),
    Err(String),
    Eof,
}

enum Io {
    #[cfg(target_os = "linux")]
    Poll {
        stdout: std::process::ChildStdout,
        stderr: std::process::ChildStderr,
    },
    Thread {
        rx: Receiver<Msg>,
        phase: Arc<AtomicU8>,
        peek: Peek,
        joins: Vec<JoinHandle<()>>,
    },
}

pub(crate) struct Session {
    child: Child,
    stdin: Option<ChildStdin>,
    io: Io,
    err_log: String,
    pid: u32,
    spawned_at: Instant,
    cold_spawn_to_first_pcm_ms: Option<u32>,
    io_name: &'static str,
}

pub(crate) struct SessionOut {
    pub events: Vec<StreamEvent>,
    pub detail: Vec<SentenceStat>,
    pub samples: usize,
    pub wall_ms: u32,
    pub cold_spawn_to_first_pcm_ms: Option<u32>,
    pub pid: u32,
}

struct RawUtt {
    pcm: Vec<i16>,
    first_byte_ms: u32,
    done_ms: u32,
    rtf: Option<Rtf>,
}

impl Session {
    pub(crate) fn spawn(voice: &PiperVoice) -> Result<Self, TtsError> {
        let mut cmd = Command::new(&voice.runtime);
        cmd.arg("--model")
            .arg(&voice.model)
            .arg("--config")
            .arg(&voice.config)
            .arg("--output_raw")
            .arg("--json-input")
            .arg("--noise_scale")
            .arg(&voice.noise_scale)
            .arg("--noise_w")
            .arg(&voice.noise_w)
            .arg("--length_scale")
            .arg(&voice.length_scale)
            .arg("--sentence_silence")
            .arg(&voice.sentence_silence)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(target_os = "linux")]
        {
            prepend_lib_path(&mut cmd, &voice.runtime);
            maybe_affinity(&mut cmd);
        }
        let spawned_at = Instant::now();
        let mut child = cmd
            .spawn()
            .map_err(|e| TtsError::Io(format!("piper spawn {}: {e}", voice.runtime.display())))?;
        let pid = child.id();
        let stdin = child.stdin.take();
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let (Some(stdin), Some(stdout), Some(stderr)) = (stdin, stdout, stderr) else {
            let _ = child.kill();
            let _ = child.wait();
            return Err(TtsError::Io("piper pipes missing".into()));
        };
        let mode = match voice.io {
            IoMode::Auto => {
                if cfg!(target_os = "linux") {
                    "poll"
                } else {
                    "thread"
                }
            }
            IoMode::Poll => "poll",
            IoMode::Thread => "thread",
        };
        let use_poll = mode == "poll";
        if use_poll && !cfg!(target_os = "linux") {
            let _ = child.kill();
            let _ = child.wait();
            return Err(TtsError::Io("poll io is linux-only".into()));
        }
        let (io, io_name) = if use_poll {
            #[cfg(target_os = "linux")]
            {
                if let Err(e) =
                    set_nonblock(stdout.as_raw_fd()).and_then(|_| set_nonblock(stderr.as_raw_fd()))
                {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(TtsError::Io(format!("piper nonblock: {e}")));
                }
                (Io::Poll { stdout, stderr }, "poll")
            }
            #[cfg(not(target_os = "linux"))]
            {
                let _ = (stdout, stderr);
                unreachable!("poll is linux-only");
            }
        } else {
            match spawn_threads(stdout, stderr) {
                Ok(io) => (io, "thread"),
                Err(e) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(e);
                }
            }
        };
        Ok(Self {
            child,
            stdin: Some(stdin),
            io,
            err_log: String::new(),
            pid,
            spawned_at,
            cold_spawn_to_first_pcm_ms: None,
            io_name,
        })
    }

    pub(crate) fn pid(&self) -> u32 {
        self.pid
    }

    pub(crate) fn io_name(&self) -> &'static str {
        self.io_name
    }

    pub(crate) fn is_alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    pub(crate) fn speak(
        &mut self,
        clauses: &[String],
        prosody: Prosody,
        rate: u32,
        on_pcm: &mut dyn FnMut(PcmChunk<'_>),
    ) -> Result<SessionOut, TtsError> {
        if clauses.is_empty() {
            return Err(TtsError::Empty);
        }
        let t0 = Instant::now();
        let mut events = Vec::new();
        let mut detail = Vec::new();
        let mut total = 0usize;
        let mut submit_at = Instant::now();
        events.push(StreamEvent::Submit { sentence: 0, ms: 0 });
        self.write_line(&clauses[0])?;
        for i in 0..clauses.len() {
            let utt = self.read_utterance(submit_at)?;
            if i + 1 < clauses.len() {
                events.push(StreamEvent::Submit {
                    sentence: (i as u32) + 1,
                    ms: ms_since(t0),
                });
                submit_at = Instant::now();
                self.write_line(&clauses[i + 1])?;
            }
            let shaped = shape_pcm(&utt.pcm, prosody, rate, i == 0);
            events.push(StreamEvent::Callback {
                sentence: i as u32,
                ms: ms_since(t0),
            });
            on_pcm(PcmChunk {
                sentence_index: i as u32,
                sample_rate: rate,
                samples: &shaped,
                first_of_utterance: i == 0,
            });
            total += shaped.len();
            let rtf = utt.rtf.as_ref();
            detail.push(SentenceStat {
                index: i as u32,
                chars: clauses[i].chars().count() as u32,
                submit_to_first_byte_ms: utt.first_byte_ms,
                submit_to_done_ms: utt.done_ms,
                samples: shaped.len() as u32,
                piper_rtf: rtf.map(|r| r.factor),
                piper_infer_s: rtf.and_then(|r| r.infer_s),
                piper_audio_s: rtf.and_then(|r| r.audio_s),
            });
        }
        self.trim_err();
        Ok(SessionOut {
            events,
            detail,
            samples: total,
            wall_ms: ms_since(t0),
            cold_spawn_to_first_pcm_ms: self.cold_spawn_to_first_pcm_ms,
            pid: self.pid,
        })
    }

    fn write_line(&mut self, text: &str) -> Result<(), TtsError> {
        let line = json_line(text);
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| TtsError::Died("stdin closed".into()))?;
        stdin
            .write_all(line.as_bytes())
            .map_err(|e| TtsError::Io(format!("stdin: {e}")))?;
        stdin
            .flush()
            .map_err(|e| TtsError::Io(format!("stdin flush: {e}")))?;
        Ok(())
    }

    fn read_utterance(&mut self, started: Instant) -> Result<RawUtt, TtsError> {
        let err_from = self.err_log.len();
        let mut raw = Vec::new();
        let mut pcm = Vec::new();
        let mut first: Option<u32> = None;
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            if Instant::now() >= deadline {
                return Err(TtsError::Timeout(tail(&self.err_log)));
            }
            self.pump_once(started, &mut raw, &mut pcm, &mut first, deadline)?;
            if rtf_since(&self.err_log, err_from) {
                self.finish_drain(started, &mut raw, &mut pcm, &mut first)?;
                break;
            }
        }
        let first_byte_ms =
            first.ok_or_else(|| TtsError::Protocol(format!("no PCM ({})", tail(&self.err_log))))?;
        let rtf = self.err_log[err_from..].lines().rev().find_map(parse_rtf);
        Ok(RawUtt {
            pcm,
            first_byte_ms,
            done_ms: ms_since(started),
            rtf,
        })
    }

    fn pump_once(
        &mut self,
        started: Instant,
        raw: &mut Vec<u8>,
        pcm: &mut Vec<i16>,
        first: &mut Option<u32>,
        deadline: Instant,
    ) -> Result<(), TtsError> {
        match &mut self.io {
            #[cfg(target_os = "linux")]
            Io::Poll { stdout, stderr } => {
                let remain = deadline.saturating_duration_since(Instant::now());
                let wait_ms = remain.as_millis().min(i32::MAX as u128) as i32;
                let stdout_fd = stdout.as_raw_fd();
                let stderr_fd = stderr.as_raw_fd();
                let mut fds = [
                    PollFd {
                        fd: stdout_fd,
                        events: POLLIN,
                        revents: 0,
                    },
                    PollFd {
                        fd: stderr_fd,
                        events: POLLIN,
                        revents: 0,
                    },
                ];
                let rc = unsafe { poll(fds.as_mut_ptr(), 2, wait_ms) };
                if rc < 0 {
                    let err = std::io::Error::last_os_error();
                    if err.kind() == std::io::ErrorKind::Interrupted {
                        return Ok(());
                    }
                    return Err(TtsError::Io(format!("poll: {err}")));
                }
                let mut progress = false;
                if rc > 0 && fds[0].revents & (POLLIN | POLLHUP) != 0 {
                    read_fd(
                        stdout,
                        raw,
                        pcm,
                        first,
                        started,
                        &mut self.cold_spawn_to_first_pcm_ms,
                        self.spawned_at,
                    )?;
                    progress = true;
                }
                if rc > 0 && fds[1].revents & (POLLIN | POLLHUP) != 0 {
                    let mut buf = [0u8; 4096];
                    match stderr.read(&mut buf) {
                        Ok(0) => {}
                        Ok(n) => {
                            self.err_log.push_str(&String::from_utf8_lossy(&buf[..n]));
                            progress = true;
                        }
                        Err(e)
                            if e.kind() == std::io::ErrorKind::WouldBlock
                                || e.kind() == std::io::ErrorKind::Interrupted => {}
                        Err(e) => return Err(TtsError::Io(format!("stderr: {e}"))),
                    }
                }
                if !progress {
                    if let Ok(Some(st)) = self.child.try_wait() {
                        let _ = read_fd(
                            stdout,
                            raw,
                            pcm,
                            first,
                            started,
                            &mut self.cold_spawn_to_first_pcm_ms,
                            self.spawned_at,
                        );
                        return Err(TtsError::Died(format!(
                            "status {st} ({})",
                            tail(&self.err_log)
                        )));
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
                Ok(())
            }
            Io::Thread { rx, .. } => {
                let left = deadline.saturating_duration_since(Instant::now());
                match rx.recv_timeout(left) {
                    Ok(msg) => self.ingest(msg, started, raw, pcm, first),
                    Err(RecvTimeoutError::Timeout) => Err(TtsError::Timeout(tail(&self.err_log))),
                    Err(RecvTimeoutError::Disconnected) => Err(TtsError::Died(tail(&self.err_log))),
                }
            }
        }
    }

    fn finish_drain(
        &mut self,
        started: Instant,
        raw: &mut Vec<u8>,
        pcm: &mut Vec<i16>,
        first: &mut Option<u32>,
    ) -> Result<(), TtsError> {
        match &mut self.io {
            #[cfg(target_os = "linux")]
            Io::Poll { stdout, .. } => {
                let _ = read_fd(
                    stdout,
                    raw,
                    pcm,
                    first,
                    started,
                    &mut self.cold_spawn_to_first_pcm_ms,
                    self.spawned_at,
                );
                Ok(())
            }
            Io::Thread { .. } => self.confirm_thread(started, raw, pcm, first),
        }
    }

    fn confirm_thread(
        &mut self,
        started: Instant,
        raw: &mut Vec<u8>,
        pcm: &mut Vec<i16>,
        first: &mut Option<u32>,
    ) -> Result<(), TtsError> {
        for _ in 0..5 {
            self.drain_available(started, raw, pcm, first)?;
            let (phase, pending) = self.thread_state();
            if phase == PHASE_HOLDING || pending > 0 {
                self.wait_thread(Duration::from_millis(50), started, raw, pcm, first)?;
                continue;
            }
            std::thread::sleep(Duration::from_millis(THREAD_DRAIN_CONFIRM_MS));
            self.drain_available(started, raw, pcm, first)?;
            let (phase, pending) = self.thread_state();
            if phase == PHASE_IN_READ && pending == 0 {
                return Ok(());
            }
        }
        self.drain_available(started, raw, pcm, first)
    }

    fn thread_state(&self) -> (u8, usize) {
        match &self.io {
            Io::Thread { phase, peek, .. } => (phase.load(Ordering::Acquire), peek.pending()),
            #[cfg(target_os = "linux")]
            Io::Poll { .. } => (PHASE_IN_READ, 0),
        }
    }

    fn drain_available(
        &mut self,
        started: Instant,
        raw: &mut Vec<u8>,
        pcm: &mut Vec<i16>,
        first: &mut Option<u32>,
    ) -> Result<(), TtsError> {
        loop {
            let msg = match &self.io {
                Io::Thread { rx, .. } => match rx.try_recv() {
                    Ok(m) => m,
                    Err(_) => return Ok(()),
                },
                #[cfg(target_os = "linux")]
                Io::Poll { .. } => return Ok(()),
            };
            self.ingest(msg, started, raw, pcm, first)?;
        }
    }

    fn wait_thread(
        &mut self,
        dur: Duration,
        started: Instant,
        raw: &mut Vec<u8>,
        pcm: &mut Vec<i16>,
        first: &mut Option<u32>,
    ) -> Result<(), TtsError> {
        let msg = match &self.io {
            Io::Thread { rx, .. } => match rx.recv_timeout(dur) {
                Ok(m) => m,
                Err(_) => return Ok(()),
            },
            #[cfg(target_os = "linux")]
            Io::Poll { .. } => return Ok(()),
        };
        self.ingest(msg, started, raw, pcm, first)
    }

    fn ingest(
        &mut self,
        msg: Msg,
        started: Instant,
        raw: &mut Vec<u8>,
        pcm: &mut Vec<i16>,
        first: &mut Option<u32>,
    ) -> Result<(), TtsError> {
        match msg {
            Msg::Out(bytes) => {
                if !bytes.is_empty() {
                    note_first(
                        first,
                        started,
                        &mut self.cold_spawn_to_first_pcm_ms,
                        self.spawned_at,
                    );
                }
                raw.extend_from_slice(&bytes);
                take_i16(raw, pcm);
                Ok(())
            }
            Msg::Err(line) => {
                self.err_log.push_str(&line);
                if !line.ends_with('\n') {
                    self.err_log.push('\n');
                }
                Ok(())
            }
            Msg::Eof => {
                if pcm.is_empty() && !self.err_log.contains("Real-time factor:") {
                    Err(TtsError::Died(tail(&self.err_log)))
                } else {
                    Ok(())
                }
            }
        }
    }

    fn trim_err(&mut self) {
        if self.err_log.len() > 8192 {
            let mut cut = self.err_log.len() - 2048;
            while cut < self.err_log.len() && !self.err_log.is_char_boundary(cut) {
                cut += 1;
            }
            self.err_log.drain(..cut);
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.stdin.take();
        let _ = self.child.kill();
        let _ = self.child.wait();
        // `Poll` exists only on Linux, so a bare `if let Thread` is irrefutable elsewhere.
        #[cfg(target_os = "linux")]
        {
            if let Io::Thread { joins, .. } = &mut self.io {
                for h in joins.drain(..) {
                    let _ = h.join();
                }
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            let Io::Thread { joins, .. } = &mut self.io;
            for h in joins.drain(..) {
                let _ = h.join();
            }
        }
    }
}

fn rtf_since(log: &str, from: usize) -> bool {
    log.get(from..)
        .map(|s| s.contains("Real-time factor:"))
        .unwrap_or(false)
}

fn note_first(
    first: &mut Option<u32>,
    started: Instant,
    cold: &mut Option<u32>,
    spawned_at: Instant,
) {
    if first.is_none() {
        *first = Some(ms_since(started));
    }
    if cold.is_none() {
        *cold = Some(ms(spawned_at.elapsed()));
    }
}

fn take_i16(raw: &mut Vec<u8>, pcm: &mut Vec<i16>) {
    let n = raw.len() & !1;
    pcm.extend(
        raw[..n]
            .chunks_exact(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]])),
    );
    raw.drain(..n);
}

fn spawn_threads(
    mut stdout: std::process::ChildStdout,
    mut stderr: std::process::ChildStderr,
) -> Result<Io, TtsError> {
    let peek = Peek::from_stdout(&stdout)?;
    let phase = Arc::new(AtomicU8::new(PHASE_IN_READ));
    let (tx, rx) = mpsc::channel();
    let phase_out = Arc::clone(&phase);
    let tx_out = tx.clone();
    let out_thread = std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        loop {
            phase_out.store(PHASE_IN_READ, Ordering::Release);
            match stdout.read(&mut buf) {
                Ok(0) => {
                    phase_out.store(3, Ordering::Release);
                    let _ = tx_out.send(Msg::Eof);
                    break;
                }
                Ok(n) => {
                    phase_out.store(PHASE_HOLDING, Ordering::Release);
                    if tx_out.send(Msg::Out(buf[..n].to_vec())).is_err() {
                        break;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => {
                    phase_out.store(3, Ordering::Release);
                    let _ = tx_out.send(Msg::Eof);
                    break;
                }
            }
        }
    });
    let err_thread = std::thread::spawn(move || {
        let mut buf = [0u8; 2048];
        let mut carry = String::new();
        loop {
            match stderr.read(&mut buf) {
                Ok(0) => {
                    if !carry.is_empty() {
                        let _ = tx.send(Msg::Err(carry));
                    }
                    let _ = tx.send(Msg::Eof);
                    break;
                }
                Ok(n) => {
                    carry.push_str(&String::from_utf8_lossy(&buf[..n]));
                    while let Some(i) = carry.find('\n') {
                        let line = carry.drain(..=i).collect::<String>();
                        if tx.send(Msg::Err(line)).is_err() {
                            return;
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => {
                    let _ = tx.send(Msg::Eof);
                    break;
                }
            }
        }
    });
    Ok(Io::Thread {
        rx,
        phase,
        peek,
        joins: vec![out_thread, err_thread],
    })
}

#[cfg(target_os = "linux")]
fn read_fd(
    pipe: &mut impl Read,
    raw: &mut Vec<u8>,
    pcm: &mut Vec<i16>,
    first: &mut Option<u32>,
    started: Instant,
    cold: &mut Option<u32>,
    spawned_at: Instant,
) -> Result<(), TtsError> {
    let mut buf = [0u8; 8192];
    loop {
        match pipe.read(&mut buf) {
            Ok(0) => return Ok(()),
            Ok(n) => {
                note_first(first, started, cold, spawned_at);
                raw.extend_from_slice(&buf[..n]);
                take_i16(raw, pcm);
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(TtsError::Io(format!("stdout: {e}"))),
        }
    }
}

#[cfg(target_os = "linux")]
fn prepend_lib_path(cmd: &mut Command, runtime: &std::path::Path) {
    if let Some(dir) = runtime.parent() {
        let mut val = dir.to_string_lossy().into_owned();
        if let Ok(old) = std::env::var("LD_LIBRARY_PATH") {
            if !old.is_empty() {
                val.push(':');
                val.push_str(&old);
            }
        }
        cmd.env("LD_LIBRARY_PATH", val);
    }
}

#[cfg(target_os = "linux")]
fn maybe_affinity(cmd: &mut Command) {
    if let Some(cpus) = std::env::var("GALION_PIPER_CPUS")
        .ok()
        .and_then(|v| parse_cpus(&v))
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            cmd.pre_exec(move || set_affinity(&cpus));
        }
    }
}

/// `"6-7"`, `"7"`, `"0,2-3"` -> cpu list. `None` on any parse error.
/// Affinity is applied on Linux. Tests still parse the list on every target.
#[cfg(any(target_os = "linux", test))]
pub(crate) fn parse_cpus(s: &str) -> Option<Vec<usize>> {
    let mut v = Vec::new();
    for part in s.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let (lo, hi) = part.split_once('-').unwrap_or((part, part));
        let (lo, hi): (usize, usize) = (lo.trim().parse().ok()?, hi.trim().parse().ok()?);
        if hi < lo || hi >= 1024 {
            return None;
        }
        v.extend(lo..=hi);
    }
    if v.is_empty() {
        None
    } else {
        Some(v)
    }
}

#[cfg(target_os = "linux")]
fn set_affinity(cpus: &[usize]) -> std::io::Result<()> {
    let mut mask = [0u64; 16];
    for &c in cpus {
        mask[c / 64] |= 1u64 << (c % 64);
    }
    let rc = unsafe { sched_setaffinity(0, std::mem::size_of_val(&mask), mask.as_ptr()) };
    if rc != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn set_nonblock(fd: i32) -> std::io::Result<()> {
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

#[cfg(target_os = "linux")]
use std::os::unix::io::AsRawFd;

#[cfg(target_os = "linux")]
#[derive(Clone, Copy)]
#[repr(C)]
struct PollFd {
    fd: i32,
    events: i16,
    revents: i16,
}

#[cfg(target_os = "linux")]
const POLLIN: i16 = 0x001;
#[cfg(target_os = "linux")]
const POLLHUP: i16 = 0x010;
#[cfg(target_os = "linux")]
const F_GETFL: i32 = 3;
#[cfg(target_os = "linux")]
const F_SETFL: i32 = 4;
#[cfg(target_os = "linux")]
const O_NONBLOCK: i32 = 0o4000;

#[cfg(target_os = "linux")]
extern "C" {
    fn poll(fds: *mut PollFd, nfds: u64, timeout: i32) -> i32;
    fn fcntl(fd: i32, cmd: i32, arg: i32) -> i32;
    fn sched_setaffinity(pid: i32, cpusetsize: usize, mask: *const u64) -> i32;
}

struct Peek {
    #[cfg(target_os = "linux")]
    fd: i32,
    #[cfg(windows)]
    handle: *mut std::ffi::c_void,
}

// SAFETY: the duplicated pipe HANDLE is owned by this Peek only and is used
// from one thread at a time; Windows HANDLEs may be moved between threads.
#[cfg(windows)]
unsafe impl Send for Peek {}

impl Peek {
    fn from_stdout(stdout: &std::process::ChildStdout) -> Result<Self, TtsError> {
        #[cfg(target_os = "linux")]
        {
            let fd = unsafe { dup(stdout.as_raw_fd()) };
            if fd < 0 {
                return Err(TtsError::Io(format!(
                    "dup stdout: {}",
                    std::io::Error::last_os_error()
                )));
            }
            Ok(Self { fd })
        }
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            let src = stdout.as_raw_handle() as *mut std::ffi::c_void;
            let mut duped: *mut std::ffi::c_void = std::ptr::null_mut();
            let ok = unsafe {
                DuplicateHandle(
                    GetCurrentProcess(),
                    src,
                    GetCurrentProcess(),
                    &mut duped,
                    0,
                    0,
                    0x2,
                )
            };
            if ok == 0 {
                return Err(TtsError::Io(format!(
                    "DuplicateHandle: {}",
                    std::io::Error::last_os_error()
                )));
            }
            Ok(Self { handle: duped })
        }
        #[cfg(not(any(target_os = "linux", windows)))]
        {
            let _ = stdout;
            Err(TtsError::Io(
                "Piper session IO is implemented for Linux and Windows".into(),
            ))
        }
    }

    fn pending(&self) -> usize {
        #[cfg(target_os = "linux")]
        {
            let mut n: i32 = 0;
            // FIONREAD. Bytes still in the pipe, not yet read by the stdout thread.
            let rc = unsafe { ioctl(self.fd, 0x541B, &mut n) };
            if rc < 0 || n < 0 {
                0
            } else {
                n as usize
            }
        }
        #[cfg(windows)]
        {
            let mut avail = 0u32;
            let ok = unsafe {
                PeekNamedPipe(
                    self.handle,
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null_mut(),
                    &mut avail,
                    std::ptr::null_mut(),
                )
            };
            if ok == 0 {
                0
            } else {
                avail as usize
            }
        }
        #[cfg(not(any(target_os = "linux", windows)))]
        {
            0
        }
    }
}

#[cfg(windows)]
extern "system" {
    fn GetCurrentProcess() -> *mut std::ffi::c_void;
    fn DuplicateHandle(
        src_proc: *mut std::ffi::c_void,
        src: *mut std::ffi::c_void,
        tgt_proc: *mut std::ffi::c_void,
        tgt: *mut *mut std::ffi::c_void,
        access: u32,
        inherit: i32,
        options: u32,
    ) -> i32;
    fn PeekNamedPipe(
        handle: *mut std::ffi::c_void,
        buf: *mut u8,
        buf_size: u32,
        bytes_read: *mut u32,
        avail: *mut u32,
        left: *mut u32,
    ) -> i32;
    fn CloseHandle(handle: *mut std::ffi::c_void) -> i32;
}

#[cfg(target_os = "linux")]
extern "C" {
    fn dup(fd: i32) -> i32;
    fn close(fd: i32) -> i32;
    fn ioctl(fd: i32, req: u64, n: *mut i32) -> i32;
}

impl Drop for Peek {
    fn drop(&mut self) {
        #[cfg(target_os = "linux")]
        if self.fd >= 0 {
            unsafe { close(self.fd) };
            self.fd = -1;
        }
        #[cfg(windows)]
        if !self.handle.is_null() {
            unsafe { CloseHandle(self.handle) };
            self.handle = std::ptr::null_mut();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_lists_parse() {
        assert_eq!(parse_cpus("6-7"), Some(vec![6, 7]));
        assert_eq!(parse_cpus("0,2-3"), Some(vec![0, 2, 3]));
        assert_eq!(parse_cpus("7"), Some(vec![7]));
        assert_eq!(parse_cpus("3-1"), None);
        assert_eq!(parse_cpus("x"), None);
        assert_eq!(parse_cpus(""), None);
    }
}
