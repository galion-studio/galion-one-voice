// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! A21 neural-weight slot (FP-03, owner decision t123u: BUILD A21 on CPU with an
//! MIT-licensed neural TTS). Honest by construction:
//! - no weights on disk            -> [`WeightStatus::Absent`]  (caller falls back to flite)
//! - weights in a refused format   -> [`WeightStatus::Refused`]
//! - weights present               -> [`WeightStatus::Present`] (sha256 checked against an
//!   expected hash: env, `<file>.sha256`, or the [`PINNED`] table)
//! - [`A21WeightTts`] synthesizes only when weights are Present + sha256-verified AND a
//!   decoder for that format is wired ([`decoder_for`]).
//!
//! Decoders:
//! - `.onnx` + `<file>.json` = a **Piper VITS voice pack** (rhasspy/piper-voices, MIT;
//!   default `en_US-ljspeech-medium`, LJSpeech public-domain data). Decoded by the
//!   **Piper runtime** (rhasspy/piper 2023.11.14-2, MIT; bundles its own ONNX Runtime
//!   and espeak-ng GPL-3 for phonemes) run **out of process** on the **CPU**. Nothing is
//!   linked into any Galion crate (check-no-forbidden stays green). One worker is
//!   spawned for the life of this slot and reused across clauses and turns
//!   (`--json-input --output_raw`). Runtime ([`piper_runtime`]): `$GALION_A21_RUNTIME`, else
//!   `galion-piper-rt` (same pack and phonemizer on onnxruntime 1.31.1, out of process;
//!   owner approval t613s2992, `runtimes/galion-piper-rt`), else (or with
//!   `GALION_PIPER_LEGACY=1`) the legacy CLI `<repo>/weights/runtime/piper/piper`.
//! - `.safetensors` / `.gq` = future Galion-native decoder on galion-tensor:
//!   [`GALION_NATIVE_DECODER_WIRED`] = `false`, so such packs report Present but inactive.
//!
//! Nothing here downloads anything (scripts/a21/fetch-a21.sh and
//! scripts/a21/build-piper-rt.sh are the only fetchers).

use super::piper_worker::{PiperProsody, PiperWorker};
use galion_heartbeat::symphony::NanoPcmRing;
use super::sha256;
use crate::{Audio, LanguageId, TtsEngine, VoiceError};
use std::path::{Path, PathBuf};

/// Flip to `true` only when a Galion-native decoder for `.gq`/`.safetensors` TTS packs
/// is implemented in-tree on galion-tensor.
pub const GALION_NATIVE_DECODER_WIRED: bool = false;
/// Env var: path to the Piper runtime executable. If set it is the only candidate.
pub const RUNTIME_ENV: &str = "GALION_A21_RUNTIME";
/// Env var: `1` selects the legacy piper 2023.11.14-2 CLI (onnxruntime 1.14.1)
/// instead of the default `galion-piper-rt` (onnxruntime 1.31.1).
pub const LEGACY_ENV: &str = "GALION_PIPER_LEGACY";
/// Default Piper runtime, relative to `weights/` (scripts/a21/build-piper-rt.sh).
pub const RT_REL: &str = "runtime/piper-rt/galion-piper-rt";
/// onnxruntime the default runtime loads, relative to `weights/` (LOCKS `a21_piper.onnxruntime`).
pub const RT_ORT_REL: &str = "runtime/ort/onnxruntime-linux-x64-1.31.1/lib/libonnxruntime.so.1.31.1";
/// Legacy piper CLI, relative to `weights/` (scripts/a21/fetch-a21.sh). Also ships the
/// espeak-ng library and data that both runtimes phonemize with.
pub const LEGACY_REL: &str = "runtime/piper/piper";
/// Pinned sha256 of known MIT voice packs (filename, sha256). Used when no env / `.sha256` file.
pub const PINNED: &[(&str, &str)] = &[
    ("en_US-ljspeech-medium.onnx", "6f52a751e2349abe7a76735eb09dc1875298c77ea2342ffd2fef79ff81b87f22"),
    ("en_US-ljspeech-medium.onnx.json", "141d612cc0a95ed7efc1ca936b845c2364967f2e9217c5dbfcf69fc4d6c65860"),
];

