// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Find the Piper runtime, the A21 voice pack, Vosk, and ffmpeg the way the
//! rest of the tree already does.
//!
//! Piper order matches `crates/galion-ml-voice/src/speech/weights.rs`
//! (`GALION_A21_RUNTIME`, then `galion-piper-rt`, then the legacy CLI).
//! Vosk paths match `crates/galion-asr-vosk` (`GALION_VOSK_MODEL`,
//! `weights/asr/vosk-model-small-en-us-0.15`, `libvosk.so`).
//! Nothing here downloads a file.

use std::path::{Path, PathBuf};
use std::process::Command;

pub struct Tools {
    pub piper: Option<PathBuf>,
    pub model: Option<PathBuf>,
    pub config: Option<PathBuf>,
    pub sample_rate: Option<u32>,
    pub vosk_model: Option<PathBuf>,
    pub vosk_lib: Option<PathBuf>,
    pub ffmpeg: Option<PathBuf>,
    pub ecapa_bin: Option<PathBuf>,
    pub samantha_ref: Option<PathBuf>,
    pub notes: Vec<String>,
}

impl Tools {
    pub fn discover() -> Self {
        let mut notes = Vec::new();
        let piper = find_piper();
        let model = find_voice();
        let config = model.as_ref().and_then(|m| config_for(m));
        let sample_rate = config.as_ref().and_then(|p| read_sample_rate(p));
        if piper.is_none() {
            notes.push("Piper runtime is not on this machine. Set GALION_A21_RUNTIME, or run scripts/a21/fetch-a21.sh. No audio was invented.".into());
        }
        if model.is_none() {
            notes.push("Piper voice pack is not on this machine. GALION_TTS_WEIGHTS or weights/tts/en_US-ljspeech-medium.onnx. The pack is not a Samantha clone.".into());
        }
        if model.is_some() && sample_rate.is_none() {
            notes.push(
                "The voice pack has no readable audio.sample_rate. Duration was not guessed."
                    .into(),
            );
        }
        let vosk_model = find_vosk_model();
        let vosk_lib = find_vosk_lib();
        if vosk_model.is_none() || vosk_lib.is_none() {
            notes.push("Vosk model or libvosk is missing. Set GALION_VOSK_MODEL and GALION_VOSK_LIB, or run scripts/asr/fetch-vosk.sh. No transcript was invented.".into());
        }
        let ffmpeg = find_ffmpeg();
        if ffmpeg.is_none() {
            notes.push("ffmpeg is not on PATH. The wer_t648.py resample step was not replaced with a different resampler.".into());
        }
        let ecapa_bin = find_ecapa();
        let samantha_ref = env_file("GALION_SAMANTHA_REF");
        if samantha_ref.is_none() {
            notes.push("GALION_SAMANTHA_REF is unset. ECAPA is not run. No reference clip is read or written.".into());
        } else if ecapa_bin.is_none() {
            notes.push("GALION_SAMANTHA_REF is set, but galion-ecapa is not on this machine. ECAPA was not scored.".into());
        }
        notes.push("emotion_match needs the emotion-v2 recogniser (0.70*category_blend+0.30*emb_sim). Those weights are not invoked by this crate. The CPU stub is not a score.".into());
        Self {
            piper,
            model,
            config,
            sample_rate,
            vosk_model,
            vosk_lib,
            ffmpeg,
            ecapa_bin,
            samantha_ref,
            notes,
        }
    }

    pub fn piper_ready(&self) -> bool {
        self.piper.is_some() && self.model.is_some() && self.sample_rate.is_some()
    }

    pub fn asr_ready(&self) -> bool {
        self.vosk_model.is_some()
            && self.vosk_lib.is_some()
            && self.ffmpeg.is_some()
            && self.sample_rate.is_some()
    }
}

fn weight_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(home) = std::env::var_os("GALION_HOME") {
        roots.push(PathBuf::from(home).join("weights"));
    }
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    roots.push(manifest.join("../../weights"));
    roots.push(PathBuf::from("weights"));
    roots.push(PathBuf::from(r"G:\galion\weights"));
    roots
}

fn env_file(key: &str) -> Option<PathBuf> {
    let raw = std::env::var(key).ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let path = PathBuf::from(trimmed);
    if path.is_file() {
        Some(path)
    } else {
        None
    }
}

