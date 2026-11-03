// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! V2-S14 out-of-process clone worker. Text in, PCM out.
//!
//! One child per [`CloneWorker`], the same shape as the warm Piper worker.
//! Stdin is one JSON line. Stdout is raw s16le at 24 kHz. Stderr
//! `galion-clone-utt:` frames the utterance. PCM stays in RAM.
//!
//! Nano is the primary. Turbo is the option. Piper is the fallback in
//! [`super::SpeechStack`] when this slot is absent or the child dies.
//! Weights are not vendored. [`weights_ready`] is false until the pinned
//! safetensors are on disk. Nothing here downloads them.
//!
//! The Python stub does not call Chatterbox generate and refuses a reference
//! clip. `GALION_CLONE_STUB_PCM=1` is protocol PCM, not a Flash result.
#![allow(unsafe_code)]

use super::sha256;
use crate::{Audio, LanguageId, TtsEngine, VoiceError};
use std::io::{Read, Write};
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

/// Output rate. Matches the phone lane.
pub const CLONE_RATE_HZ: u32 = 24_000;

const PINS_TEXT: &str = include_str!("../../../../scripts/clone/PINS.sha256");

/// Which Chatterbox pack the child was asked for. Nano is the default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloneModel {
    Nano,
    Turbo,
}

impl CloneModel {
    pub fn as_str(self) -> &'static str {
        match self {
            CloneModel::Nano => "nano",
            CloneModel::Turbo => "turbo",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "nano" => Some(CloneModel::Nano),
            "turbo" => Some(CloneModel::Turbo),
            _ => None,
        }
    }

    fn t3_rel(self) -> &'static str {
        match self {
            CloneModel::Nano => "nano/t3_nano_v1.safetensors",
            CloneModel::Turbo => "turbo/t3_turbo_v1.safetensors",
        }
    }
}

/// `samantha` keeps tags. `jarvis` does not. Anything else is Samantha.
pub fn persona_name(s: &str) -> &'static str {
    match s.trim().to_ascii_lowercase().as_str() {
        "jarvis" => "jarvis",
        _ => "samantha",
    }
}

pub fn tags_allowed(persona: &str) -> bool {
    persona_name(persona) == "samantha"
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Pin {
    rel: String,
    sha: String,
    size: u64,
}

fn pins() -> Vec<Pin> {
    let mut out = Vec::new();
    for line in PINS_TEXT.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let Some(rel) = parts.next() else { continue };
        let Some(sha) = parts.next() else { continue };
        let Some(size) = parts.next().and_then(|s| s.parse().ok()) else {
            continue;
        };
        out.push(Pin {
            rel: rel.to_string(),
            sha: sha.to_string(),
            size,
        });
    }
    out
}

fn clone_root() -> PathBuf {
    if let Ok(p) = std::env::var("GALION_CLONE_WEIGHTS") {
        if !p.trim().is_empty() {
            return PathBuf::from(p);
        }
    }
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../weights/clone");
    if repo.is_dir() {
        repo
    } else {
        PathBuf::from("weights/clone")
    }
}

fn pin_matches(root: &Path, pin: &Pin) -> bool {
    let path = root.join(&pin.rel);
    let Ok(meta) = std::fs::metadata(&path) else {
        return false;
    };
    if meta.len() != pin.size {
        return false;
    }
    sha256::file_hex(&path).ok().as_deref() == Some(pin.sha.as_str())
}

/// True when the shared tensors and this model's T3 file match [`PINS_TEXT`].
/// Missing files return false without hashing. This does not download.
pub fn weights_ready(model: CloneModel) -> bool {
    let table = pins();
    if table.is_empty() {
        return false;
    }
    let root = clone_root();
    let need = [
        "shared/s3gen.safetensors",
        "shared/s3gen_meanflow.safetensors",
        "shared/ve.safetensors",
        model.t3_rel(),
    ];
    need.iter().all(|rel| {
        table
            .iter()
            .any(|pin| pin.rel == *rel && pin_matches(&root, pin))
    })
}

pub fn worker_script() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("GALION_CLONE_WORKER") {
        if !p.trim().is_empty() {
            let path = PathBuf::from(p);
            return path.is_file().then_some(path);
        }
    }
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/clone/chatterbox_worker.py");
    path.is_file().then_some(path)
}