fn weights_dirs() -> [PathBuf; 2] {
    [Path::new(env!("CARGO_MANIFEST_DIR")).join("../../weights"), PathBuf::from("weights")]
}

/// The legacy piper 2023.11.14-2 CLI (onnxruntime 1.14.1), if installed.
pub fn piper_legacy_runtime() -> Option<PathBuf> {
    weights_dirs().into_iter().map(|w| w.join(LEGACY_REL)).find(|p| p.is_file())
}

/// The default runtime `galion-piper-rt` (onnxruntime 1.31.1), if installed with
/// everything it loads: the onnxruntime library and the Piper pack's espeak-ng.
pub fn piper_rt_runtime() -> Option<PathBuf> {
    weights_dirs().into_iter()
        .find(|w| w.join(RT_REL).is_file() && w.join(RT_ORT_REL).is_file() && w.join(LEGACY_REL).is_file())
        .map(|w| w.join(RT_REL))
}

/// True when `GALION_PIPER_LEGACY=1` (or `true`/`yes`).
pub fn legacy_forced() -> bool {
    matches!(std::env::var(LEGACY_ENV).ok().as_deref().map(str::trim), Some("1" | "true" | "yes"))
}

/// Locate the Piper runtime (CPU, out of process). `None` = not installed.
/// Order: `$GALION_A21_RUNTIME` (only candidate when set, so it can also disable A21);
/// `GALION_PIPER_LEGACY=1` -> legacy CLI; else `galion-piper-rt` if installed;
/// else the legacy CLI.
pub fn piper_runtime() -> Option<PathBuf> {
    if let Some(p) = std::env::var(RUNTIME_ENV).ok().filter(|s| !s.trim().is_empty()) {
        return Some(PathBuf::from(p)).filter(|p| p.is_file());
    }
    if legacy_forced() { return piper_legacy_runtime(); }
    piper_rt_runtime().or_else(piper_legacy_runtime)
}

/// Human label for a runtime path.
pub fn runtime_label(p: &Path) -> &'static str {
    if p.file_name().is_some_and(|n| n == "galion-piper-rt") { "galion-piper-rt, onnxruntime 1.31.1" } else { "piper 2023.11.14-2, onnxruntime 1.14.1" }
}

/// Decoder that can run a pack of `format`, or why not.
pub fn decoder_for(format: &str) -> Result<String, String> {
    match format {
        "onnx" => piper_runtime().map(|p| format!("piper-cpu ({}; {})", runtime_label(&p), p.display())).ok_or_else(|| format!("Piper runtime missing (set ${RUNTIME_ENV} or run scripts/a21/fetch-a21.sh and scripts/a21/build-piper-rt.sh)")),
        _ if GALION_NATIVE_DECODER_WIRED => Ok("galion-native".into()),
        _ => Err(format!("no Galion-native decoder for .{format} yet")),
    }
}

