// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Optional whisper.cpp backend (whisper-rs). CPU only: `use_gpu` is false and
//! the Cargo feature does not turn on CUDA, Metal, Vulkan, or HIP.
//!
//! whisper.cpp decodes a buffer. It is not a 20 ms Kaldi recognizer. `push_pcm`
//! only stores audio unless `partial_every_samples` is set, in which case each
//! window runs a full decode. `finish` runs one decode and returns the final.

use std::path::{Path, PathBuf};

use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::engine::Asr;
use crate::error::AsrError;
use crate::event::{samples_to_ms, AsrEvent, AsrKind};
use crate::paths::env_u32;
use crate::text::clean_transcript;

pub struct WhisperConfig {
    pub model_path: PathBuf,
    pub threads: u32,
    /// When set, `push_pcm` re-decodes the whole buffer every this many samples.
    /// Leave `None` for one decode on `finish`.
    pub partial_every_samples: Option<usize>,
}

impl WhisperConfig {
    pub fn from_env(which: &str) -> Result<Self, AsrError> {
        Ok(Self {
            model_path: crate::paths::find_whisper_model(which)?,
            threads: env_u32("GALION_WHISPER_THREADS", default_threads())?,
            partial_every_samples: partial_from_env()?,
        })
    }
}

fn default_threads() -> u32 {
    let n = std::thread::available_parallelism()
        .map(|n| n.get() as u32)
        .unwrap_or(2);
    n.clamp(1, 4)
}

fn partial_from_env() -> Result<Option<usize>, AsrError> {
    match std::env::var("GALION_WHISPER_PARTIAL_MS") {
        Err(_) => Ok(None),
        Ok(value) if value.is_empty() || value == "0" => Ok(None),
        Ok(value) => {
            let ms: u32 = value.parse().map_err(|_| {
                AsrError::new(format!(
                    "GALION_WHISPER_PARTIAL_MS={value} is not an integer. No transcript was invented."
                ))
            })?;
            Ok(Some(
                crate::event::SAMPLE_RATE as usize * ms as usize / 1000,
            ))
        }
    }
}

pub struct WhisperAsr {
    ctx: WhisperContext,
    pending: Vec<i16>,
    fed: usize,
    since_partial: usize,
    last_partial: String,
    partial_every: Option<usize>,
    threads: i32,
    label: String,
    model_path: PathBuf,
}

impl WhisperAsr {
    pub fn open(config: WhisperConfig) -> Result<Self, AsrError> {
        if !config.model_path.is_file() {
            return Err(AsrError::new(format!(
                "whisper model {} is not a file. No transcript was invented.",
                config.model_path.display()
            )));
        }
        let path = config.model_path.to_str().ok_or_else(|| {
            AsrError::new("whisper model path is not utf-8. No transcript was invented.")
        })?;
        let mut params = WhisperContextParameters::default();
        params.use_gpu = false;
        let ctx = WhisperContext::new_with_params(path, params).map_err(|err| {
            AsrError::new(format!(
                "whisper model failed to load ({err}). No transcript was invented."
            ))
        })?;
        let file_name = config
            .model_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("whisper");
        Ok(Self {
            ctx,
            pending: Vec::new(),
            fed: 0,
            since_partial: 0,
            last_partial: String::new(),
            partial_every: config.partial_every_samples,
            threads: i32::try_from(config.threads.max(1)).unwrap_or(1),
            label: format!("whisper:{file_name}"),
            model_path: config.model_path,
        })
    }

    pub fn model_path(&self) -> &Path {
        &self.model_path
    }

    pub fn threads(&self) -> u32 {
        self.threads as u32
    }

    fn transcribe(&mut self, pcm: &[i16]) -> Result<String, AsrError> {
        if pcm.is_empty() {
            return Ok(String::new());
        }
        let samples: Vec<f32> = pcm.iter().map(|s| *s as f32 / 32768.0).collect();
        let mut state = self.ctx.create_state().map_err(|err| {
            AsrError::new(format!("whisper state: {err}. No transcript was invented."))
        })?;
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_n_threads(self.threads);
        params.set_language(Some("en"));
        params.set_translate(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_special(false);
        params.set_print_timestamps(false);
        params.set_no_context(true);
        params.set_no_timestamps(true);
        state.full(params, &samples).map_err(|err| {
            AsrError::new(format!(
                "whisper decode failed ({err}). No transcript was invented."
            ))
        })?;
        let mut parts = Vec::new();
        let n = state.full_n_segments();
        for i in 0..n {
            if let Some(segment) = state.get_segment(i) {
                let text = segment.to_str().map_err(|err| {
                    AsrError::new(format!(
                        "whisper segment: {err}. No transcript was invented."
                    ))
                })?;
                let cleaned = clean_transcript(text);
                if !cleaned.is_empty() {
                    parts.push(cleaned);
                }
            }
        }
        Ok(parts.join(" "))
    }
}

impl Asr for WhisperAsr {
    fn push_pcm(&mut self, pcm: &[i16]) -> Result<Vec<AsrEvent>, AsrError> {
        if pcm.is_empty() {
            return Ok(Vec::new());
        }
        self.pending.extend_from_slice(pcm);
        self.fed = self.fed.saturating_add(pcm.len());
        self.since_partial = self.since_partial.saturating_add(pcm.len());
        let Some(every) = self.partial_every else {
            return Ok(Vec::new());
        };
        if every == 0 || self.since_partial < every {
            return Ok(Vec::new());
        }
        let pcm = self.pending.clone();
        let text = self.transcribe(&pcm)?;
        self.since_partial = 0;
        if text.is_empty() || text == self.last_partial {
            return Ok(Vec::new());
        }
        self.last_partial = text.clone();
        Ok(vec![AsrEvent {
            kind: AsrKind::Partial,
            text,
            audio_ms: samples_to_ms(self.fed),
            words: Vec::new(),
            why: "partial",
        }])
    }

    fn finish(&mut self) -> Result<Vec<AsrEvent>, AsrError> {
        if self.pending.is_empty() {
            return Ok(Vec::new());
        }
        let pcm = std::mem::take(&mut self.pending);
        let text = self.transcribe(&pcm)?;
        let audio_ms = samples_to_ms(self.fed);
        self.fed = 0;
        self.since_partial = 0;
        self.last_partial.clear();
        Ok(vec![AsrEvent {
            kind: AsrKind::Final,
            text,
            audio_ms,
            words: Vec::new(),
            why: "finish",
        }])
    }

    fn reset(&mut self) -> Result<(), AsrError> {
        self.pending.clear();
        self.fed = 0;
        self.since_partial = 0;
        self.last_partial.clear();
        Ok(())
    }

    fn name(&self) -> &str {
        &self.label
    }
}
