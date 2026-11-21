// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! In-process streaming Vosk. One recognizer, 20 ms frames, partials when the
//! hypothesis changes, a final when libvosk endpoints or when `finish` is called.
//! This is the same library and model directory as `galion-asr-vosk` (PR #26).
//! The #26 worker speaks one whole utterance per line; this session accepts
//! audio as it arrives so a heartbeat can push 80 ms at a time.

use std::path::{Path, PathBuf};

use crate::engine::Asr;
use crate::error::AsrError;
use crate::event::{samples_to_ms, AsrEvent, AsrKind, Word, SAMPLE_RATE, VOSK_FRAME_SAMPLES};
use crate::json::{parse, Json};
use crate::paths::{self, env_u32};
use crate::text::clean_transcript;
use crate::vosk_ffi::Ffi;

pub struct VoskConfig {
    pub model_dir: PathBuf,
    pub lib_path: Option<PathBuf>,
    pub threads: u32,
}

impl VoskConfig {
    pub fn from_env() -> Result<Self, AsrError> {
        Ok(Self {
            model_dir: paths::find_vosk_model()?,
            lib_path: paths::find_vosk_lib(),
            threads: env_u32("GALION_VOSK_THREADS", 2)?,
        })
    }
}

pub struct VoskAsr {
    ffi: Ffi,
    model: *mut std::ffi::c_void,
    rec: *mut std::ffi::c_void,
    pending: Vec<i16>,
    fed: usize,
    open_samples: usize,
    last_partial: String,
    label: String,
    threads: u32,
    model_dir: PathBuf,
}

impl VoskAsr {
    pub fn open(config: VoskConfig) -> Result<Self, AsrError> {
        if !paths::model_dir_ok(&config.model_dir) {
            return Err(AsrError::new(format!(
                "Vosk model not found at {} (need am/final.mdl and conf/model.conf). No transcript was invented.",
                config.model_dir.display()
            )));
        }
        apply_threads(config.threads);
        let ffi = Ffi::open(config.lib_path.as_deref())?;
        let model_str = path_utf8(&config.model_dir)?;
        let model = ffi.model_new(&model_str)?;
        let rec = ffi.recognizer_new(model, SAMPLE_RATE as f32);
        if rec.is_null() {
            ffi.model_free(model);
            return Err(AsrError::new(
                "vosk_recognizer_new returned null. No transcript was invented.",
            ));
        }
        ffi.set_words(rec, 1);
        let file_name = config
            .model_dir
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("vosk");
        Ok(Self {
            ffi,
            model,
            rec,
            pending: Vec::new(),
            fed: 0,
            open_samples: 0,
            last_partial: String::new(),
            label: format!("vosk:{file_name}"),
            threads: config.threads,
            model_dir: config.model_dir,
        })
    }

    pub fn open_from_env() -> Result<Self, AsrError> {
        Self::open(VoskConfig::from_env()?)
    }

    pub fn model_dir(&self) -> &Path {
        &self.model_dir
    }

    pub fn threads(&self) -> u32 {
        self.threads
    }

    fn reset_rec(&mut self) {
        self.ffi.reset(self.rec);
        self.ffi.set_words(self.rec, 1);
        self.open_samples = 0;
        self.last_partial.clear();
    }

    fn feed_samples(&mut self, samples: &[i16]) -> Result<Vec<AsrEvent>, AsrError> {
        if samples.is_empty() {
            return Ok(Vec::new());
        }
        let mut bytes = Vec::with_capacity(samples.len() * 2);
        for sample in samples {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        self.fed = self.fed.saturating_add(samples.len());
        self.open_samples = self.open_samples.saturating_add(samples.len());
        let rc = self.ffi.accept(self.rec, &bytes)?;
        let audio_ms = samples_to_ms(self.fed);
        if rc == 1 {
            let (text, words) = result_of(&self.ffi.result(self.rec))?;
            self.reset_rec();
            return Ok(vec![final_event(text, words, audio_ms, "endpoint")]);
        }
        let partial = match partial_of(&self.ffi.partial(self.rec)) {
            Ok(text) => text,
            Err(_) => return Ok(Vec::new()),
        };
        if !partial.is_empty() && partial != self.last_partial {
            self.last_partial = partial.clone();
            return Ok(vec![AsrEvent {
                kind: AsrKind::Partial,
                text: partial,
                audio_ms,
                words: Vec::new(),
                why: "partial",
            }]);
        }
        Ok(Vec::new())
    }
}

impl Asr for VoskAsr {
    fn push_pcm(&mut self, pcm: &[i16]) -> Result<Vec<AsrEvent>, AsrError> {
        if pcm.is_empty() {
            return Ok(Vec::new());
        }
        self.pending.extend_from_slice(pcm);
        let mut events = Vec::new();
        while self.pending.len() >= VOSK_FRAME_SAMPLES {
            let frame: Vec<i16> = self.pending.drain(..VOSK_FRAME_SAMPLES).collect();
            events.extend(self.feed_samples(&frame)?);
        }
        Ok(events)
    }

