// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

use std::path::PathBuf;

use crate::engine::Asr;
use crate::error::AsrError;
use crate::fake::FakeAsr;
use crate::paths::{self, env_u32};
use crate::vosk::{VoskAsr, VoskConfig};
use crate::worker::VoskWorkerAsr;

pub struct EngineOpts {
    pub backend: String,
    pub model: Option<PathBuf>,
    pub lib: Option<PathBuf>,
    pub bin: Option<PathBuf>,
    pub threads: Option<u32>,
    /// Fake backend only. The scripted transcript. Not used as a measurement.
    pub fake_transcript: Option<String>,
}

impl EngineOpts {
    pub fn backend(name: impl Into<String>) -> Self {
        Self {
            backend: name.into(),
            model: None,
            lib: None,
            bin: None,
            threads: None,
            fake_transcript: None,
        }
    }
}

pub fn open_engine(opts: &EngineOpts) -> Result<(Box<dyn Asr>, String, u32), AsrError> {
    match opts.backend.as_str() {
        "fake" => {
            let text = opts.fake_transcript.clone().unwrap_or_default();
            Ok((Box::new(FakeAsr::new(&text)), "fake".into(), 0))
        }
        "vosk" => {
            let mut config = VoskConfig::from_env()?;
            if let Some(model) = &opts.model {
                config.model_dir = model.clone();
            }
            if let Some(lib) = &opts.lib {
                config.lib_path = Some(lib.clone());
            }
            if let Some(threads) = opts.threads {
                config.threads = threads;
            }
            let model = config.model_dir.display().to_string();
            let threads = config.threads;
            let engine = VoskAsr::open(config)?;
            Ok((Box::new(engine), model, threads))
        }
        "vosk-worker" => {
            let bin = match &opts.bin {
                Some(path) => path.clone(),
                None => paths::find_vosk_bin().ok_or_else(|| {
                    AsrError::new(
                        "galion-asr-vosk binary not found. Set GALION_VOSK_BIN or build -p galion-asr-vosk. No transcript was invented.",
                    )
                })?,
            };
            let model = match &opts.model {
                Some(path) => path.clone(),
                None => paths::find_vosk_model()?,
            };
            let lib = opts.lib.clone().or_else(paths::find_vosk_lib);
            let threads = match opts.threads {
                Some(threads) => {
                    std::env::set_var("GALION_VOSK_THREADS", threads.to_string());
                    threads
                }
                None => env_u32("GALION_VOSK_THREADS", 2)?,
            };
            let label = model.display().to_string();
            let engine = VoskWorkerAsr::open(&bin, &model, lib.as_deref())?;
            Ok((Box::new(engine), label, threads))
        }
        other => Err(AsrError::new(format!(
            "unknown backend {other}. Use vosk, vosk-worker, or fake. No transcript was invented."
        ))),
    }
}
