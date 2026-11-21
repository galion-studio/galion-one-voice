// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Shared types for the Swarm 16 Samantha path.
//!
//! This crate does not open a microphone, a speaker, a model, or a socket.
//! It does not vendor crate `horbm`. The turn fields match the owner's
//! interface; `Intent` and `Arm` stay generic because that crate is not in
//! this tree and this file does not invent their variants.
//!
//! Normative wiring, the socket, and the latency ceilings are in
//! `voice-stack/ARCHITECTURE.md`.

pub mod audio;
pub mod brain;
pub mod budget;
pub mod speak;
pub mod ws;

pub use audio::{AudioError, AudioIn, AudioOut, EndpointEvent, PcmFrame, VadStage};
pub use brain::{Brain, BrainPaths, Pad, TurnOut};
pub use budget::{StageBudget, STAGE_BUDGETS};
pub use galion_emotion::express::ProsodyControl;
pub use galion_emotion::frame::EmotionFrame;
pub use galion_emotion::persona::CANNED_EMPATHY;
pub use galion_emotion::persona_mood::SAMANTHA;
pub use galion_emotion::rika::FRAME_MS;
pub use speak::{clone_tags_ok, contains_bracket_tag, is_scripted_comfort, may_speak_piper};

/// Pipeline rate. Same 24 kHz window `galion-emotion` documents for one frame.
pub const SAMPLE_RATE_HZ: u32 = 24_000;
/// Samples in one 80 ms frame at [`SAMPLE_RATE_HZ`]. 24000 * 80 / 1000.
pub const FRAME_SAMPLES: usize = 1_920;
/// Vosk phone rate. `galion_asr_vosk::SAMPLE_RATE`.
pub const ASR_SAMPLE_RATE_HZ: u32 = 16_000;
/// 20 ms at 16 kHz. `galion_asr_vosk::FRAME_SAMPLES`.
pub const ASR_FRAME_SAMPLES: usize = 320;

/// Loopback socket for this stack. Not 7878 (one-dot HTTP), 7879 (hard loop),
/// 7880 (clone worker), 8642 (Hermes), or 8766 (nebula feed).
pub const SAMANTHA_WS_ADDR: &str = "127.0.0.1:7881";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_matches_the_lock_and_the_frame_size() {
        assert_eq!(FRAME_MS, 80);
        assert_eq!(SAMPLE_RATE_HZ * FRAME_MS / 1000, FRAME_SAMPLES as u32);
        assert_eq!(FRAME_SAMPLES, 1_920);
        assert_eq!(ASR_SAMPLE_RATE_HZ * 20 / 1000, ASR_FRAME_SAMPLES as u32);
    }

    #[test]
    fn canned_empathy_stays_off_and_energy_does_not_fall_to_a_whisper() {
        // The lock is a const. The assertion is here so flipping it fails this test.
        #[allow(clippy::assertions_on_constants)]
        {
            assert!(!CANNED_EMPATHY);
        }
        assert!(SAMANTHA.voice.energy_bounds.0 >= 0.6);
        let ctrl = ProsodyControl {
            energy: 0.05,
            ..ProsodyControl::default()
        };
        assert!(ctrl.clamped().energy >= 0.6);
    }
}