/// One warm clone process. Drop kills it. Barge-in must not.
pub struct CloneWorker {
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
    model: CloneModel,
    persona: &'static str,
    engine: String,
}

impl CloneWorker {
    pub fn spawn(script: &Path, model: CloneModel, persona: &str) -> Result<Self, VoiceError> {
        let persona = persona_name(persona);
        let mut child = Command::new("python3")
            .arg(script)
            .env("GALION_CLONE_MODEL", model.as_str())
            .env("GALION_CLONE_PERSONA", persona)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                VoiceError::Backend(format!("clone worker spawn {}: {e}", script.display()))
            })?;
        let pid = child.id();
        let stdin = child.stdin.take();
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let (Some(stdin), Some(stdout), Some(stderr)) = (stdin, stdout, stderr) else {
            let _ = child.kill();
            let _ = child.wait();
            return Err(VoiceError::Backend("clone worker pipes missing".into()));
        };
        if let Err(e) =
            set_nonblock(stdout.as_raw_fd()).and_then(|_| set_nonblock(stderr.as_raw_fd()))
        {
            let _ = child.kill();
            let _ = child.wait();
            return Err(VoiceError::Backend(format!("clone worker nonblock: {e}")));
        }
        Ok(Self {
            child: Some(child),
            stdin: Some(stdin),
            stdout: Some(stdout),
            stderr: Some(stderr),
            err_log: String::new(),
            pcm_buf: Vec::new(),
            pid,
            calls: 0,
            cold_ms: None,
            warm_ms: None,
            model,
            persona,
            engine: String::new(),
        })
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }
    pub fn calls(&self) -> u32 {
        self.calls
    }
    pub fn cold_ms(&self) -> Option<u32> {
        self.cold_ms
    }
    pub fn warm_ms(&self) -> Option<u32> {
        self.warm_ms
    }
    pub fn model(&self) -> CloneModel {
        self.model
    }
    pub fn persona(&self) -> &'static str {
        self.persona
    }
    pub fn engine(&self) -> &str {
        &self.engine
    }

    pub fn is_alive(&mut self) -> bool {
        match self.child.as_mut().map(|c| c.try_wait()) {
            Some(Ok(None)) => true,
            _ => false,
        }
    }

    /// One clause in, one PCM buffer out. Same process on every call.
    pub fn synthesize(&mut self, text: &str) -> Result<Vec<i16>, VoiceError> {
        let t0 = Instant::now();
        let line = request_line(text, self.model, self.persona);
        {
            let stdin = self
                .stdin
                .as_mut()
                .ok_or_else(|| VoiceError::Backend("clone worker stdin closed".into()))?;
            stdin
                .write_all(line.as_bytes())
                .map_err(|e| VoiceError::Backend(format!("clone worker stdin: {e}")))?;
            stdin
                .flush()
                .map_err(|e| VoiceError::Backend(format!("clone worker stdin flush: {e}")))?;
        }
        self.pcm_buf.clear();
        let pcm = self.read_utterance(Duration::from_secs(60))?;
        let ms = elapsed_ms(t0);
        self.calls += 1;
        if self.calls == 1 {
            self.cold_ms = Some(ms);
        } else if self.calls == 2 {
            self.warm_ms = Some(ms);
        }
        if self.err_log.len() > 8192 {
            let mut cut = self.err_log.len() - 2048;
            while cut < self.err_log.len() && !self.err_log.is_char_boundary(cut) {
                cut += 1;
            }
            self.err_log.drain(..cut);
        }
        Ok(pcm)
    }

    fn read_utterance(&mut self, timeout: Duration) -> Result<Vec<i16>, VoiceError> {
        let deadline = Instant::now() + timeout;
        let mark = self.err_log.len();
        loop {
            if Instant::now() >= deadline {
                return Err(VoiceError::Backend(format!(
                    "clone worker timed out ({})",
                    tail(&self.err_log)
                )));
            }
            let remain = deadline.saturating_duration_since(Instant::now());
            let wait_ms = remain.as_millis().min(i32::MAX as u128) as i32;
            let stdout_fd = self.stdout.as_ref().unwrap().as_raw_fd();
            let stderr_fd = self.stderr.as_ref().unwrap().as_raw_fd();
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
            // SAFETY: fds points at two live pipe fds owned by this worker; nfds is 2.
            let rc = unsafe { poll(fds.as_mut_ptr(), 2, wait_ms) };
            if rc < 0 {
                let err = std::io::Error::last_os_error();
                if err.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(VoiceError::Backend(format!("clone worker poll: {err}")));
            }
            let mut progress = false;
            if rc > 0 {
                if fds[0].revents & (POLLIN | POLLHUP) != 0 {
                    self.drain(Pipe::Out)?;
                    progress = true;
                }
                if fds[1].revents & (POLLIN | POLLHUP) != 0 {
                    self.drain(Pipe::Err)?;
                    progress = true;
                }
            }
            if let Some(engine) = line_value(&self.err_log[mark..], "galion-clone-engine:") {
                self.engine = engine;
            }
            if self.err_log[mark..].contains("galion-clone-utt:") {
                if self.stdout.as_ref().is_some() {
                    let _ = self.drain(Pipe::Out);
                }
                break;
            }
            if self.err_log[mark..].contains("galion-clone-refuse:") {
                return Err(VoiceError::Backend(format!(
                    "clone worker refused ({})",
                    tail(&self.err_log)
                )));
            }
            let exited = matches!(self.child.as_mut().map(|c| c.try_wait()), Some(Ok(Some(_))));
            if exited {
                let _ = self.drain(Pipe::Out);
                let _ = self.drain(Pipe::Err);
                if self.err_log[mark..].contains("galion-clone-utt:") {
                    break;
                }
                return Err(VoiceError::Backend(format!(
                    "clone worker exited ({})",
                    tail(&self.err_log)
                )));
            }
            if !progress {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        let bytes = std::mem::take(&mut self.pcm_buf);
        if bytes.len() < 2 || self.err_log[mark..].contains("galion-clone-unwired") {
            return Err(VoiceError::Backend(format!(
                "clone worker returned no PCM ({})",
                tail(&self.err_log)
            )));
        }
        Ok(bytes
            .chunks_exact(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]]))
            .collect())
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
                Err(e) => return Err(VoiceError::Backend(format!("clone worker read: {e}"))),
            }
        }
    }
}

