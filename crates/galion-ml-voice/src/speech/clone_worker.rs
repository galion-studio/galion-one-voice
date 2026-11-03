// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! V2-S14 out-of-process clone worker. Text in, PCM out.
//!
//! One child per [`CloneWorker`], the same shape as the warm Piper worker.
//! Stdin is one JSON line. Stdout is raw s16le at 24 kHz. Stderr
//! `galion-clone-utt:` frames the utterance. PCM stays in RAM.
//!
//! That stdout is clone output, not the symphony runtime file. Compile it
//! with `galion-voice-compile` into `voices/compiled/<persona>.gq`
//! ([`crate::artifact`]). Chatterbox safetensors stay this worker's weight
//! input. The heartbeat does not play them.
//!
//! Nano is the primary. Turbo is the option. Piper is the fallback in
//! [`super::SpeechStack`] when this slot is absent or the child dies.
//! Weights are not vendored. [`weights_ready`] is false until the pinned
//! safetensors are on disk. Nothing here downloads them.
//!
//! The child is CPU-only Chatterbox Nano (Turbo remains the quality option,
//! not a swap). Its CPU placement is a [`ClonePlan`]: `session` is one thread
//! (the ~1 vCPU multi-tenant voice-session budget); on a dedicated host with
//! 8+ usable cores the two personas get disjoint cores and matching thread
//! pools (`split4`: Samantha 0-3, Jarvis 4-7, 4 threads each; `split3`:
//! 0-2 / 3-5, 3 threads, 6-7 left to ASR, Piper and the loop). The pin is a
//! `sched_setaffinity` in the child before exec, not a taskset wrapper.
//! No GPU. No GGUF. PyTorch stays in that process and is not linked here.
//!
//! A reference is attached only for the owner-cleared Samantha and Jarvis
//! wavs (t273) when the file is present and its sha256 matches
//! `scripts/clone/REFS.sha256`. Missing files are omitted (CI skip). A
//! mismatch, an MP3, or a denoise request is an error. Jarvis hub texture
//! is character and is not cleaned. The stub still does not call generate.
//! `GALION_CLONE_STUB_PCM=1` is protocol PCM, not a Flash result.
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
const REFS_TEXT: &str = include_str!("../../../../scripts/clone/REFS.sha256");

/// Plan `session`: this child asks for one hardware thread. That is the
/// voice-session budget (~1 vCPU). The cascade burst of ~4 is not this
/// process. Dedicated hosts use a split [`ClonePlan`] instead.
pub const CLONE_SESSION_VCPU: u32 = 1;
pub const CLONE_BURST_VCPU: u32 = 4;

const CPU_THREAD_KEYS: &[&str] = &[
    "OMP_NUM_THREADS",
    "OPENBLAS_NUM_THREADS",
    "MKL_NUM_THREADS",
    "NUMEXPR_NUM_THREADS",
    "VECLIB_MAXIMUM_THREADS",
    "TORCH_NUM_THREADS",
    "GALION_CLONE_THREADS",
];

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
    // Columns after size are the Hugging Face repo, the remote path, and the
    // GitHub Release asset. scripts/clone/fetch-clone-weights.sh reads those.
    // This check uses the path, the content sha256, and the byte size.
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

#[derive(Clone, Debug, PartialEq, Eq)]
struct RefPin {
    persona: String,
    sha: String,
    duration_s: String,
    rate_hz: u32,
    channels: u32,
    bits: u32,
    repo_rel: String,
    store_rel: String,
    texture: String,
}

fn ref_pins() -> Vec<RefPin> {
    let mut out = Vec::new();
    for line in REFS_TEXT.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let Some(persona) = parts.next() else { continue };
        let Some(sha) = parts.next() else { continue };
        let Some(duration_s) = parts.next() else { continue };
        let Some(rate_hz) = parts.next().and_then(|s| s.parse().ok()) else { continue };
        let Some(channels) = parts.next().and_then(|s| s.parse().ok()) else { continue };
        let Some(bits) = parts.next().and_then(|s| s.parse().ok()) else { continue };
        let Some(repo_rel) = parts.next() else { continue };
        let Some(store_rel) = parts.next() else { continue };
        let Some(texture) = parts.next() else { continue };
        if !repo_rel.ends_with(".wav") || !store_rel.ends_with(".wav") {
            continue;
        }
        if !texture.starts_with("preserve") {
            continue;
        }
        out.push(RefPin {
            persona: persona.to_string(),
            sha: sha.to_string(),
            duration_s: duration_s.to_string(),
            rate_hz,
            channels,
            bits,
            repo_rel: repo_rel.to_string(),
            store_rel: store_rel.to_string(),
            texture: texture.to_string(),
        });
    }
    out
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// `true` only when the persona TOML both allows a send and is not still
/// on `rights = "hold"`. A missing file means do not send.
fn send_reference_enabled(repo: &Path, persona: &str) -> bool {
    let path = repo.join(format!("docs/galion-voice-v2/personas/voice/{persona}.toml"));
    let Ok(text) = std::fs::read_to_string(path) else {
        return false;
    };
    let mut in_worker = false;
    let mut in_reference = false;
    let mut send = false;
    let mut present = false;
    let mut rights_hold = false;
    let mut denoise = false;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(header) = line.strip_prefix('[') {
            let header = header.trim_end_matches(']');
            in_worker = header == "worker";
            in_reference = header == "reference";
            continue;
        }
        let Some((k, v)) = line.split_once('=') else { continue };
        let v = v.split('#').next().unwrap_or(v).trim().trim_matches('"');
        if in_worker && k.trim() == "send_reference" && v == "true" {
            send = true;
        }
        if in_reference && k.trim() == "present" && v == "true" {
            present = true;
        }
        if in_reference && k.trim() == "rights" && v == "hold" {
            rights_hold = true;
        }
        if in_reference && k.trim() == "denoise" && v == "true" {
            denoise = true;
        }
    }
    send && present && !rights_hold && !denoise
}

