// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! S01 audio I/O for the Samantha voice stack.
//!
//! Microphone in, speaker out, on the CPU. No model and no network.
//!
//! Capture matches the Vosk worker and the one-dot page: 16 kHz mono,
//! 20 ms, 320 PCM16 samples (`crates/galion-asr-vosk/src/protocol.rs`,
//! `crates/galion-console/assets/v1-screen.html`). Playback matches the
//! heartbeat: 24 kHz (`crates/galion-heartbeat/src/lib.rs`, 80 ms = 1920
//! samples). Piper's 22050 Hz stream is resampled on the way in, the same
//! rate pair `archive/2026-11-24/crates/galion-v2-live` already uses.
//!
//! The playback ring is lock-free. `Playback::barge_in` drops every sample
//! queued at that instant. The next render period is silence. Samples pushed
//! after `barge_in` returns stay queued. Gain is a separate atomic so the
//! heartbeat duck (`DUCK_GAIN` 0.35 at 80 ms of overlap) can land without a
//! flush. The 200 ms silence in `galion-heartbeat`'s mixer is this flush.
//!
//! Device hosts: WASAPI on Windows, ALSA on Linux. PulseAudio and PipeWire
//! are reached through the ALSA `default` plugin when that plugin is
//! installed. cpal has no Pulse host of its own.
//!
//! The audio callback does not allocate on the steady path. There is no
//! sound-card requirement for the tests: a file source and an in-process
//! loopback cover the capture, resample, and flush paths.

#![deny(unsafe_code)]

#[allow(unsafe_code)]
mod ring;

mod bus;
mod capture;
mod convert;
mod device;
mod loopback;
mod measure;
mod playback;
mod resample;
mod wav;

pub use bus::{AudioIn, AudioOut, HeartbeatIn, PcmFrame};
pub use capture::{CaptureFrame, CaptureQueue, FileCapture};
pub use convert::{f32_to_i16, i16_to_f32};
pub use device::{
    available_host_names, compiled_host, host_report, open, pulse_plugin_present, AudioSession,
    Direction, HostReport, ListedDevice, SessionConfig, SessionInfo,
};
pub use loopback::Loopback;
pub use measure::{
    full_json, measure_all, measure_live, measure_software, FullReport, LiveReport, SoftwareReport,
};
pub use playback::{Playback, RenderResult, Renderer};
pub use resample::{resample_linear, BoxResampler};
pub use wav::{read_wav, write_wav, PcmWav};

/// Microphone rate. Same constant as `galion_asr_vosk::SAMPLE_RATE`.
pub const CAPTURE_HZ: u32 = 16_000;
/// One VAD/ASR frame. Same hop as `galion-v2-live` (`FRAME` 320 at 16 kHz).
pub const CAPTURE_FRAME_MS: u32 = 20;
/// 20 ms at 16 kHz. Same value as `galion_asr_vosk::FRAME_SAMPLES`.
pub const CAPTURE_FRAME_SAMPLES: usize = (CAPTURE_HZ as usize * CAPTURE_FRAME_MS as usize) / 1_000;
/// Speaker ring rate. Same constant as `galion_heartbeat::SAMPLE_RATE`.
pub const PLAYBACK_HZ: u32 = 24_000;
/// Heartbeat period. `galion_heartbeat::FRAME_MS`.
pub const PLAYBACK_FRAME_MS: u32 = 80;
/// 80 ms at 24 kHz. `galion_heartbeat::FRAME_SAMPLES` is 1920.
pub const PLAYBACK_FRAME_SAMPLES: usize =
    (PLAYBACK_HZ as usize * PLAYBACK_FRAME_MS as usize) / 1_000;
/// Piper's pinned rate (`galion_v2_live::piper::PIPER_HZ`). Not negotiated.
pub const PIPER_HZ: u32 = 22_050;
/// Duck applied by the heartbeat mixer at 80 ms of overlap.
/// `galion_heartbeat::mixer::DUCK_GAIN`.
pub const DUCK_GAIN: f32 = 0.35;

const _: () = assert!(CAPTURE_FRAME_SAMPLES == 320);
const _: () = assert!(PLAYBACK_FRAME_SAMPLES == 1_920);
const _: () = assert!(CAPTURE_FRAME_MS * 50 == 1_000);
const _: () = assert!(PLAYBACK_FRAME_MS * 125 == 10_000);

/// Recoverable audio-path failure. Device absence is an error from [`open`]
/// and a field on [`HostReport`], not a panic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioError {
    Host(String),
    Device(String),
    Wav(String),
    /// A sample rate of zero cannot be resampled.
    Rate,
    Frame {
        got: usize,
        expect: usize,
    },
    /// The playback ring did not have room for the whole block.
    Overrun,
}

impl std::fmt::Display for AudioError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Host(s) | Self::Device(s) | Self::Wav(s) => f.write_str(s),
            Self::Rate => f.write_str("sample rate is 0"),
            Self::Frame { got, expect } => {
                write!(f, "frame has {got} samples, expected {expect}")
            }
            Self::Overrun => f.write_str("playback ring is full"),
        }
    }
}

impl std::error::Error for AudioError {}

/// Monotonic nanoseconds since the first call in this process.
/// Used for flush-honor timing. Not the 80 ms heartbeat clock.
pub fn mono_ns() -> u64 {
    use std::sync::OnceLock;
    use std::time::Instant;
    static ORIGIN: OnceLock<Instant> = OnceLock::new();
    ORIGIN
        .get_or_init(Instant::now)
        .elapsed()
        .as_nanos()
        .min(u128::from(u64::MAX)) as u64
}