/// True if a decoder exists for the default A21 pack format (`.onnx` Piper).
pub fn decoder_wired() -> bool { decoder_for("onnx").is_ok() }
/// Env vars searched, in order.
pub const ENV_VARS: &[&str] = &["GALION_TTS_WEIGHTS", "GALION_SPEECH_WEIGHTS"];
/// Env var with the expected sha256 (hex). Alternative: `<weights>.sha256` next to the file.
pub const SHA_ENV: &str = "GALION_TTS_WEIGHTS_SHA256";
/// Allowed weight formats (LOCKS `formats.weight_formats`, + `.gq` once a student exists).
/// `.onnx` is accepted ONLY as a Piper voice pack (needs `<file>.json` beside it),
/// decoded out of process (owner decision t123u; see A21-BLOCKERS.md LOCKS note).
pub const ALLOWED_EXT: &[&str] = &["onnx", "safetensors", "gq"];
/// Formats recognised but refused (foreign runtime / container).
pub const REFUSED_EXT: &[&str] = &["pt", "pth", "bin", "ckpt", "ggml"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WeightStatus {
    Absent { searched: Vec<String> },
    Refused { path: PathBuf, reason: String },
    Present { path: PathBuf, format: String, bytes: u64, expected_sha256: Option<String>, sha256: Option<String> },
}

impl WeightStatus {
    /// Look in env vars, then `<repo>/weights/tts` and `./weights/tts`.
    pub fn discover() -> Self {
        let mut searched = Vec::new();
        let mut cands: Vec<PathBuf> = Vec::new();
        for v in ENV_VARS {
            match std::env::var(v) { Ok(p) if !p.trim().is_empty() => { searched.push(format!("${v}={p}")); cands.push(PathBuf::from(p)); } _ => searched.push(format!("${v} (unset)")) }
        }
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../weights/tts");
        for d in [repo, PathBuf::from("weights/tts")] { searched.push(d.display().to_string()); cands.push(d); }
        for c in cands {
            if let Some(s) = Self::inspect(&c) { return s; }
        }
        WeightStatus::Absent { searched }
    }

    /// Inspect one file or directory; `None` if nothing weight-like is there.
    pub fn inspect(p: &Path) -> Option<Self> {
        let files: Vec<PathBuf> = if p.is_dir() {
            let mut v: Vec<PathBuf> = std::fs::read_dir(p).ok()?.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.is_file()).collect();
            v.sort();
            v
        } else if p.is_file() { vec![p.to_path_buf()] } else { return None };
        let ext = |f: &PathBuf| f.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
        if let Some(f) = files.iter().find(|f| ext(f) == "onnx" && !Path::new(&format!("{}.json", f.display())).is_file()) {
            if !files.iter().any(|g| ext(g) != "onnx" && ALLOWED_EXT.contains(&ext(g).as_str())) {
                return Some(WeightStatus::Refused { path: f.clone(), reason: format!(".onnx without Piper config {}.json (only Piper voice packs are accepted)", f.display()) });
            }
        }
        let ok = |f: &PathBuf| ALLOWED_EXT.contains(&ext(f).as_str()) && (ext(f) != "onnx" || Path::new(&format!("{}.json", f.display())).is_file());
        if let Some(f) = files.iter().find(|f| ok(f)) {
            let bytes = std::fs::metadata(f).map(|m| m.len()).unwrap_or(0);
            let expected = std::env::var(SHA_ENV).ok().or_else(|| {
                std::fs::read_to_string(format!("{}.sha256", f.display())).ok().and_then(|s| s.split_whitespace().next().map(str::to_string))
            }).or_else(|| {
                let name = f.file_name()?.to_string_lossy().into_owned();
                PINNED.iter().find(|(n, _)| *n == name).map(|(_, h)| h.to_string())
            }).map(|s| s.to_ascii_lowercase());
            let sha = expected.as_ref().and_then(|_| sha256::file_hex(f).ok());
            return Some(WeightStatus::Present { path: f.clone(), format: ext(f), bytes, expected_sha256: expected, sha256: sha });
        }
        files.iter().find(|f| REFUSED_EXT.contains(&ext(f).as_str())).map(|f| WeightStatus::Refused {
            path: f.clone(),
            reason: format!(".{} refused by LOCKS formats (allowed: .safetensors/.gq; no foreign runtime)", ext(f)),
        })
    }

    /// sha256 matched an expected hash.
    pub fn verified(&self) -> bool {
        matches!(self, WeightStatus::Present { expected_sha256: Some(e), sha256: Some(s), .. } if e == s)
    }

    pub fn id(&self) -> &'static str {
        match self { WeightStatus::Absent { .. } => "absent", WeightStatus::Refused { .. } => "refused", WeightStatus::Present { .. } => if self.verified() { "present-verified" } else { "present-unverified" } }
    }

    pub fn summary(&self) -> String {
        match self {
            WeightStatus::Absent { searched } => format!("A21 weights ABSENT (searched: {})", searched.join(", ")),
            WeightStatus::Refused { path, reason } => format!("A21 weights REFUSED {}: {reason}", path.display()),
            WeightStatus::Present { path, format, bytes, expected_sha256, .. } => format!(
                "A21 weights PRESENT {} (.{format}, {bytes} B, sha256 {}); decoder: {}",
                path.display(),
                match (expected_sha256, self.verified()) { (None, _) => "NOT CHECKED (no expected hash)", (Some(_), true) => "OK", (Some(_), false) => "MISMATCH" },
                decoder_for(format).unwrap_or_else(|e| format!("NONE ({e})"))
            ),
        }
    }
}