/// A consented wav the child may condition on. The bytes are not copied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedRef {
    pub persona: String,
    pub path: PathBuf,
    pub sha256: String,
    pub texture: String,
}

fn clip_store_from(raw: Option<&str>) -> Option<PathBuf> {
    let raw = raw?.trim();
    if raw.is_empty() {
        None
    } else {
        Some(PathBuf::from(raw))
    }
}

fn resolve_reference_at(
    persona: &str,
    store: Option<&Path>,
    repo: &Path,
) -> Result<Option<ResolvedRef>, VoiceError> {
    let persona = persona_name(persona);
    if !send_reference_enabled(repo, persona) {
        return Ok(None);
    }
    let Some(pin) = ref_pins().into_iter().find(|p| p.persona == persona) else {
        return Ok(None);
    };
    let mut candidates = Vec::new();
    if let Some(store) = store {
        candidates.push(store.join(&pin.store_rel));
    }
    candidates.push(repo.join(&pin.repo_rel));
    for path in candidates {
        if !path.is_file() {
            continue;
        }
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        let lower = name.to_ascii_lowercase();
        if !lower.ends_with(".wav") || lower.contains("mp3") {
            return Err(VoiceError::Backend("mp3 is not a clone reference".into()));
        }
        let got = sha256::file_hex(&path)
            .map_err(|e| VoiceError::Backend(format!("reference hash: {e}")))?;
        if got != pin.sha {
            return Err(VoiceError::Backend(format!(
                "reference sha256 mismatch for {persona}"
            )));
        }
        return Ok(Some(ResolvedRef {
            persona: persona.to_string(),
            path,
            sha256: got,
            texture: pin.texture,
        }));
    }
    Ok(None)
}

fn resolve_reference(persona: &str) -> Result<Option<ResolvedRef>, VoiceError> {
    let store = clip_store_from(std::env::var("GALION_CLIP_STORE").ok().as_deref());
    resolve_reference_at(persona, store.as_deref(), &repo_root())
}

/// How the clone children share the host CPUs (latency-240, agent 02).
///
/// Measured on the 8-core live box (latency-240/02-core-pinning.md): two
/// unpinned Nano workers at 8 threads each oversubscribe 16 threads on 8
/// cores and live RTF drifts to ~2x; disjoint cores with matching pools keep
/// each voice near its solo RTF.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClonePlan {
    /// One thread, no pin: the ~1 vCPU voice-session budget (multi-tenant).
    Session,
    /// Samantha cores 0-3, Jarvis cores 4-7, 4 threads each.
    Split4,
    /// Samantha cores 0-2, Jarvis cores 3-5, 3 threads each; 6-7 reserved
    /// for ASR, Piper and the call loop.
    Split3,
    /// V2 fully local: both voices share cores 0-3 at 2 threads each (never
    /// more threads than cores, even when both speak at once); cores 4-7 are
    /// the local LLM's (4 threads). ASR and Piper are not pinned.
    Local,
}

/// Plan `auto` picks on a dedicated host with at least [`DEDICATED_MIN_CPUS`]
/// usable cores. Chosen from the measured matrix in 02-core-pinning.md.
pub const DEDICATED_PLAN: ClonePlan = ClonePlan::Split4;
/// Plan `auto` picks when `GALION_LLM_LOCAL` is set (V2 fully local).
pub const LOCAL_LLM_PLAN: ClonePlan = ClonePlan::Local;
/// The dedicated plans index cores 0-7.
pub const DEDICATED_MIN_CPUS: usize = 8;
/// Local LLM placement under [`ClonePlan::Local`]: cores 4-7, 4 threads.
pub const LOCAL_LLM_THREADS: u32 = 4;