impl Drop for CloneWorker {
    fn drop(&mut self) {
        self.stdin.take();
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Speech-stack slot. Constructible only when pinned weights and the script exist.
/// The process starts on the first synthesis.
pub struct CloneTts {
    script: PathBuf,
    model: CloneModel,
    persona: &'static str,
    worker: Option<CloneWorker>,
    spawns: u32,
    /// Worker PCM for the utterance in progress. Landed once, as one packet.
    pending: Vec<i16>,
    dir: PathBuf,
}

impl CloneTts {
    pub fn try_new() -> Result<Self, String> {
        let model = std::env::var("GALION_CLONE_MODEL")
            .ok()
            .as_deref()
            .and_then(CloneModel::parse)
            .unwrap_or(CloneModel::Nano);
        let persona = persona_name(&std::env::var("GALION_CLONE_PERSONA").unwrap_or_default());
        let script = worker_script().ok_or("clone worker script missing")?;
        if !weights_ready(model) {
            return Err(format!("clone weights absent for {} (scripts/clone/fetch-clone-weights.sh; GALION_CLONE_FETCH=1)", model.as_str()));
        }
        Ok(Self {
            script,
            model,
            persona,
            worker: None,
            spawns: 0,
            pending: Vec::new(),
            dir: super::gqvoice::gqvoice_dir(),
        })
    }

    #[cfg(test)]
    pub fn for_test(worker: CloneWorker, dir: PathBuf) -> Self {
        Self {
            script: PathBuf::from("test"),
            model: worker.model(),
            persona: worker.persona(),
            worker: Some(worker),
            spawns: 1,
            pending: Vec::new(),
            dir,
        }
    }

    pub(crate) fn clear_pending(&mut self) {
        self.pending.clear();
    }

    /// Compile the utterance and return samples read back from the landed file.
    pub(crate) fn land_pending(&mut self) -> Result<Option<crate::Audio>, VoiceError> {
        let pcm = std::mem::take(&mut self.pending);
        if pcm.is_empty() {
            return Ok(None);
        }
        let audio = super::gqvoice::handoff_at(&pcm, self.persona, self.model.as_str(), &self.dir)?;
        Ok(Some(audio))
    }

    pub fn model_name(&self) -> &'static str {
        self.model.as_str()
    }
    pub fn persona(&self) -> &'static str {
        self.persona
    }
    pub fn clone_pid(&self) -> Option<u32> {
        self.worker.as_ref().map(CloneWorker::pid)
    }
    pub fn clone_spawns(&self) -> u32 {
        self.spawns
    }

