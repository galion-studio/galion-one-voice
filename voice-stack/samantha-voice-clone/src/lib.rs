// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Samantha s711 voice clone.
//!
//! This crate is the voice-stack TTS backend. It speaks by driving the repo
//! Chatterbox worker (`scripts/clone/chatterbox_worker.py`) through
//! `python/s711_worker.py`. The s711 clip is not the CC0 pin in
//! `scripts/clone/REFS.sha256`, and that process refuses any device other
//! than CPU, so the expressive generate path lives in the sector worker.
//! Framing, sha256, and PCM writes are the repo module.
//!
//! Output is 24 kHz 16-bit mono, raw. The reference wav is read from
//! `GALION_SAMANTHA_REF` and must sha256-prefix `10ac44dc`. Nothing here
//! downloads a model or writes a wav into the repo.

mod cache;
mod consent;
mod device;
mod error;
mod frames;
mod recipe;
mod reference;
mod samantha;
mod tts;
mod worker;

pub use cache::{cache_dir_is_inside_repo, PhraseCache};
pub use consent::{Consent, CONSENT_SCOPE};
pub use device::{probe_compute, python_bin, resolve_python, ComputeKind, ComputeProbe};
pub use error::TtsError;
pub use frames::{frames_of, tail_of, FRAME_MS, FRAME_SAMPLES, SAMPLE_RATE_HZ};
pub use recipe::{
    plan_sentences, sentences, sha_prefix_ok, SentenceJob, CFG, EXAGGERATION, MODE, REF_SHA_PREFIX,
    SEED_BASE,
};
pub use reference::{sha256_file, sha256_hex, Reference};
pub use samantha::SamanthaTts;
pub use tts::{FakeTts, Pad, SentenceSpeaker, Speech, Spoken, Tts};
pub use worker::{
    parse_self_check, parse_worker_log, repo_chatterbox_worker, run_repo_stub_probe,
    run_self_check, s711_worker_script, RepoStubProbe, SelfCheck, WorkerLog, WorkerSpeaker,
};