impl ClonePlan {
    pub fn as_str(self) -> &'static str {
        match self {
            ClonePlan::Session => "session",
            ClonePlan::Split4 => "split4",
            ClonePlan::Split3 => "split3",
            ClonePlan::Local => "local",
        }
    }

    /// `session`, `split4`, `split3`, `local`; `auto` (or empty) resolves by
    /// core count (local LLM hosts: see [`ClonePlan::auto`]).
    pub fn parse(s: &str, usable_cpus: usize) -> Option<Self> {
        Self::parse_with(s, usable_cpus, false)
    }

    /// `auto` with the local-LLM flag: [`LOCAL_LLM_PLAN`] on a dedicated host
    /// that also runs the LLM, [`DEDICATED_PLAN`] when the LLM is remote.
    pub fn parse_with(s: &str, usable_cpus: usize, local_llm: bool) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "local" => Some(ClonePlan::Local),
            "" | "auto" if local_llm && usable_cpus >= DEDICATED_MIN_CPUS => Some(LOCAL_LLM_PLAN),
            "session" | "1" => Some(ClonePlan::Session),
            "split4" | "4+4" => Some(ClonePlan::Split4),
            "split3" | "3+3" => Some(ClonePlan::Split3),
            "" | "auto" => Some(if usable_cpus >= DEDICATED_MIN_CPUS {
                DEDICATED_PLAN
            } else {
                ClonePlan::Session
            }),
            _ => None,
        }
    }

    /// Cores this plan leaves for ASR, Piper and the loop.
    pub fn reserved_cpus(self) -> Vec<usize> {
        match self {
            ClonePlan::Split3 => vec![6, 7],
            ClonePlan::Local => vec![4, 5, 6, 7],
            _ => Vec::new(),
        }
    }

    /// Where a local LLM server should run under this plan (cores, threads).
    /// Pin it with [`apply_affinity`] and pass the thread count (`-t`).
    pub fn llm_placement(self) -> Option<(Vec<usize>, u32)> {
        match self {
            ClonePlan::Local => Some(((4..8).collect(), LOCAL_LLM_THREADS)),
            _ => None,
        }
    }
}

/// Where one clone child runs and how many threads its pools get.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClonePlacement {
    pub plan: ClonePlan,
    /// Empty means not pinned (inherit the parent's mask).
    pub cpus: Vec<usize>,
    pub threads: u32,
}

impl ClonePlacement {
    /// The plan's default cores for `persona` (only Samantha and Jarvis exist).
    pub fn for_persona(plan: ClonePlan, persona: &str, usable_cpus: usize) -> Self {
        let jarvis = persona_name(persona) == "jarvis";
        let (cpus, threads) = match plan {
            ClonePlan::Session => (Vec::new(), CLONE_SESSION_VCPU),
            ClonePlan::Split4 => (
                if jarvis {
                    (4..8).collect()
                } else {
                    (0..4).collect()
                },
                4,
            ),
            ClonePlan::Split3 => (
                if jarvis {
                    (3..6).collect()
                } else {
                    (0..3).collect()
                },
                3,
            ),
            ClonePlan::Local => ((0..4).collect(), 2),
        };
        if plan != ClonePlan::Session && usable_cpus < DEDICATED_MIN_CPUS {
            // A split plan on a small host would pin onto cores that are not there.
            return Self::for_persona(ClonePlan::Session, persona, usable_cpus);
        }
        Self {
            plan,
            cpus,
            threads,
        }
    }

    /// `GALION_CLONE_CPU_PLAN` (default auto), then per-persona overrides
    /// `GALION_CLONE_CPUS_<PERSONA>` (`0-3`, `0,2,4`, or `none`) and
    /// `GALION_CLONE_THREADS`. A pinned child never gets more threads than
    /// cores: oversubscription is what this plan exists to stop.
    pub fn from_env(persona: &str) -> Self {
        let n = usable_cpus();
        let get = |k: &str| std::env::var(k).ok();
        let local_llm = get("GALION_LLM_LOCAL").is_some_and(|v| {
            !matches!(v.trim().to_ascii_lowercase().as_str(), "" | "0" | "false" | "no")
        });
        let plan_env = get("GALION_CLONE_CPU_PLAN");
        let plan = ClonePlan::parse_with(plan_env.as_deref().unwrap_or(""), n, local_llm)
            .unwrap_or(ClonePlan::Session);
        let mut p = Self::for_persona(plan, persona, n);
        if p.plan == ClonePlan::Session {
            return p;
        }
        let key = format!(
            "GALION_CLONE_CPUS_{}",
            persona_name(persona).to_ascii_uppercase()
        );
        if let Some(v) = get(&key) {
            if v.trim().eq_ignore_ascii_case("none") {
                p.cpus.clear();
            } else if let Some(c) = parse_cpu_list(&v).filter(|c| c.iter().all(|&i| i < n)) {
                p.threads = p.threads.min(c.len() as u32).max(1);
                p.cpus = c;
            }
        }
        if let Some(t) = get("GALION_CLONE_THREADS").and_then(|t| t.trim().parse::<u32>().ok()) {
            let cap = if p.cpus.is_empty() {
                n as u32
            } else if p.plan == ClonePlan::Local {
                // Both voices share these cores: half each, so two at once fit.
                (p.cpus.len() / 2) as u32
            } else {
                p.cpus.len() as u32
            };
            p.threads = t.clamp(1, cap.max(1));
        }
        p
    }

