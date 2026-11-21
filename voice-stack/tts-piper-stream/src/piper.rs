// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Warm Piper mouth. Runtime lookup follows `galion-ml-voice` `weights.rs`:
//! `$GALION_A21_RUNTIME`, else `galion-piper-rt` when it is installed, else the
//! legacy `piper` CLI. `GALION_PIPER_LEGACY=1` skips the rt binary.
//! The voice is the pinned stock female pack `en_US-ljspeech-medium`.

use crate::clause::{prepare_text, split_clauses};
use crate::mood::Prosody;
use crate::session::Session;
use crate::tts::{Pad, PcmChunk, SpeakReport, Tts, TtsError};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

pub const VOICE_ONNX: &str = "en_US-ljspeech-medium.onnx";
pub const VOICE_JSON: &str = "en_US-ljspeech-medium.onnx.json";
/// `crates/galion-ml-voice` `PINNED` and `scripts/a21/fetch-a21.sh`.
pub const PINNED_ONNX_SHA: &str =
    "6f52a751e2349abe7a76735eb09dc1875298c77ea2342ffd2fef79ff81b87f22";
pub const PINNED_JSON_SHA: &str =
    "141d612cc0a95ed7efc1ca936b845c2364967f2e9217c5dbfcf69fc4d6c65860";

/// How the session reads the child. `Auto` is `poll` on Linux and `thread` elsewhere.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[doc(hidden)]
pub enum IoMode {
    Auto,
    Poll,
    Thread,
}

/// Paths and spawn-time scales. Scales are fixed for the life of the process.
#[derive(Clone, Debug)]
pub struct PiperVoice {
    pub runtime: PathBuf,
    pub model: PathBuf,
    pub config: PathBuf,
    pub sample_rate: u32,
    pub length_scale: String,
    pub noise_scale: String,
    pub noise_w: String,
    pub sentence_silence: String,
    pub io: IoMode,
}

impl PiperVoice {
    pub fn from_paths(runtime: PathBuf, model: PathBuf, config: PathBuf) -> Result<Self, TtsError> {
        if !runtime.is_file() {
            return Err(TtsError::Missing(format!("runtime {}", runtime.display())));
        }
        if !model.is_file() {
            return Err(TtsError::Missing(format!("model {}", model.display())));
        }
        if !config.is_file() {
            return Err(TtsError::Missing(format!("config {}", config.display())));
        }
        let text = std::fs::read_to_string(&config)
            .map_err(|e| TtsError::Io(format!("{}: {e}", config.display())))?;
        let sample_rate = sample_rate_in(&text).unwrap_or(22_050);
        let (length_scale, noise_scale, noise_w, sentence_silence) = scales_from_env();
        Ok(Self {
            runtime,
            model,
            config,
            sample_rate,
            length_scale,
            noise_scale,
            noise_w,
            sentence_silence,
            io: IoMode::Auto,
        })
    }

    #[doc(hidden)]
    pub fn with_io(mut self, io: IoMode) -> Self {
        self.io = io;
        self
    }
}

/// First voice and runtime that exist. Does not download and does not hash.
pub fn resolve() -> Result<PiperVoice, TtsError> {
    let runtime = find_runtime()?;
    let (model, config) = find_model()?;
    PiperVoice::from_paths(runtime, model, config)
}

/// Check pinned sha256 when the filename is the stock voice. Other names pass.
pub fn verify_voice(voice: &PiperVoice) -> Result<(), TtsError> {
    verify_pins(voice)
}

fn verify_pins(voice: &PiperVoice) -> Result<(), TtsError> {
    check_pin(&voice.model, VOICE_ONNX, PINNED_ONNX_SHA)?;
    check_pin(&voice.config, VOICE_JSON, PINNED_JSON_SHA)
}