/// The A21 TTS slot. Constructible only when it can really speak.
/// The Piper process is started on the first synthesis and kept for later turns.
pub struct A21WeightTts {
    model: PathBuf,
    config: PathBuf,
    runtime: PathBuf,
    /// Legacy CLI to fall back to when the default runtime cannot start or dies
    /// before its first utterance. `None` once on the legacy runtime.
    fallback: Option<PathBuf>,
    rate: u32,
    voice: String,
    worker: Option<PiperWorker>,
    spawns: u32,
    /// Persona-mood plan for the next utterances (`None` = V1 neutral path, unchanged).
    plan: Option<galion_emotion::PiperProsodyVector>,
    speaker_id: Option<u32>,
    /// Reused output buffer for the pitch/energy shaper (grows once, then reused).
    shaped: Vec<i16>,
}

impl A21WeightTts {
    pub fn try_new(status: &WeightStatus) -> Result<Self, String> {
        match status {
            WeightStatus::Present { .. } if !status.verified() => Err(format!("{} -> slot inactive until sha256 verifies", status.summary())),
            WeightStatus::Present { path, format, .. } => {
                let dec = decoder_for(format).map_err(|e| format!("{} -> slot inactive: {e}", status.summary()))?;
                if format != "onnx" { return Err(format!("{} -> {dec} not implemented", status.summary())); }
                let runtime = piper_runtime().ok_or("Piper runtime missing")?;
                let config = PathBuf::from(format!("{}.json", path.display()));
                let cfg = std::fs::read_to_string(&config).map_err(|e| format!("{}: {e}", config.display()))?;
                let rate = json_u32(&cfg, "sample_rate").ok_or_else(|| format!("{}: no audio.sample_rate", config.display()))?;
                let voice = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                let fallback = piper_legacy_runtime().filter(|l| *l != runtime && std::env::var(RUNTIME_ENV).is_err());
                Ok(Self { model: path.clone(), config, runtime, fallback, rate, voice, worker: None, spawns: 0, plan: None, speaker_id: None, shaped: Vec::new() })
            }
            other => Err(other.summary()),
        }
    }
    /// Voice pack name, e.g. `en_US-ljspeech-medium`.
    pub fn voice(&self) -> &str { &self.voice }
    /// Native pack rate before the ring resample (Piper ljspeech is 22050).
    pub fn native_rate(&self) -> u32 { self.rate }
    /// Pid of the warm worker, once it has been started.
    pub fn piper_pid(&self) -> Option<u32> { self.worker.as_ref().map(PiperWorker::pid) }
    /// How many times the Piper process has been spawned. `1` means every turn reused it.
    pub fn piper_spawns(&self) -> u32 { self.spawns }
    pub fn worker_alive(&mut self) -> bool { self.worker.as_mut().is_some_and(PiperWorker::is_alive) }

    /// Runtime executable this slot spawns now.
    pub fn runtime(&self) -> &Path { &self.runtime }

    /// Pack sample rate (Hz).
    pub fn sample_rate(&self) -> u32 { self.rate }
    /// Multi-speaker packs: which speaker to use (per line, no respawn).
    pub fn set_speaker(&mut self, id: Option<u32>) {
        self.speaker_id = id;
        if let Some(w) = self.worker.as_mut() { w.set_speaker(id); }
    }
    /// Current persona plan, if any.
    pub fn plan(&self) -> Option<&galion_emotion::PiperProsodyVector> { self.plan.as_ref() }
    /// Drop the persona plan: back to the V1 neutral scales.
    pub fn clear_plan(&mut self) { self.plan = None; }

    /// Scales the worker should run with right now.
    fn wanted_prosody(&self) -> PiperProsody {
        match &self.plan {
            Some(v) => PiperProsody::from_vector(v),
            // Neutral is the phone default (consent deny). The hook does not change scales.
            None => PiperProsody::from_env().with_emotion("Neutral"),
        }
    }