    /// taskset-style list, e.g. `0-3`; empty when not pinned.
    pub fn cpu_list(&self) -> String {
        format_cpu_list(&self.cpus)
    }
}

/// Usable cores for this process (respects an inherited affinity mask).
pub fn usable_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

/// Parse `0-3`, `0,2,4`, `0-2,6`. None on anything else.
pub fn parse_cpu_list(s: &str) -> Option<Vec<usize>> {
    let mut v = Vec::new();
    for part in s.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        match part.split_once('-') {
            Some((a, b)) => {
                let (a, b) = (
                    a.trim().parse::<usize>().ok()?,
                    b.trim().parse::<usize>().ok()?,
                );
                if a > b || b >= CPU_SET_BITS {
                    return None;
                }
                v.extend(a..=b);
            }
            None => {
                let i = part.parse::<usize>().ok()?;
                if i >= CPU_SET_BITS {
                    return None;
                }
                v.push(i);
            }
        }
    }
    v.sort_unstable();
    v.dedup();
    (!v.is_empty()).then_some(v)
}

fn format_cpu_list(cpus: &[usize]) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < cpus.len() {
        let mut j = i;
        while j + 1 < cpus.len() && cpus[j + 1] == cpus[j] + 1 {
            j += 1;
        }
        out.push(if i == j {
            cpus[i].to_string()
        } else {
            format!("{}-{}", cpus[i], cpus[j])
        });
        i = j + 1;
    }
    out.join(",")
}

const CPU_SET_BITS: usize = 1024;