    fn ensure(&mut self) -> Result<(), VoiceError> {
        let alive = self.worker.as_mut().is_some_and(CloneWorker::is_alive);
        if alive {
            return Ok(());
        }
        let worker = CloneWorker::spawn(&self.script, self.model, self.persona)?;
        self.worker = Some(worker);
        self.spawns += 1;
        Ok(())
    }
}

impl TtsEngine for CloneTts {
    fn languages(&self) -> &[LanguageId] {
        LanguageId::ALL
    }
    fn synthesize(&mut self, text: &str, lang: LanguageId) -> Result<Audio, VoiceError> {
        let _ = lang;
        let (_, body) = crate::split_lang_token(text);
        let body = body.trim().replace(['\n', '\r'], " ");
        if body.is_empty() {
            return Err(VoiceError::EmptyInput);
        }
        self.ensure()?;
        let pcm = self.worker.as_mut().unwrap().synthesize(&body)?;
        let accepted = super::gqvoice::accept_worker_pcm(&pcm, self.persona, self.model.as_str())?;
        self.pending.extend_from_slice(&accepted.playable);
        Ok(accepted.audio)
    }
}

/// CPU time-to-first-audio report. Absent reports omit `ttfa_ms`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlashReport {
    pub measured: bool,
    pub product_claim: bool,
    pub engine: Option<String>,
    pub ttfa_ms: Option<u32>,
    pub note: String,
}

impl FlashReport {
    pub fn absent(note: impl Into<String>) -> Self {
        Self {
            measured: false,
            product_claim: false,
            engine: None,
            ttfa_ms: None,
            note: note.into(),
        }
    }

    /// A number from a run. `product_claim` is true only for engine `model`.
    /// This tree does not produce that engine: generate is not wired.
    pub fn from_pcm(engine: &str, ttfa_ms: u32) -> Self {
        let model = engine == "model";
        let note = if model {
            "CPU time to first PCM. Record it in LEDGER before calling it a Flash result."
        } else {
            "protocol-stub clock only. Not a Flash result and not a product claim."
        };
        Self {
            measured: true,
            product_claim: model,
            engine: Some(engine.to_string()),
            ttfa_ms: Some(ttfa_ms),
            note: note.to_string(),
        }
    }

    pub fn json(&self) -> String {
        let engine = match &self.engine {
            None => "null".to_string(),
            Some(s) => format!("\"{}\"", escape(s)),
        };
        let mut s = format!(
            "{{\"measured\":{},\"product_claim\":{},\"engine\":{},\"note\":\"{}\"",
            if self.measured { "true" } else { "false" },
            if self.product_claim { "true" } else { "false" },
            engine,
            escape(&self.note),
        );
        if let Some(ms) = self.ttfa_ms {
            s.push_str(&format!(",\"ttfa_ms\":{ms}"));
        }
        s.push('}');
        s
    }
}

/// Default harness path. No number unless `GALION_FLASH_EVAL=1` and a worker
/// returns PCM. Does not download and does not send a reference clip.
pub fn flash_from_env() -> FlashReport {
    if std::env::var("GALION_FLASH_EVAL").ok().as_deref() != Some("1") {
        return FlashReport::absent("GALION_FLASH_EVAL is not 1; no TTFA number");
    }
    let model = std::env::var("GALION_CLONE_MODEL")
        .ok()
        .as_deref()
        .and_then(CloneModel::parse)
        .unwrap_or(CloneModel::Nano);
    let stub = std::env::var("GALION_CLONE_STUB_PCM").ok().as_deref() == Some("1");
    if !stub && !weights_ready(model) {
        return FlashReport::absent(
            "clone weights absent and the protocol stub is not armed; no TTFA number",
        );
    }
    let Some(script) = worker_script() else {
        return FlashReport::absent("clone worker script missing; no TTFA number");
    };
    let persona = persona_name(&std::env::var("GALION_CLONE_PERSONA").unwrap_or_default());
    let mut worker = match CloneWorker::spawn(&script, model, persona) {
        Ok(w) => w,
        Err(e) => {
            return FlashReport::absent(format!("clone worker did not start ({e}); no TTFA number"))
        }
    };
    let t0 = Instant::now();
    match worker.synthesize("Hi.") {
        Ok(pcm) if !pcm.is_empty() && worker.engine() != "unwired" => {
            FlashReport::from_pcm(worker.engine(), elapsed_ms(t0))
        }
        Ok(_) => FlashReport::absent("generate is not wired; no TTFA number"),
        Err(e) => FlashReport::absent(format!("worker returned no PCM ({e}); no TTFA number")),
    }
}