    /// Start the worker if needed. Piper reads its scales once, at spawn, so a
    /// persona plan whose (2-decimal) scales differ restarts it here — between
    /// utterances, never mid-clause. Call [`Self::prepare`] while the user is
    /// still speaking to pay that restart off the reply's critical path.
    fn ensure(&mut self) -> Result<(), VoiceError> {
        let want = self.wanted_prosody();
        let alive = self.worker.as_mut().is_some_and(PiperWorker::is_alive);
        if alive && self.worker.as_ref().is_some_and(|w| *w.prosody() == want) { return Ok(()); }
        self.worker = None;
        let mut worker = match PiperWorker::spawn(&self.runtime, &self.model, &self.config, &want) {
            Ok(w) => w,
            Err(e) => {
                if !self.use_fallback(&format!("{e}")) { return Err(e); }
                PiperWorker::spawn(&self.runtime, &self.model, &self.config, &want)?
            }
        };
        worker.set_speaker(self.speaker_id);
        self.worker = Some(worker);
        self.spawns += 1;
        Ok(())
    }

    /// Switch to the legacy CLI. `false` when already on it.
    fn use_fallback(&mut self, why: &str) -> bool {
        let Some(legacy) = self.fallback.take() else { return false };
        eprintln!("[a21] {} failed ({why}); falling back to {}", runtime_label(&self.runtime), runtime_label(&legacy));
        self.runtime = legacy;
        true
    }

    /// Spawn (if the plan changed) and warm the worker with one short line, so the
    /// next reply's first clause does not pay process start or model load.
    pub fn prepare(&mut self) -> Result<(), VoiceError> {
        let fresh = !self.worker.as_mut().is_some_and(PiperWorker::is_alive)
            || self.worker.as_ref().is_some_and(|w| *w.prosody() != self.wanted_prosody());
        self.ensure()?;
        if fresh { self.worker.as_mut().unwrap().synthesize("Okay.")?; }
        Ok(())
    }

    /// Persona-conditioned PCM at the pack rate: pre-pause silence, Piper with
    /// the persona scales, then the post-synthesis pitch/energy shaper. Without
    /// a plan this is the V1 PCM, untouched.
    pub fn synthesize_pcm(&mut self, text: &str) -> Result<Vec<i16>, VoiceError> {
        self.ensure()?;
        let pcm = self.worker.as_mut().unwrap().synthesize(text)?;
        let Some(plan) = self.plan else { return Ok(pcm) };
        let mut shaper = crate::emotive::EmotiveShaper::new(plan.pitch_shift_st, plan.energy);
        let pause = (plan.pre_pause_ms as u64 * self.rate as u64 / 1000) as usize;
        let need = pause + shaper.max_out(pcm.len());
        if self.shaped.len() < need { self.shaped.resize(need, 0); }
        let p = crate::emotive::EmotiveShaper::silence(plan.pre_pause_ms as u32, self.rate, &mut self.shaped[..pause]);
        let n = shaper.process(&pcm, &mut self.shaped[p..need]);
        Ok(self.shaped[..p + n].to_vec())
    }

    /// Start a new process after the previous one died. Not used for barge-in.
    /// A default runtime that died before producing any audio (missing library,
    /// bad install) is replaced by the legacy CLI. `galion-piper-rt` re-warms
    /// itself at load, so the next utterance is not a cold one.
    pub fn respawn(&mut self) -> Result<(), VoiceError> {
        let never_spoke = self.worker.as_ref().is_some_and(|w| w.calls() == 0);
        self.worker = None;
        if never_spoke { self.use_fallback("died before first audio"); }
        self.ensure()
    }
}

/// What one [`A21RingFeed::poll`] moved into the ring.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct A21Poll {
    /// 24 kHz samples stored in the ring on this call.
    pub pushed: usize,
    /// 24 kHz samples resampled from Piper so far.
    pub samples_24k: usize,
    pub utterance_done: bool,
}

/// Piper CPU feed for the symphony ring. Opens only when `GALION_SPEECH=a21`
/// and the pinned pack can speak. `poll` resamples native s16le to 24 kHz
/// and pushes it. That work is off `Heartbeat::step`. `GALION_CLONE_STUB_FRAMES`
/// is not this feed and is not a voice.
pub struct A21RingFeed {
    tts: A21WeightTts,
    pushed: usize,
}