    fn finish(&mut self) -> Result<Vec<AsrEvent>, AsrError> {
        let mut events = Vec::new();
        if !self.pending.is_empty() {
            let rest = std::mem::take(&mut self.pending);
            events.extend(self.feed_samples(&rest)?);
        }
        if self.open_samples == 0 {
            return Ok(events);
        }
        let (text, words) = result_of(&self.ffi.final_result(self.rec))?;
        let audio_ms = samples_to_ms(self.fed);
        self.reset_rec();
        events.push(final_event(text, words, audio_ms, "finish"));
        Ok(events)
    }

    fn reset(&mut self) -> Result<(), AsrError> {
        self.pending.clear();
        self.fed = 0;
        self.reset_rec();
        Ok(())
    }

    fn name(&self) -> &str {
        &self.label
    }
}

impl Drop for VoskAsr {
    fn drop(&mut self) {
        self.ffi.recognizer_free(self.rec);
        self.rec = std::ptr::null_mut();
        self.ffi.model_free(self.model);
        self.model = std::ptr::null_mut();
    }
}

fn final_event(text: String, words: Vec<Word>, audio_ms: u32, why: &'static str) -> AsrEvent {
    AsrEvent {
        kind: AsrKind::Final,
        text,
        audio_ms,
        words,
        why,
    }
}

fn apply_threads(threads: u32) {
    let value = threads.max(1).to_string();
    if std::env::var_os("OMP_NUM_THREADS").is_none() {
        std::env::set_var("OMP_NUM_THREADS", &value);
    }
    if std::env::var_os("OPENBLAS_NUM_THREADS").is_none() {
        std::env::set_var("OPENBLAS_NUM_THREADS", &value);
    }
}

fn path_utf8(path: &Path) -> Result<String, AsrError> {
    path.to_str()
        .map(str::to_string)
        .ok_or_else(|| AsrError::new("model path is not utf-8. No transcript was invented."))
}

fn partial_of(blob: &str) -> Result<String, AsrError> {
    let value = parse(blob.trim()).map_err(|err| AsrError::new(format!("vosk partial: {err}")))?;
    let text = value.get("partial").and_then(Json::as_str).unwrap_or("");
    Ok(clean_transcript(text))
}

fn result_of(blob: &str) -> Result<(String, Vec<Word>), AsrError> {
    let value = parse(blob.trim()).map_err(|err| {
        AsrError::new(format!(
            "vosk result was not JSON ({err}). No transcript was invented."
        ))
    })?;
    let text = clean_transcript(value.get("text").and_then(Json::as_str).unwrap_or(""));
    let mut words = Vec::new();
    if let Some(arr) = value.get("result").and_then(Json::as_arr) {
        for item in arr {
            let Some(word) = item.get("word").and_then(Json::as_str) else {
                continue;
            };
            if word == "[unk]" || word == "<unk>" {
                continue;
            }
            words.push(Word {
                word: word.to_string(),
                start: item.get("start").and_then(Json::as_f64).unwrap_or(0.0) as f32,
                end: item.get("end").and_then(Json::as_f64).unwrap_or(0.0) as f32,
                conf: item.get("conf").and_then(Json::as_f64).unwrap_or(0.0) as f32,
            });
        }
    }
    Ok((text, words))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_words_and_drops_unk() {
        let blob = r#"{"result":[{"conf":0.87,"end":0.45,"start":0.12,"word":"hello"},{"conf":1.0,"end":0.9,"start":0.45,"word":"[unk]"}],"text":"hello [unk]"}"#;
        let (text, words) = result_of(blob).unwrap();
        assert_eq!(text, "hello");
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].word, "hello");
        assert!((words[0].conf - 0.87).abs() < 1e-6);
    }
}
