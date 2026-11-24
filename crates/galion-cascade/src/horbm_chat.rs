// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Chat turn answered by the HORBM heuristic pick.
//!
//! This is the slot [`crate::CannedLlm`] fills when `GALION_LLM=canned`.
//! `GALION_LLM=horbm` and [`HorbmChat`] answer from the committed clause
//! fixture instead. The canned table stays for the phone tests that name it.
//! Piper is not called here. A miss is the stored persona clause.

use horbm_brain::{Answer, Heuristic};

use crate::{CascadeError, Emotion, Llm, Transcript};

/// Stored-clause brain for one chat turn. No weights and no socket.
#[derive(Clone, Debug)]
pub struct HorbmChat {
    brain: Heuristic,
}

impl HorbmChat {
    pub fn fixture() -> Self {
        Self {
            brain: Heuristic::committed(),
        }
    }

    pub fn answer(&self, text: &str) -> Answer {
        self.brain.answer(text)
    }
}

impl Llm for HorbmChat {
    fn name(&self) -> &'static str {
        "horbm (heuristic clause pick; stored fixture; no weights)"
    }

    fn reply(&mut self, t: &Transcript, _e: Emotion) -> Result<String, CascadeError> {
        if t.text.trim().is_empty() {
            return Err(CascadeError::Empty);
        }
        // Emotion does not replace the clause. The line is the stored text.
        Ok(self.brain.answer(&t.text).clause)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Lang;
    use horbm_brain::{MarkKind, SidebandMark};

    fn tr(text: &str) -> Transcript {
        Transcript {
            text: text.into(),
            lang: Lang::En,
        }
    }

    /// Phase 4 gate. A chat turn produces the fixture clause. No network and
    /// no audio file: this test reads the committed JSON and calls the pick.
    #[test]
    fn a_chat_turn_produces_the_fixture_clause() {
        let mut chat = HorbmChat::fixture();
        let reply = chat
            .reply(&tr("where is the kettle"), Emotion::Neutral)
            .unwrap();
        assert_eq!(reply, "The kettle is on the hob.");
        let detail = chat.answer("where is the kettle");
        assert_eq!(detail.clause, reply);
        assert_eq!(detail.first, "The kettle");
        assert!(detail.first.split_whitespace().count() <= 2);
        assert_eq!(detail.rest, "is on the hob.");
        assert!(matches!(
            detail.mark,
            Some(SidebandMark {
                kind: MarkKind::Lookup { .. },
                posted: false,
            })
        ));
        let miss = chat.reply(&tr("who won the match"), Emotion::Sad).unwrap();
        assert_eq!(miss, "I don't know.");
        assert!(matches!(
            chat.reply(&tr("  "), Emotion::Neutral),
            Err(CascadeError::Empty)
        ));
    }
}
