// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! HORBM layer 3, Empathy.
//!
//! Empathy is a weight on tone and prosody. It is not a sentence and not a
//! ninth emotion label. The pipeline calls [`Empathy::tick`] once per 80 ms
//! (`LOCKS.toml` `frame_ms`). The layer 5 router calls
//! [`Empathy::prepare_lookup`] before a lookup. Rest matches
//! `voice-stack/local-server/PROTOCOL.md`: arousal 0.3, valence 0.25,
//! dominance 0.5.
//!
//! Mood is arousal, valence, dominance. It rests at the HORBM pad
//! (0.3, 0.25, 0.5) and slews toward a policy target fed by
//! [`galion_emotion_in`] (PR 230) and the ASR lexicon in `galion-emotion`.
//! The weight `e` is [`galion_emotion::empathy_weight`]. Joy, calm, and a
//! low-confidence frame stay at 0. Anger is answered, not copied.
//! [`galion_sam_prosody::apply_empathy`] turns `e` into speaking knobs.
//! A feeling is named only when the caller invites it, and the name is one
//! word, not a line.
//!
//! CPU only. No paid API, no LLM, no audio, no network.
//!
//! ```
//! use galion_sam_empathy::{Empathy, Intent, PronounClass, TickIn};
//!
//! let mut layer = Empathy::new();
//! layer.names_mut().note("Maciej", PronounClass::He);
//! assert_eq!(layer.rewrite_query("ask him"), "ask Maciej");
//! let out = layer.tick(TickIn {
//!     inbound: None,
//!     asr: "I am so tired",
//!     asr_final: true,
//!     invite_name: false,
//!     frame_ms: 80,
//! });
//! assert!(out.e > 0.0);
//! assert!(out.mood.valence > 0.0);
//! assert!(out.named.is_none());
//! assert_eq!(out.intent, Intent::Vent);
//! assert_eq!(out.prosody.e, out.e);
//! ```

#![forbid(unsafe_code)]

mod intent;
mod layer;
mod mood;
mod names;
mod paths;
mod rewrite;
mod scan;
mod wants;

pub use galion_sam_prosody::EmpathyProsody;
pub use intent::{detect_intent, Intent};
pub use layer::{
    Empathy, LookupPrep, PipelineLayer, RouterLayer, RouterView, TickIn, TickOut, LAYER,
};
pub use mood::{FeelingName, InboundSignal, MoodPoint, FRAME_MS, MOOD_ALPHA, MOOD_BUDGET_US};
pub use names::{NameBook, NameSource, PronounClass};
pub use paths::{
    galion_home, reject_foreign_windows_drive, require_g_drive, windows_galion_home, PathReject,
    WINDOWS_GALION_HOME,
};
pub use rewrite::rewrite_query;
pub use wants::{Want, WantState};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_clock_matches_the_voice_heartbeat_and_canned_lines_stay_off() {
        assert_eq!(LAYER, 3);
        assert_eq!(FRAME_MS, 80);
        assert_eq!(FRAME_MS, galion_emotion::rika::FRAME_MS);
        assert_eq!(FRAME_MS, galion_emotion_in::FRAME_MS);
        assert!(!galion_emotion::CANNED_EMPATHY);
        assert_eq!(MoodPoint::RESTING.arousal, 0.3);
        assert_eq!(MoodPoint::RESTING.valence, 0.25);
        assert_eq!(MoodPoint::RESTING.dominance, 0.5);
        assert_eq!(MOOD_BUDGET_US, 80_000);
    }

    #[test]
    fn a_feeling_word_has_no_space_and_is_withheld_until_invited() {
        for name in [
            FeelingName::Sad,
            FeelingName::Tired,
            FeelingName::Anxious,
            FeelingName::Angry,
            FeelingName::Calm,
            FeelingName::Joy,
            FeelingName::Excited,
        ] {
            assert!(!name.word().contains(' '));
            assert!(!name.word().contains('.'));
        }
        let mut layer = Empathy::new();
        let quiet = layer.tick(TickIn {
            inbound: None,
            asr: "I am angry",
            asr_final: false,
            invite_name: false,
            frame_ms: 80,
        });
        assert!(quiet.e > 0.0);
        assert!(quiet.named.is_none());
        assert!(quiet.mood.valence > 0.0);
        let invited = layer.tick(TickIn {
            inbound: None,
            asr: "I am angry",
            asr_final: false,
            invite_name: true,
            frame_ms: 80,
        });
        assert_eq!(invited.named, Some(FeelingName::Angry));
        let asked = Empathy::new().tick(TickIn {
            inbound: None,
            asr: "how do I feel, I am anxious",
            asr_final: false,
            invite_name: false,
            frame_ms: 80,
        });
        assert_eq!(asked.named, Some(FeelingName::Anxious));
        let unknown = Empathy::new().tick(TickIn {
            inbound: None,
            asr: "how do I feel",
            asr_final: false,
            invite_name: true,
            frame_ms: 80,
        });
        assert!(unknown.named.is_none());
    }
}
