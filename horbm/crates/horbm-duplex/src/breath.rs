// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Breath port stand-in.
//!
//! The breath crate is still an open change. This cue is exhale while the
//! me band is speaking, and inhale otherwise. It writes no samples.

use crate::ports::{Breath, BreathCue, BreathPhase};

/// Exhale on speech, inhale otherwise. Not `plan_speech`.
#[derive(Clone, Copy, Debug, Default)]
pub struct SpeakingBreath;

impl Breath for SpeakingBreath {
    fn cue(&mut self, _t_ms: u32, speaking: bool) -> BreathCue {
        if speaking {
            BreathCue {
                phase: BreathPhase::Exhale,
                speaking: true,
            }
        } else {
            BreathCue {
                phase: BreathPhase::Inhale,
                speaking: false,
            }
        }
    }
}
