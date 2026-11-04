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
//!   (`--json-input --output_raw`). Runtime: `$GALION_A21_RUNTIME` or
//!   `<repo>/weights/runtime/piper/piper`.
//! - `.safetensors` / `.gq` = future Galion-native decoder on galion-tensor:
//!   [`GALION_NATIVE_DECODER_WIRED`] = `false`, so such packs report Present but inactive.
//! Nothing here downloads anything (scripts/a21/fetch-a21.sh is the only fetcher).

use super::piper_worker::{PiperProsody, PiperWorker};
use galion_heartbeat::symphony::NanoPcmRing;
use super::sha256;
use crate::{Audio, LanguageId, TtsEngine, VoiceError};
use std::path::{Path, PathBuf};

/// Flip to `true` only when a Galion-native decoder for `.gq`/`.safetensors` TTS packs
/// is implemented in-tree on galion-tensor.
pub const GALION_NATIVE_DECODER_WIRED: bool = false;
/// Env var: path to the Piper runtime executable (default `<repo>/weights/runtime/piper/piper`).
pub const RUNTIME_ENV: &str = "GALION_A21_RUNTIME";
/// Pinned sha256 of known MIT voice packs (filename, sha256). Used when no env / `.sha256` file.
pub const PINNED: &[(&str, &str)] = &[
    ("en_US-ljspeech-medium.onnx", "6f52a751e2349abe7a76735eb09dc1875298c77ea2342ffd2fef79ff81b87f22"),
    ("en_US-ljspeech-medium.onnx.json", "141d612cc0a95ed7efc1ca936b845c2364967f2e9217c5dbfcf69fc4d6c65860"),
];

/// Locate the Piper runtime (CPU, out of process). `None` = not installed.
/// If `$GALION_A21_RUNTIME` is set it is the only candidate (so it can also disable A21).
pub fn piper_runtime() -> Option<PathBuf> {
    if let Some(p) = std::env::var(RUNTIME_ENV).ok().filter(|s| !s.trim().is_empty()) {
        return Some(PathBuf::from(p)).filter(|p| p.is_file());
    }
    [Path::new(env!("CARGO_MANIFEST_DIR")).join("../../weights/runtime/piper/piper"), PathBuf::from("weights/runtime/piper/piper")]
        .into_iter()
        .find(|p| p.is_file())
}

/// Decoder that can run a pack of `format`, or why not.
pub fn decoder_for(format: &str) -> Result<String, String> {
    match format {
        "onnx" => piper_runtime().map(|p| format!("piper-cpu ({})", p.display())).ok_or_else(|| format!("Piper runtime missing (set ${RUNTIME_ENV} or run scripts/a21/fetch-a21.sh)")),
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
    rate: u32,
    voice: String,
    worker: Option<PiperWorker>,
    spawns: u32,
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
                Ok(Self { model: path.clone(), config, runtime, rate, voice, worker: None, spawns: 0 })
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

    fn ensure(&mut self) -> Result<(), VoiceError> {
        let alive = self.worker.as_mut().is_some_and(PiperWorker::is_alive);
        if alive { return Ok(()); }
        // Neutral is the phone default (consent deny). The hook does not change scales.
        let prosody = PiperProsody::from_env().with_emotion("Neutral");
        let worker = PiperWorker::spawn(&self.runtime, &self.model, &self.config, &prosody)?;
        self.worker = Some(worker);
        self.spawns += 1;
        Ok(())
    }

    /// Start a new process after the previous one died. Not used for barge-in.
    pub fn respawn(&mut self) -> Result<(), VoiceError> {
        self.worker = None;
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

impl TtsEngine for A21WeightTts {
    fn languages(&self) -> &[LanguageId] { LanguageId::ALL }
    fn synthesize(&mut self, text: &str, lang: LanguageId) -> Result<Audio, VoiceError> {
        let _ = lang; // English voice pack.
        let (_, body) = crate::split_lang_token(text);
        let body = body.trim().replace(['\n', '\r'], " ");
        if body.is_empty() { return Err(VoiceError::EmptyInput); }
        self.ensure()?;
        let rate = self.rate;
        let pcm = self.worker.as_mut().unwrap().synthesize(&body)?;
        let f: Vec<f32> = pcm.iter().map(|&s| s as f32 / 32768.0).collect();
        Ok(Audio { sample_rate: super::SPEECH_RATE, samples: super::resample(&f, rate, super::SPEECH_RATE) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