fn request_line(text: &str, model: CloneModel, persona: &str) -> String {
    let tags = if tags_allowed(persona) {
        "true"
    } else {
        "false"
    };
    let mut s = String::with_capacity(text.len() + 64);
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
    s.push_str("\",\"model\":\"");
    s.push_str(model.as_str());
    s.push_str("\",\"persona\":\"");
    s.push_str(persona_name(persona));
    s.push_str("\",\"tags\":");
    s.push_str(tags);
    s.push_str("}\n");
    s
}

fn line_value(log: &str, key: &str) -> Option<String> {
    log.lines().rev().find_map(|line| {
        let rest = line.trim().strip_prefix(key)?.trim();
        if rest.is_empty() {
            None
        } else {
            Some(rest.to_string())
        }
    })
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn tail(s: &str) -> String {
    let t = s.trim();
    if t.len() <= 300 {
        t.to_string()
    } else {
        t[t.len() - 300..].to_string()
    }
}

fn elapsed_ms(t0: Instant) -> u32 {
    t0.elapsed().as_millis().min(u32::MAX as u128) as u32
}

#[derive(Clone, Copy)]
enum Pipe {
    Out,
    Err,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::process::{Command, Stdio};

    #[test]
    fn pins_are_safetensors_and_weights_are_not_in_the_tree() {
        let table = pins();
        assert!(table.len() >= 5, "{table:?}");
        for pin in &table {
            assert!(pin.rel.ends_with(".safetensors"), "{}", pin.rel);
            assert!(
                !pin.rel.contains("wav") && !pin.rel.ends_with(".pt"),
                "{}",
                pin.rel
            );
            assert_eq!(pin.sha.len(), 64, "{}", pin.rel);
        }
        assert!(table.iter().any(|p| p.rel == "nano/t3_nano_v1.safetensors"));
        assert!(table
            .iter()
            .any(|p| p.rel == "turbo/t3_turbo_v1.safetensors"));
        let nano = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../weights/clone/nano/t3_nano_v1.safetensors");
        assert!(!nano.exists(), "clone weights must not be committed");
        if std::env::var_os("GALION_CLONE_WEIGHTS").is_none() {
            assert!(!weights_ready(CloneModel::Nano));
            assert!(!weights_ready(CloneModel::Turbo));
        }
    }

    #[test]
    fn request_line_has_no_reference_and_jarvis_tags_are_off() {
        let line = request_line("say \"hi\"", CloneModel::Nano, "samantha");
        assert!(line.contains("\"model\":\"nano\""));
        assert!(line.contains("\"tags\":true"));
        assert!(!line.contains("reference"));
        assert!(!line.contains("audio_prompt"));
        let jarvis = request_line("hello", CloneModel::Turbo, "jarvis");
        assert!(jarvis.contains("\"model\":\"turbo\""));
        assert!(jarvis.contains("\"tags\":false"));
        assert!(!tags_allowed("jarvis"));
        assert_eq!(persona_name("other"), "samantha");
    }

    #[test]
    fn absent_flash_report_has_no_number() {
        let report = FlashReport::absent("not run");
        let json = report.json();
        assert!(json.contains("\"measured\":false"));
        assert!(json.contains("\"product_claim\":false"));
        assert!(!json.contains("ttfa"));
        let stub = FlashReport::from_pcm("protocol-stub", 12);
        assert!(stub.json().contains("\"ttfa_ms\":12"));
        assert!(!stub.product_claim);
        assert!(!FlashReport::from_pcm("model", 4).note.is_empty());
        assert!(FlashReport::from_pcm("model", 4).product_claim);
    }

    #[test]
    fn synthesize_compiles_before_it_returns_audio() {
        let src = include_str!("clone_worker.rs");
        let head = src.split("mod tests").next().expect("impl");
        assert!(
            head.contains("gqvoice::accept_worker_pcm"),
            "worker pcm must enter the compiler"
        );
        assert!(
            head.contains("gqvoice::handoff_at"),
            "the utterance must be landed before playback"
        );
        assert!(
            !head.contains("32768.0"),
            "do not hand worker pcm to the stack in place"
        );
    }

    #[test]
    fn fetch_script_refuses_without_the_env() {
        let script = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../scripts/clone/fetch-clone-weights.sh");
        let out = Command::new("bash")
            .arg(&script)
            .env_remove("GALION_CLONE_FETCH")
            .output()
            .expect("run fetch script");
        assert_eq!(
            out.status.code(),
            Some(2),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(err.contains("GALION_CLONE_FETCH"), "{err}");
        assert!(!err.to_ascii_lowercase().contains("http"));
    }

    #[test]
    fn real_stub_refuses_a_reference_and_speaks_protocol_pcm() {
        let script = worker_script().expect("worker script");
        let mut child = Command::new("python3")
            .arg(&script)
            .env("GALION_CLONE_STUB_PCM", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        {
            let mut stdin = child.stdin.take().unwrap();
            stdin
                .write_all(b"{\"text\":\"hi\",\"reference\":\"clip.wav\"}\n")
                .unwrap();
        }
        let out = child.wait_with_output().unwrap();
        assert_eq!(
            out.status.code(),
            Some(3),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(err.contains("rights hold"), "{err}");
        assert!(out.stdout.is_empty(), "a refused clip must not produce PCM");

        let absent = Command::new("python3")
            .arg(&script)
            .env_remove("GALION_CLONE_STUB_PCM")
            .env_remove("GALION_CLONE_WEIGHTS")
            .output()
            .unwrap();
        assert_eq!(absent.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&absent.stderr).contains("galion-clone-absent"));
    }

    #[test]
    fn two_turns_reuse_one_clone_process_and_the_clock_is_not_a_product_claim() {
        let dir = std::env::temp_dir().join(format!("galion-clone-worker-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("fake-clone.py");
        std::fs::write(&script, FAKE_CLONE).unwrap();
        let mut perm = std::fs::metadata(&script).unwrap().permissions();
        perm.set_mode(0o755);
        std::fs::set_permissions(&script, perm).unwrap();
        let before = entries(&dir);
        let mut w = CloneWorker::spawn(&script, CloneModel::Nano, "jarvis").unwrap();
        let pid = w.pid();
        let a = w.synthesize("turn one [laugh]").unwrap();
        let b = w.synthesize("turn two says hi").unwrap();
        assert_eq!(w.pid(), pid);
        assert!(w.is_alive());
        assert_eq!(w.calls(), 2);
        assert_eq!(w.engine(), "protocol-stub");
        assert!(w.cold_ms().is_some() && w.warm_ms().is_some());
        assert_ne!(a[0], b[0]);
        let report = FlashReport::from_pcm(w.engine(), w.warm_ms().unwrap());
        assert!(!report.product_claim);
        assert!(report.json().contains("protocol-stub"));
        assert_eq!(
            before,
            entries(&dir),
            "the worker must not cache PCM on disk"
        );
        drop(w);
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn entries(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
    }

    const FAKE_CLONE: &str = r#"#!/usr/bin/env python3
import json, struct, sys
while True:
    line = sys.stdin.buffer.readline()
    if not line:
        break
    msg = json.loads(line.decode())
    if msg.get("reference") or msg.get("audio_prompt") or msg.get("audio_prompt_path"):
        sys.stderr.write("galion-clone-refuse: reference clips are on the rights hold\n")
        sys.stderr.flush()
        sys.exit(3)
    text = msg.get("text", "")
    if msg.get("persona") == "jarvis":
        text = text.replace("[laugh]", "").replace("[chuckle]", "")
    n = max(1, min(len(text), 32767))
    pcm = struct.pack("<h", n) + struct.pack("<200h", *([1000 + (n % 50)] * 200))
    sys.stdout.buffer.write(pcm)
    sys.stdout.buffer.flush()
    sys.stderr.write("galion-clone-engine: protocol-stub\n")
    sys.stderr.write("galion-clone-utt: %d\n" % len(pcm))
    sys.stderr.flush()
"#;
}