fn check_pin(path: &Path, name: &str, expected: &str) -> Result<(), TtsError> {
    if path.file_name().and_then(|s| s.to_str()) != Some(name) {
        return Ok(());
    }
    let got = sha256_file(path)?;
    if got != expected {
        return Err(TtsError::BadSha {
            path: path.display().to_string(),
            expected: expected.to_string(),
            got,
        });
    }
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String, TtsError> {
    let mut f = File::open(path).map_err(|e| TtsError::Io(format!("{}: {e}", path.display())))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 1 << 20];
    loop {
        let n = f
            .read(&mut buf)
            .map_err(|e| TtsError::Io(format!("{}: {e}", path.display())))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn sample_rate_in(json: &str) -> Option<u32> {
    let key = "\"sample_rate\"";
    let i = json.find(key)?;
    let rest = &json[i + key.len()..];
    let digits: String = rest
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse().ok()
}

fn scales_from_env() -> (String, String, String, String) {
    // Same split as `PiperProsody::from_env`. Sentence silence stays 0 so a
    // mood pause is PCM, not a respawn.
    let natural = std::env::var("GALION_A21_NOISE").ok().as_deref() == Some("natural");
    if natural {
        ("1.0".into(), "0.667".into(), "0.8".into(), "0".into())
    } else {
        ("1.0".into(), "0".into(), "0".into(), "0".into())
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

fn find_runtime() -> Result<PathBuf, TtsError> {
    if let Some(p) = env_path("GALION_A21_RUNTIME") {
        if p.is_file() {
            return Ok(p);
        }
        return Err(TtsError::Missing(format!(
            "GALION_A21_RUNTIME={}",
            p.display()
        )));
    }
    let mut tried = Vec::new();
    for c in runtime_candidates() {
        let name = c.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if legacy_forced() && name == "galion-piper-rt" {
            continue;
        }
        tried.push(c.display().to_string());
        if c.is_file() {
            return Ok(c);
        }
    }
    Err(TtsError::Missing(format!(
        "piper runtime. looked in: {}",
        tried.join(", ")
    )))
}

fn find_model() -> Result<(PathBuf, PathBuf), TtsError> {
    if let Some(model) = env_path("GALION_PIPER_MODEL")
        .or_else(|| env_path("GALION_TTS_WEIGHTS"))
        .or_else(|| env_path("GALION_SPEECH_WEIGHTS"))
    {
        if !model.is_file() {
            return Err(TtsError::Missing(format!("model {}", model.display())));
        }
        let config = env_path("GALION_PIPER_CONFIG")
            .unwrap_or_else(|| PathBuf::from(format!("{}.json", model.display())));
        if !config.is_file() {
            return Err(TtsError::Missing(format!("config {}", config.display())));
        }
        return Ok((model, config));
    }
    let mut tried = Vec::new();
    for model in model_candidates() {
        tried.push(model.display().to_string());
        let config = PathBuf::from(format!("{}.json", model.display()));
        if model.is_file() && config.is_file() {
            return Ok((model, config));
        }
    }
    Err(TtsError::Missing(format!(
        "en_US-ljspeech-medium not on disk. Run scripts/fetch-voice.sh. looked in: {}",
        tried.join(", ")
    )))
}

fn env_path(key: &str) -> Option<PathBuf> {
    std::env::var(key)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
}

fn runtime_candidates() -> Vec<PathBuf> {
    let mut v = Vec::new();
    for root in roots() {
        v.push(root.join("weights/runtime/piper-rt/galion-piper-rt"));
        v.push(root.join("weights/runtime/piper/piper"));
        v.push(root.join("var/piper/piper"));
        v.push(root.join("var/piper/piper.exe"));
    }
    v.push(PathBuf::from("weights/runtime/piper-rt/galion-piper-rt"));
    v.push(PathBuf::from("weights/runtime/piper/piper"));
    v.push(PathBuf::from("var/piper/piper"));
    v.push(PathBuf::from("var/piper/piper.exe"));
    v
}

fn model_candidates() -> Vec<PathBuf> {
    let name = format!("weights/tts/{VOICE_ONNX}");
    let local = format!("var/voices/{VOICE_ONNX}");
    let mut v = Vec::new();
    for root in roots() {
        v.push(root.join(&name));
        v.push(root.join(&local));
    }
    v.push(PathBuf::from(name));
    v.push(PathBuf::from(local));
    v
}

fn roots() -> Vec<PathBuf> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    vec![
        manifest.join("../.."),
        manifest.clone(),
        manifest.join(".."),
    ]
}

/// Warm Piper session. Drop kills the child. A dead child is replaced on the next speak.
pub struct PiperTts {
    voice: PiperVoice,
    session: Option<Session>,
    calls: u32,
}

impl PiperTts {
    /// Hash the pinned filenames, then spawn.
    pub fn open(voice: PiperVoice) -> Result<Self, TtsError> {
        verify_pins(&voice)?;
        Self::spawn(voice)
    }

    /// Spawn without hashing. The bench times this separately from the sha pass.
    pub fn spawn(voice: PiperVoice) -> Result<Self, TtsError> {
        let session = Session::spawn(&voice)?;
        Ok(Self {
            voice,
            session: Some(session),
            calls: 0,
        })
    }

    pub fn open_resolved() -> Result<Self, TtsError> {
        Self::open(resolve()?)
    }

    pub fn sample_rate(&self) -> u32 {
        self.voice.sample_rate
    }

    pub fn io_name(&self) -> &'static str {
        self.session.as_ref().map(|s| s.io_name()).unwrap_or("down")
    }

    pub fn pid(&self) -> Option<u32> {
        self.session.as_ref().map(|s| s.pid())
    }

    fn ensure(&mut self) -> Result<(), TtsError> {
        let alive = match self.session.as_mut() {
            Some(s) => s.is_alive(),
            None => false,
        };
        if !alive {
            self.session = Some(Session::spawn(&self.voice)?);
            self.calls = 0;
        }
        Ok(())
    }
}

impl Tts for PiperTts {
    fn id(&self) -> &str {
        "piper"
    }
    fn sample_rate(&self) -> u32 {
        self.voice.sample_rate
    }
    fn speak(
        &mut self,
        text: &str,
        mood: Pad,
        on_pcm: &mut dyn FnMut(PcmChunk<'_>),
    ) -> Result<SpeakReport, TtsError> {
        let clauses = split_clauses(&prepare_text(text));
        if clauses.is_empty() {
            return Err(TtsError::Empty);
        }
        self.ensure()?;
        let prosody = Prosody::from_pad(mood);
        let rate = self.voice.sample_rate;
        let cold = self.calls == 0;
        let spoken = {
            let session = self
                .session
                .as_mut()
                .ok_or_else(|| TtsError::Died("no session".into()))?;
            session.speak(&clauses, prosody, rate, on_pcm)
        };
        let out = match spoken {
            Ok(out) => out,
            Err(e) => {
                let dead = self.session.as_mut().map(|s| !s.is_alive()).unwrap_or(true);
                if dead {
                    self.session = None;
                }
                return Err(e);
            }
        };
        self.calls += 1;
        Ok(SpeakReport::from_parts(
            "piper",
            rate,
            cold,
            Some(out.pid),
            out.cold_spawn_to_first_pcm_ms,
            out.samples,
            out.wall_ms,
            out.events,
            out.detail,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_rate_reads_the_piper_config() {
        assert_eq!(
            sample_rate_in(r#"{"audio": {"sample_rate": 22050, "q": 1}}"#),
            Some(22050)
        );
        assert_eq!(PINNED_ONNX_SHA.len(), 64);
        assert_eq!(PINNED_JSON_SHA.len(), 64);
        assert!(PINNED_ONNX_SHA.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn pinned_name_with_wrong_bytes_is_refused() {
        let dir = std::env::temp_dir().join(format!("galion-tts-pin-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let model = dir.join(VOICE_ONNX);
        let config = dir.join("other.json");
        std::fs::write(&model, b"not-the-voice").unwrap();
        std::fs::write(&config, b"{}").unwrap();
        let err = check_pin(&model, VOICE_ONNX, PINNED_ONNX_SHA).unwrap_err();
        assert!(matches!(err, TtsError::BadSha { .. }));
        assert!(check_pin(&config, VOICE_ONNX, PINNED_ONNX_SHA).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