fn first_file(candidates: impl IntoIterator<Item = PathBuf>) -> Option<PathBuf> {
    candidates.into_iter().find(|p| p.is_file())
}

fn find_piper() -> Option<PathBuf> {
    if let Some(path) = env_file("GALION_A21_RUNTIME") {
        return Some(path);
    }
    let legacy = std::env::var("GALION_PIPER_LEGACY")
        .ok()
        .map(|v| matches!(v.trim(), "1" | "true" | "yes"))
        .unwrap_or(false);
    let mut rt = Vec::new();
    let mut old = Vec::new();
    for root in weight_roots() {
        rt.push(root.join("runtime/piper-rt/galion-piper-rt"));
        rt.push(root.join("runtime/piper-rt/galion-piper-rt.exe"));
        old.push(root.join("runtime/piper/piper"));
        old.push(root.join("runtime/piper/piper.exe"));
    }
    if legacy {
        return first_file(old);
    }
    first_file(rt).or_else(|| first_file(old))
}

fn find_voice() -> Option<PathBuf> {
    if let Some(path) = env_file("GALION_TTS_WEIGHTS") {
        return Some(path);
    }
    let mut candidates = Vec::new();
    for root in weight_roots() {
        candidates.push(root.join("tts/en_US-ljspeech-medium.onnx"));
    }
    first_file(candidates)
}

fn config_for(model: &Path) -> Option<PathBuf> {
    let joined = PathBuf::from(format!("{}.json", model.display()));
    if joined.is_file() {
        return Some(joined);
    }
    None
}

pub fn read_sample_rate(config: &Path) -> Option<u32> {
    let text = std::fs::read_to_string(config).ok()?;
    parse_sample_rate(&text)
}

pub fn parse_sample_rate(text: &str) -> Option<u32> {
    let idx = text.find("\"sample_rate\"")?;
    let rest = &text[idx + "\"sample_rate\"".len()..];
    let digits: String = rest
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit())
        .collect();
    let rate = digits.parse().ok()?;
    if rate == 0 {
        None
    } else {
        Some(rate)
    }
}

fn vosk_model_ok(path: &Path) -> bool {
    path.is_dir() && path.join("am/final.mdl").is_file() && path.join("conf/model.conf").is_file()
}

fn find_vosk_model() -> Option<PathBuf> {
    if let Some(raw) = std::env::var("GALION_VOSK_MODEL")
        .ok()
        .filter(|s| !s.trim().is_empty())
    {
        let path = PathBuf::from(raw.trim());
        return vosk_model_ok(&path).then_some(path);
    }
    weight_roots()
        .into_iter()
        .map(|root| root.join("asr/vosk-model-small-en-us-0.15"))
        .find(|p| vosk_model_ok(p))
}

fn find_vosk_lib() -> Option<PathBuf> {
    if let Some(path) = env_file("GALION_VOSK_LIB") {
        return Some(path);
    }
    let mut candidates = Vec::new();
    for root in weight_roots() {
        candidates.push(root.join("asr/libvosk/libvosk.so"));
        candidates.push(root.join("asr/libvosk/libvosk.dll"));
    }
    first_file(candidates)
}

fn find_ffmpeg() -> Option<PathBuf> {
    if let Some(path) = env_file("GALION_FFMPEG") {
        return Some(path);
    }
    Command::new("ffmpeg")
        .arg("-version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|_| PathBuf::from("ffmpeg"))
}

fn find_ecapa() -> Option<PathBuf> {
    if let Some(path) = env_file("GALION_ECAPA") {
        return Some(path);
    }
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let rel = manifest.join("../../tools/cloning-v2/scorers/ecapa/target/release/galion-ecapa");
    rel.is_file().then_some(rel)
}

/// Path relative to the repo root when the file lives inside it.
/// Otherwise the file name only, so a report does not carry a home directory.
pub fn display_path(path: &Path) -> String {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo = manifest.join("../..");
    if let (Ok(path), Ok(repo)) = (path.canonicalize(), repo.canonicalize()) {
        if let Ok(rel) = path.strip_prefix(&repo) {
            return rel.display().to_string();
        }
    }
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "set".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_rate_parser_reads_the_piper_json_shape() {
        let text = r#"{"audio":{"sample_rate":22050,"quality":"medium"}}"#;
        assert_eq!(parse_sample_rate(text), Some(22050));
        assert_eq!(parse_sample_rate("{}"), None);
    }
}