/// Pin a child to `cpus` with `sched_setaffinity` after fork, before exec.
/// Empty `cpus` leaves the inherited mask. Usable for any worker (Piper, ASR).
pub fn apply_affinity(cmd: &mut Command, cpus: &[usize]) {
    if cpus.is_empty() {
        return;
    }
    let mut mask = [0u64; CPU_SET_BITS / 64];
    for &c in cpus {
        mask[c / 64] |= 1u64 << (c % 64);
    }
    use std::os::unix::process::CommandExt;
    // SAFETY: the closure runs in the forked child before exec and only makes
    // one async-signal-safe syscall on a mask it owns by value.
    unsafe {
        cmd.pre_exec(move || {
            if sched_setaffinity(0, std::mem::size_of_val(&mask), mask.as_ptr()) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

fn apply_cpu_budget(cmd: &mut Command, placement: &ClonePlacement) {
    for key in CPU_THREAD_KEYS {
        cmd.env(key, placement.threads.to_string());
    }
    cmd.env("GALION_CLONE_CPU_PLAN", placement.plan.as_str());
    cmd.env("GALION_CLONE_CPUS", placement.cpu_list());
    apply_affinity(cmd, &placement.cpus);
    cmd.env("GALION_CLONE_DEVICE", "cpu");
    cmd.env("CUDA_VISIBLE_DEVICES", "");
    cmd.env("HIP_VISIBLE_DEVICES", "");
    cmd.env("GALION_CLONE_DENOISE", "0");
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
    /// Spawn with [`ClonePlacement::from_env`] (plan `auto` by default).
    pub fn spawn(script: &Path, model: CloneModel, persona: &str) -> Result<Self, VoiceError> {
        let placement = ClonePlacement::from_env(persona);
        Self::spawn_placed(script, model, persona, &placement)
    }

    pub fn spawn_placed(
        script: &Path,
        model: CloneModel,
        persona: &str,
        placement: &ClonePlacement,
    ) -> Result<Self, VoiceError> {
        let persona = persona_name(persona);
        let mut cmd = Command::new("python3");
        cmd.arg(script)
            .env("GALION_CLONE_MODEL", model.as_str())
            .env("GALION_CLONE_PERSONA", persona)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        apply_cpu_budget(&mut cmd, placement);
        let mut child = cmd.spawn().map_err(|e| {
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
        let resolved = resolve_reference(self.persona)?;
        let line = request_line(
            text,
            self.model,
            self.persona,
            resolved.as_ref(),
            jarvis_texture_enabled(self.persona),
        );
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
/// returns PCM. Does not download. A consented reference is attached only
/// when the pinned wav is present and the hash matches. This is not a model
/// TTFA.
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

fn push_json_str(s: &mut String, text: &str) {
    for c in text.chars() {
        match c {
            '"' => s.push_str("\\\""),
            '\\' => s.push_str("\\\\"),
            '\n' | '\r' | '\t' => s.push(' '),
            c if (c as u32) < 0x20 => s.push(' '),
            c => s.push(c),
        }
    }
}

/// Jarvis hub layer toggle. Unset means on. `0` / `false` / `off` leaves the
/// layer in the clip and does not apply it. Samantha has no hub layer.
pub fn jarvis_texture_enabled(persona: &str) -> bool {
    if persona_name(persona) != "jarvis" {
        return false;
    }
    match std::env::var("GALION_JARVIS_TEXTURE")
        .ok()
        .as_deref()
        .map(str::trim)
    {
        Some("0") | Some("false") | Some("off") => false,
        _ => true,
    }
}

pub(crate) fn ref_sha(persona: &str) -> Option<String> {
    let persona = persona_name(persona);
    ref_pins().into_iter().find(|p| p.persona == persona).map(|p| p.sha)
}

fn request_line(
    text: &str,
    model: CloneModel,
    persona: &str,
    reference: Option<&ResolvedRef>,
    texture_layer: bool,
) -> String {
    let persona = persona_name(persona);
    let texture_layer = texture_layer && persona == "jarvis";
    let tags = if tags_allowed(persona) {
        "true"
    } else {
        "false"
    };
    let mut s = String::with_capacity(text.len() + 96);
    s.push_str("{\"text\":\"");
    push_json_str(&mut s, text);
    s.push_str("\",\"model\":\"");
    s.push_str(model.as_str());
    s.push_str("\",\"persona\":\"");
    s.push_str(persona_name(persona));
    s.push_str("\",\"tags\":");
    s.push_str(tags);
    s.push_str(",\"device\":\"cpu\",\"denoise\":false,\"texture_layer\":");
    s.push_str(if texture_layer { "true" } else { "false" });
    if persona == "jarvis" {
        s.push_str(",\"texture\":\"preserve-hub\"");
    }
    if let Some(reference) = reference {
        let path = reference.path.to_string_lossy();
        s.push_str(",\"audio_prompt_path\":\"");
        push_json_str(&mut s, &path);
        s.push_str("\",\"reference\":\"");
        push_json_str(&mut s, &path);
        if persona != "jarvis" {
            s.push_str("\",\"texture\":\"");
            push_json_str(&mut s, &reference.texture);
            s.push('"');
        }
    }
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
    fn sched_setaffinity(pid: i32, cpusetsize: usize, mask: *const u64) -> i32;
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
        // "Committed" means tracked by git. Weights fetched into the gitignored
        // weights/clone/ by fetch-clone-weights.sh are allowed on disk.
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        if let Ok(out) = Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(["ls-files", "--", "weights/clone"])
            .output()
        {
            if out.status.success() {
                let tracked = String::from_utf8_lossy(&out.stdout);
                assert!(
                    !tracked.lines().any(|l| l.ends_with(".safetensors") || l.ends_with(".pt")),
                    "clone weights must not be committed: {tracked}"
                );
            }
        }
        if std::env::var_os("GALION_CLONE_WEIGHTS").is_none() && !nano.exists() {
            assert!(!weights_ready(CloneModel::Nano));
            assert!(!weights_ready(CloneModel::Turbo));
        }
    }

    #[test]
    fn request_line_omits_a_missing_reference_and_jarvis_tags_are_off() {
        let line = request_line("say \"hi\"", CloneModel::Nano, "samantha", None, true);
        assert!(line.contains("\"model\":\"nano\""));
        assert!(line.contains("\"tags\":true"));
        assert!(line.contains("\"device\":\"cpu\""));
        assert!(line.contains("\"denoise\":false"));
        assert!(!line.contains("reference"));
        assert!(!line.contains("audio_prompt"));
        assert!(!line.contains("gguf"));
        let jarvis = request_line("hello", CloneModel::Turbo, "jarvis", None, false);
        assert!(jarvis.contains("\"model\":\"turbo\""));
        assert!(jarvis.contains("\"tags\":false"));
        assert!(jarvis.contains("\"texture_layer\":false"));
        assert!(jarvis.contains("\"texture\":\"preserve-hub\""));
        assert!(!jarvis.contains("reference"));
        assert!(!tags_allowed("jarvis"));
        assert_eq!(persona_name("other"), "samantha");
        let attach = ResolvedRef {
            persona: "jarvis".into(),
            path: PathBuf::from("docs/galion-voice-v2/personas/refs/jarvis.wav"),
            sha256: "c2200b59fa78b3c98d13ac5dc7db3a6c14e802310dca73033d7770bd4b532c60".into(),
            texture: "preserve-hub".into(),
        };
        let sent = request_line("hello", CloneModel::Nano, "jarvis", Some(&attach), true);
        assert!(sent.contains("\"audio_prompt_path\":\"docs/galion-voice-v2/personas/refs/jarvis.wav\""));
        assert!(sent.contains("\"texture\":\"preserve-hub\""));
        assert!(sent.contains("\"texture_layer\":true"));
        assert_eq!(sent.matches("\"texture\":").count(), 1);
        assert!(sent.contains("\"denoise\":false"));
        assert!(sent.contains("\"tags\":false"));
        assert!(!sent.contains("mp3"));
    }

    #[test]
    fn reference_pins_keep_the_current_wavs_and_jarvis_texture() {
        let table = ref_pins();
        assert_eq!(table.len(), 2, "{table:?}");
        let sam = table.iter().find(|p| p.persona == "samantha").unwrap();
        let jar = table.iter().find(|p| p.persona == "jarvis").unwrap();
        assert_eq!(sam.sha, "bdcb0c2903bb91c7b0fda0cf79a42406fdaa662fd8d31f6eaed3895d9c7a8693");
        assert_eq!(sam.duration_s, "17.41");
        assert_eq!(sam.rate_hz, 44_100);
        assert_eq!(sam.channels, 2);
        assert_eq!(sam.bits, 16);
        assert_eq!(sam.texture, "preserve");
        assert!(sam.repo_rel.ends_with("samantha.wav"));
        assert_eq!(sam.store_rel, "samantha/reference.wav");
        assert_eq!(jar.sha, "c2200b59fa78b3c98d13ac5dc7db3a6c14e802310dca73033d7770bd4b532c60");
        assert_eq!(jar.duration_s, "15.95");
        assert_eq!(jar.rate_hz, 48_000);
        assert_eq!(jar.channels, 2);
        assert_eq!(jar.bits, 16);
        assert_eq!(jar.texture, "preserve-hub");
        assert!(jar.repo_rel.ends_with("jarvis.wav"));
        assert_eq!(jar.store_rel, "jarvis/reference.wav");
        for pin in &table {
            assert!(!pin.repo_rel.contains("mp3") && !pin.store_rel.contains("mp3"));
            assert!(pin.texture.starts_with("preserve"));
        }
        assert_eq!(CLONE_SESSION_VCPU, 1);
        assert_eq!(CLONE_BURST_VCPU, 4);
    }

    #[test]
    fn toml_sends_the_pinned_wav_and_does_not_swap_the_model() {
        let root = repo_root();
        for persona in ["samantha", "jarvis"] {
            let text = std::fs::read_to_string(
                root.join(format!("docs/galion-voice-v2/personas/voice/{persona}.toml")),
            )
            .unwrap();
            let pin = ref_pins().into_iter().find(|p| p.persona == persona).unwrap();
            assert!(text.contains(&format!("clip_path = \"{}\"", pin.repo_rel)), "{persona}");
            assert!(text.contains(&pin.sha), "{persona}");
            assert!(text.contains("present = true"), "{persona}");
            assert!(text.contains("send_reference = true"), "{persona}");
            assert!(text.contains("denoise = false"), "{persona}");
            assert!(text.contains("ingest = false"), "{persona}");
            assert!(text.contains("model = \"nano\""), "{persona}");
            assert!(text.contains("device = \"cpu\""), "{persona}");
            assert!(text.contains("threads = 1"), "{persona}");
            assert!(send_reference_enabled(&root, persona), "{persona}");
        }
        let jarvis = std::fs::read_to_string(
            root.join("docs/galion-voice-v2/personas/voice/jarvis.toml"),
        )
        .unwrap();
        assert!(jarvis.contains("preserve-hub"));
        assert!(jarvis.contains("clicks"));
        assert!(jarvis.contains("GALION_JARVIS_TEXTURE"));
        assert!(jarvis.contains("texture_stripped = false"));
        let worker = std::fs::read_to_string(
            root.join("scripts/clone/chatterbox_worker.py"),
        )
        .unwrap();
        assert!(!worker.contains("import torch"));
        assert!(!worker.contains("import gguf") && !worker.contains(".gguf"));
        assert!(!worker.contains("ffmpeg") && !worker.contains("noisereduce"));
        assert!(worker.contains("CUDA_VISIBLE_DEVICES"));
        assert!(worker.contains("denoise refused"));
    }

    #[test]
    fn absent_reference_is_skipped_and_a_bad_file_is_refused() {
        let root = repo_root();
        let store = clip_store_from(None);
        let got = resolve_reference_at("samantha", store.as_deref(), &root).unwrap();
        if let Some(found) = got {
            assert_eq!(found.sha256, "bdcb0c2903bb91c7b0fda0cf79a42406fdaa662fd8d31f6eaed3895d9c7a8693");
            assert_eq!(found.texture, "preserve");
            assert!(found.path.ends_with("samantha.wav"));
        }
        let dir = std::env::temp_dir().join(format!("galion-clone-ref-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let voice = dir.join("docs/galion-voice-v2/personas/voice");
        let refs = dir.join("docs/galion-voice-v2/personas/refs");
        std::fs::create_dir_all(&voice).unwrap();
        std::fs::create_dir_all(&refs).unwrap();
        std::fs::write(
            voice.join("samantha.toml"),
            "[reference]\nrights = \"owner-t273\"\npresent = true\ndenoise = false\n[worker]\nsend_reference = true\n",
        )
        .unwrap();
        std::fs::write(refs.join("samantha.wav"), b"not-the-clip").unwrap();
        let err = resolve_reference_at("samantha", None, &dir).unwrap_err();
        assert!(err.to_string().contains("sha256 mismatch"), "{err}");
        std::fs::write(
            voice.join("samantha.toml"),
            "[reference]\nrights = \"hold\"\npresent = true\ndenoise = false\n[worker]\nsend_reference = true\n",
        )
        .unwrap();
        assert!(resolve_reference_at("samantha", None, &dir).unwrap().is_none());
        let _ = std::fs::remove_dir_all(&dir);
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
    fn fetch_script_release_pins_stay_local() {
        let script = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../scripts/clone/fetch-clone-weights.test.sh");
        let out = Command::new("bash")
            .arg(&script)
            .env_remove("GALION_CLONE_FETCH")
            .env_remove("GALION_CLONE_PUBLISH")
            .output()
            .expect("run fetch pin tests");
        assert!(
            out.status.success(),
            "stdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
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
        assert!(err.contains("threads=1"), "{err}");
        assert!(err.contains("device=cpu"), "{err}");
        assert!(err.contains("gguf=false"), "{err}");
        assert!(out.stdout.is_empty(), "a refused clip must not produce PCM");

        let absent = Command::new("python3")
            .arg(&script)
            .env_remove("GALION_CLONE_STUB_PCM")
            // An empty weights root: the absent path must hold even on a host
            // that has fetched the real weights into weights/clone/.
            .env("GALION_CLONE_WEIGHTS", std::env::temp_dir().join("galion-clone-weights-empty"))
            .output()
            .unwrap();
        assert_eq!(absent.status.code(), Some(2));
        let absent_err = String::from_utf8_lossy(&absent.stderr);
        assert!(absent_err.contains("galion-clone-absent"), "{absent_err}");
        assert!(absent_err.contains("session_vcpu=1"), "{absent_err}");

        let denoise = Command::new("python3")
            .arg(&script)
            .env("GALION_CLONE_STUB_PCM", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut denoise = denoise;
        {
            let mut stdin = denoise.stdin.take().unwrap();
            stdin
                .write_all(b"{\"text\":\"hi\",\"persona\":\"jarvis\",\"denoise\":true}\n")
                .unwrap();
        }
        let denied = denoise.wait_with_output().unwrap();
        assert_eq!(denied.status.code(), Some(3));
        assert!(denied.stdout.is_empty());
        assert!(String::from_utf8_lossy(&denied.stderr).contains("denoise refused"));

        let gpu = Command::new("python3")
            .arg(&script)
            .env("GALION_CLONE_DEVICE", "cuda")
            .env("GALION_CLONE_STUB_PCM", "1")
            .output()
            .unwrap();
        assert_eq!(gpu.status.code(), Some(3));
        assert!(String::from_utf8_lossy(&gpu.stderr).contains("CPU-only"));
        assert!(gpu.stdout.is_empty());

        let threads = Command::new("python3")
            .arg(&script)
            .env("GALION_CLONE_THREADS", "8")
            .env("GALION_CLONE_STUB_PCM", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut threads = threads;
        {
            let mut stdin = threads.stdin.take().unwrap();
            stdin.write_all(b"{\"text\":\"hi\",\"persona\":\"jarvis\"}\n").unwrap();
        }
        let clamped = threads.wait_with_output().unwrap();
        assert!(clamped.status.success(), "{}", String::from_utf8_lossy(&clamped.stderr));
        let clog = String::from_utf8_lossy(&clamped.stderr);
        assert!(clog.contains("threads=1"), "{clog}");
        assert!(clog.contains("asked=8 applied=1"), "{clog}");
        assert!(!clamped.stdout.is_empty());
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

    #[test]
    fn clone_plans_split_the_cores_and_never_oversubscribe() {
        assert_eq!(ClonePlan::parse("auto", 8), Some(DEDICATED_PLAN));
        assert_eq!(ClonePlan::parse("", 4), Some(ClonePlan::Session));
        assert_eq!(ClonePlan::parse("3+3", 8), Some(ClonePlan::Split3));
        assert_eq!(ClonePlan::parse("bogus", 8), None);
        let s4 = ClonePlacement::for_persona(ClonePlan::Split4, "samantha", 8);
        let j4 = ClonePlacement::for_persona(ClonePlan::Split4, "jarvis", 8);
        assert_eq!((s4.cpu_list().as_str(), s4.threads), ("0-3", 4));
        assert_eq!((j4.cpu_list().as_str(), j4.threads), ("4-7", 4));
        let s3 = ClonePlacement::for_persona(ClonePlan::Split3, "samantha", 8);
        let j3 = ClonePlacement::for_persona(ClonePlan::Split3, "jarvis", 8);
        assert_eq!((s3.cpu_list().as_str(), s3.threads), ("0-2", 3));
        assert_eq!((j3.cpu_list().as_str(), j3.threads), ("3-5", 3));
        assert_eq!(ClonePlan::Split3.reserved_cpus(), vec![6, 7]);
        assert_eq!(ClonePlan::parse_with("auto", 8, true), Some(LOCAL_LLM_PLAN));
        assert_eq!(ClonePlan::parse_with("auto", 4, true), Some(ClonePlan::Session));
        let sl = ClonePlacement::for_persona(ClonePlan::Local, "samantha", 8);
        let jl = ClonePlacement::for_persona(ClonePlan::Local, "jarvis", 8);
        assert_eq!((sl.cpu_list().as_str(), sl.threads), ("0-3", 2));
        assert_eq!(sl.cpus, jl.cpus);
        assert!(sl.threads + jl.threads <= sl.cpus.len() as u32, "both speaking never oversubscribe");
        let (llm, t) = ClonePlan::Local.llm_placement().unwrap();
        assert_eq!((format_cpu_list(&llm).as_str(), t), ("4-7", 4));
        assert!(llm.iter().all(|c| !sl.cpus.contains(c)));
        for (a, b) in [(&s4, &j4), (&s3, &j3)] {
            assert!(a.cpus.iter().all(|c| !b.cpus.contains(c)), "disjoint");
            assert!(a.threads as usize <= a.cpus.len() && b.threads as usize <= b.cpus.len());
        }
        let small = ClonePlacement::for_persona(ClonePlan::Split4, "jarvis", 4);
        assert_eq!(
            small,
            ClonePlacement {
                plan: ClonePlan::Session,
                cpus: vec![],
                threads: 1
            }
        );
        assert_eq!(parse_cpu_list("0-2,6"), Some(vec![0, 1, 2, 6]));
        assert_eq!(parse_cpu_list("3-1"), None);
        assert_eq!(parse_cpu_list("x"), None);
        assert_eq!(format_cpu_list(&[0, 1, 2, 6]), "0-2,6");
    }

    #[test]
    fn real_stub_takes_split_threads_up_to_its_pinned_cores() {
        let script = worker_script().expect("worker script");
        let k = usable_cpus().min(4);
        let cpus: Vec<usize> = (0..k).collect();
        let mut cmd = Command::new("python3");
        cmd.arg(&script)
            .env("GALION_CLONE_STUB_PCM", "1")
            .env("GALION_CLONE_CPU_PLAN", "split4")
            .env("GALION_CLONE_THREADS", "8")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        apply_affinity(&mut cmd, &cpus);
        let mut child = cmd.spawn().unwrap();
        {
            let mut stdin = child.stdin.take().unwrap();
            stdin
                .write_all(b"{\"text\":\"hi\",\"persona\":\"jarvis\"}\n")
                .unwrap();
        }
        let out = child.wait_with_output().unwrap();
        let log = String::from_utf8_lossy(&out.stderr);
        assert!(out.status.success(), "{log}");
        assert!(log.contains(&format!("threads={k} ")), "{log}");
        assert!(log.contains(&format!("asked=8 applied={k}")), "{log}");
    }

    #[test]
    fn affinity_lands_in_the_child_before_exec() {
        if usable_cpus() < 2 {
            return;
        }
        let mut cmd = Command::new("sh");
        cmd.arg("-c")
            .arg("grep Cpus_allowed_list /proc/self/status");
        apply_affinity(&mut cmd, &[1]);
        let out = cmd.output().unwrap();
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(text.trim_end().ends_with("\t1"), "{text}");
    }

    const FAKE_CLONE: &str = r#"#!/usr/bin/env python3
import json, os, struct, sys
keys = ("OMP_NUM_THREADS", "OPENBLAS_NUM_THREADS", "MKL_NUM_THREADS", "TORCH_NUM_THREADS", "GALION_CLONE_THREADS")
want = os.environ.get("GALION_CLONE_THREADS", "")
plan = os.environ.get("GALION_CLONE_CPU_PLAN", "")
if not want.isdigit() or int(want) < 1 or any(os.environ.get(k) != want for k in keys):
    sys.stderr.write("galion-clone-refuse: cpu pin missing\n")
    sys.exit(3)
if plan not in ("session", "split4", "split3", "local") or (plan == "session" and want != "1"):
    sys.stderr.write("galion-clone-refuse: cpu plan missing\n")
    sys.exit(3)
if plan != "session" and int(want) > len(os.sched_getaffinity(0)):
    sys.stderr.write("galion-clone-refuse: threads exceed pinned cores\n")
    sys.exit(3)
if os.environ.get("GALION_CLONE_DEVICE") != "cpu" or os.environ.get("CUDA_VISIBLE_DEVICES") != "" or os.environ.get("GALION_CLONE_DENOISE") != "0":
    sys.stderr.write("galion-clone-refuse: cpu pin missing\n")
    sys.exit(3)
while True:
    line = sys.stdin.buffer.readline()
    if not line:
        break
    msg = json.loads(line.decode())
    if msg.get("denoise") in (True, "true", "1", 1):
        sys.stderr.write("galion-clone-refuse: denoise refused\n")
        sys.exit(3)
    if msg.get("device") not in (None, "", "cpu"):
        sys.stderr.write("galion-clone-refuse: CPU-only\n")
        sys.exit(3)
    for key in ("reference", "audio_prompt", "audio_prompt_path", "clip"):
        value = msg.get(key) or ""
        if str(value).lower().endswith(".mp3"):
            sys.stderr.write("galion-clone-refuse: mp3 is not a clone reference\n")
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