impl A21RingFeed {
    pub fn open() -> Result<Self, String> {
        if std::env::var("GALION_SPEECH").ok().as_deref() != Some("a21") {
            return Err("GALION_SPEECH is not a21; Piper does not feed the ring".into());
        }
        let status = WeightStatus::discover();
        let tts = A21WeightTts::try_new(&status)?;
        Ok(Self { tts, pushed: 0 })
    }

    pub fn voice(&self) -> &str { self.tts.voice() }
    pub fn native_hz(&self) -> u32 { self.tts.native_rate() }

    /// Ask the warm Piper process for one clause. Does not read PCM yet.
    pub fn submit(&mut self, text: &str) -> Result<(), VoiceError> {
        self.tts.ensure()?;
        self.tts.worker.as_mut().unwrap().write_text(text)?;
        self.pushed = 0;
        Ok(())
    }

    /// Non-blocking. Resamples whatever native PCM has arrived to 24 kHz
    /// ([`super::SPEECH_RATE`]) and pushes the new tail. Allocates. Not `step`.
    pub fn poll(&mut self, ring: &mut NanoPcmRing) -> Result<A21Poll, VoiceError> {
        self.tts.ensure()?;
        let rate = self.tts.native_rate();
        let worker = self.tts.worker.as_mut().unwrap();
        worker.poll_pipes()?;
        let done = worker.utterance_done();
        let native = worker.copy_i16();
        let f32s: Vec<f32> = native.iter().map(|s| *s as f32 / 32768.0).collect();
        let up = super::resample(&f32s, rate, super::SPEECH_RATE);
        let fresh = if up.len() > self.pushed { &up[self.pushed..] } else { &[] };
        let pushed = ring.push(fresh);
        self.pushed += pushed;
        Ok(A21Poll { pushed, samples_24k: up.len(), utterance_done: done })
    }
}

/// Minimal `"key": <uint>` lookup (no serde in this crate).
fn json_u32(s: &str, key: &str) -> Option<u32> {
    let i = s.find(&format!("\"{key}\""))?;
    let rest = s[i + key.len() + 2..].trim_start().strip_prefix(':')?.trim_start();
    rest.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().ok()
}

/// Persona + mood + user emotion → Piper. Stores the plan only (no I/O, no
/// allocation); the scales take effect at the next [`A21WeightTts::prepare`] or utterance.
impl galion_emotion::ConditionedDecoder for A21WeightTts {
    fn condition(&mut self, c: &galion_emotion::PersonaConditioning) {
        self.plan = Some(c.piper());
    }
}

