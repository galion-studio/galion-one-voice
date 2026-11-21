// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! End-to-end voice quality gates for Samantha (swarm 16, sector S13).
//!
//! Reuses the rollout numbers already locked in the emotion harness and
//! recorded by `galion-listen-back`, the Piper command from the A21 worker,
//! and the Vosk feeding loop from `wer_t648.py` / `galion-asr-vosk`.
//! HORBM is a trait. The fake engine is what tests call. The real crate is
//! not linked and is not reimplemented.

pub mod brain;
pub mod gate;
pub mod golden;
pub mod jsonutil;
pub mod lint;
pub mod locate;
pub mod piper;
pub mod report;
pub mod roundtrip;
pub mod vosk;
pub mod wer;
