// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! V2 live-call core map. The table and the pinning live in the std-only
//! `galion-cores` crate so the Vosk launcher, the call loop and `galion-pin`
//! (llama-server) share one map. Default when on: **v3**: LLM 2-5, Piper 6-7
//! (see `galion_cores` for the measured A/B). `GALION_CORE_MAP=v2` keeps the
//! old one-role-per-core table for A/B.
pub use galion_cores::*;
