// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! V2 live-call core map. The table and the pinning live in the std-only
//! `galion-cores` crate so the Vosk launcher, the call loop and `galion-pin`
//! share one map. Default when on: **v3**: bus + retrieval 0, Vosk 1,
//! in-house reasoning (S0 on galion-tensor) 2-5, Piper Samantha 6, Piper
//! Jarvis 7 (see `galion_cores`). `GALION_CORE_MAP=v2` keeps the
//! old one-role-per-core table for A/B.
pub use galion_cores::*;
