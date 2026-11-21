// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Clock and bus sizes copied from the locks this loop has to share.
//!
//! `HEARTBEAT_MS` is `LOCKS.toml` `[clock] frame_ms` (80). The capture quantum
//! is the Vosk phone frame: 20 ms, 16 kHz, 320 samples
//! (`galion-asr-vosk` `protocol::FRAME_SAMPLES`). Playback is the me-band
//! rate: 24 kHz, 1920 samples per heartbeat (`galion-heartbeat`).
//! `BARGE_IN_MAX_MS` is `LOCKS.toml` `[gates] barge_in_max_ms`.
//!
//! This crate does not call `Heartbeat::step`. That step never blocks and
//! never allocates. Mood ticks on the same 80 ms period, beside the step.

/// Heartbeat period. Locked.
pub const HEARTBEAT_MS: u32 = 80;
/// Locked cadence in tenths of a hertz. 125 means 12.5 Hz.
pub const HEARTBEAT_HZ_TENTHS: u32 = 125;
/// You-band / Vosk rate.
pub const CAPTURE_RATE_HZ: u32 = 16_000;
/// Me-band / Piper / heartbeat rate.
pub const PLAYBACK_RATE_HZ: u32 = 24_000;
/// One Vosk frame.
pub const CAPTURE_FRAME_MS: u32 = 20;
/// Samples in one capture frame.
pub const CAPTURE_FRAME_SAMPLES: usize =
    (CAPTURE_RATE_HZ as usize * CAPTURE_FRAME_MS as usize) / 1000;
/// Capture samples in one heartbeat (4 Vosk frames).
pub const HEARTBEAT_CAPTURE_SAMPLES: usize =
    (CAPTURE_RATE_HZ as usize * HEARTBEAT_MS as usize) / 1000;
/// Playback samples in one heartbeat.
pub const HEARTBEAT_PLAY_SAMPLES: usize =
    (PLAYBACK_RATE_HZ as usize * HEARTBEAT_MS as usize) / 1000;
/// Yield on user barge-in. Locked ceiling, not a measured speaker time.
pub const BARGE_IN_MAX_MS: u32 = 200;
/// Per-stage streaming bar the fake loop is tested against, in microseconds.
pub const STAGE_BAR_US: u32 = 80_000;
/// Mic frames queued between capture and the endpointer. 8 × 20 ms = 160 ms.
pub const FRAME_CHANNEL_BOUND: usize = 8;
/// One pending utterance. A newer end-of-turn replaces the overflow slot.
pub const TURN_CHANNEL_BOUND: usize = 1;
/// Sentence chunks waiting on the speaker. Barge-in does not wait on this queue.
pub const CHUNK_CHANNEL_BOUND: usize = 4;
/// Same ceiling as the Vosk worker: close the turn instead of growing forever.
pub const MAX_UTTERANCE_MS: u32 = 60_000;
/// Default end-of-turn hang. Two heartbeats of silence.
pub const VAD_HANG_MS: f64 = 160.0;
/// Same minimum voiced run as `galion-v2-live` `Vad`.
pub const VAD_MIN_VOICED_MS: f64 = 60.0;

const _: () = assert!(HEARTBEAT_MS == 80);
const _: () = assert!(HEARTBEAT_MS * HEARTBEAT_HZ_TENTHS == 10_000);
const _: () = assert!(CAPTURE_FRAME_SAMPLES == 320);
const _: () = assert!(HEARTBEAT_CAPTURE_SAMPLES == 1_280);
const _: () = assert!(HEARTBEAT_PLAY_SAMPLES == 1_920);
const _: () = assert!(BARGE_IN_MAX_MS == 200);
const _: () = assert!(STAGE_BAR_US == HEARTBEAT_MS * 1000);