impl TtsEngine for A21WeightTts {
    fn languages(&self) -> &[LanguageId] { LanguageId::ALL }
    fn synthesize(&mut self, text: &str, lang: LanguageId) -> Result<Audio, VoiceError> {
        let _ = lang; // English voice pack.
        let (_, body) = crate::split_lang_token(text);
        let body = body.trim().replace(['\n', '\r'], " ");
        if body.is_empty() { return Err(VoiceError::EmptyInput); }
        let rate = self.rate;
        let pcm = self.synthesize_pcm(&body)?;
        let f: Vec<f32> = pcm.iter().map(|&s| s as f32 / 32768.0).collect();
        Ok(Audio { sample_rate: super::SPEECH_RATE, samples: super::resample(&f, rate, super::SPEECH_RATE) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exe(dir: &Path, name: &str, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let p = dir.join(name);
        std::fs::write(&p, body).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }
    const FAKE_LEGACY: &str = "#!/usr/bin/env python3\nimport json, struct, sys\nfor line in sys.stdin.buffer:\n    n = len(json.loads(line).get('text', ''))\n    sys.stdout.buffer.write(struct.pack('<200h', *([1000 + n] * 200))); sys.stdout.buffer.flush()\n    sys.stderr.write('Real-time factor: 0.1\\n'); sys.stderr.flush()\n";

    /// lat240: a default runtime that dies before its first audio (here: exits at
    /// once, like a missing onnxruntime library) is replaced by the legacy CLI
    /// within the same synthesize call path; later turns stay on the legacy process.
    #[test]
    fn dead_default_runtime_falls_back_to_legacy() {
        let d = tmp("rt-fallback");
        let rt = exe(&d, "galion-piper-rt", "#!/bin/sh\nexit 1\n");
        let legacy = exe(&d, "piper", FAKE_LEGACY);
        let model = d.join("v.onnx"); std::fs::write(&model, b"x").unwrap();
        let config = d.join("v.onnx.json"); std::fs::write(&config, b"{}").unwrap();
        let mut t = A21WeightTts { model, config, runtime: rt, fallback: Some(legacy.clone()), rate: 22050, voice: "v".into(), plan: None, speaker_id: None, shaped: Vec::new(), worker: None, spawns: 0 };
        let first = t.synthesize("hello", LanguageId::En);
        assert!(first.is_err() && !t.worker_alive(), "dead runtime must report an error, not hang");
        t.respawn().unwrap();
        assert_eq!(t.runtime(), legacy.as_path());
        let a = t.synthesize("hello", LanguageId::En).unwrap();
        assert!(!a.samples.is_empty());
        let pid = t.piper_pid();
        t.synthesize("again", LanguageId::En).unwrap();
        assert_eq!(t.piper_pid(), pid, "legacy process is reused");
        assert_eq!(runtime_label(&legacy), "piper 2023.11.14-2, onnxruntime 1.14.1");
        assert_eq!(runtime_label(Path::new("x/galion-piper-rt")), "galion-piper-rt, onnxruntime 1.31.1");
        let _ = std::fs::remove_dir_all(&d);
    }
    fn tmp(name: &str) -> PathBuf { let d = std::env::temp_dir().join(format!("galion-v103-{name}-{}", std::process::id())); let _ = std::fs::remove_dir_all(&d); std::fs::create_dir_all(&d).unwrap(); d }
    #[test] fn empty_dir_is_none() { assert_eq!(WeightStatus::inspect(&tmp("empty")), None); }
    #[test] fn bare_onnx_refused() {
        let d = tmp("onnx"); std::fs::write(d.join("voice.onnx"), b"x").unwrap();
        assert!(matches!(WeightStatus::inspect(&d), Some(WeightStatus::Refused { .. })));
    }
    #[test] fn pt_refused() {
        let d = tmp("pt"); std::fs::write(d.join("voice.pt"), b"x").unwrap();
        assert!(matches!(WeightStatus::inspect(&d), Some(WeightStatus::Refused { .. })));
    }
    #[test] fn piper_pack_unverified_stays_inactive() {
        let d = tmp("piper"); std::fs::write(d.join("v.onnx"), b"abd").unwrap();
        std::fs::write(d.join("v.onnx.json"), r#"{"audio":{"sample_rate": 22050}}"#).unwrap();
        std::fs::write(d.join("v.onnx.sha256"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad").unwrap();
        let s = WeightStatus::inspect(&d).unwrap();
        assert_eq!(s.id(), "present-unverified");
        assert!(A21WeightTts::try_new(&s).is_err(), "sha mismatch must not speak");
    }
    #[test] fn json_rate() { assert_eq!(json_u32(r#"{"audio": {"sample_rate": 22050, "q": 1}}"#, "sample_rate"), Some(22050)); }
    #[test] fn present_but_slot_stays_inactive() {
        let d = tmp("gq"); std::fs::write(d.join("tts.gq"), b"abc").unwrap();
        std::fs::write(d.join("tts.gq.sha256"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad  tts.gq\n").unwrap();
        let s = WeightStatus::inspect(&d).unwrap();
        assert!(s.verified(), "{s:?}");
        assert!(A21WeightTts::try_new(&s).is_err(), "must not speak without a Galion-native decoder");
    }
    #[test] fn sha_mismatch_not_verified() {
        let d = tmp("bad"); std::fs::write(d.join("tts.safetensors"), b"abd").unwrap();
        std::fs::write(d.join("tts.safetensors.sha256"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad").unwrap();
        let s = WeightStatus::inspect(&d).unwrap(); assert!(!s.verified()); assert_eq!(s.id(), "present-unverified");
    }
}
