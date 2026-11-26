// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Finished ASR text through the layer 5 router, then the reply builder.
//!
//! The mouth after that is the pipeline `Tts` stage. Piper implements that
//! stage in `galion-tts-piper`. This module does not start Piper and does
//! not read a voice file. [`RouterMouth`] keeps the pad it was given. It
//! does not invent a mood.

use galion_sam_horbm::{speakable_chunks, ReasoningRouter, RouteOutcome};

use crate::voice_traits::{Pad, Reply, ReplyEngine};

/// Reply stage. `turn` is what the think loop calls after ASR finishes.
pub struct RouterMouth {
    router: ReasoningRouter,
}

impl RouterMouth {
    pub fn new(router: ReasoningRouter) -> Self {
        Self { router }
    }

    pub fn gaps(&self) -> &[galion_sam_horbm::GapEntry] {
        self.router.gaps()
    }

    pub fn router_mut(&mut self) -> &mut ReasoningRouter {
        &mut self.router
    }
}

impl ReplyEngine for RouterMouth {
    fn turn(&mut self, heard: &str, tick: Pad) -> Reply {
        let outcome = self.router.route(heard);
        reply_from(outcome, tick)
    }
}

fn reply_from(outcome: RouteOutcome, tick: Pad) -> Reply {
    Reply {
        text: outcome.spoken,
        mood: tick,
        intent: String::new(),
        arm: String::new(),
        propensity: 0.0,
        evidence: outcome.evidence,
        sources: outcome.sources,
        violations: 0,
        lookup_us: outcome.lookup_us,
        took_us: outcome.took_us,
    }
}

/// ASR text, then the router, then the speakable chunks Piper receives.
pub fn asr_to_piper_lines(router: &mut ReasoningRouter, heard: &str) -> Vec<String> {
    let outcome = router.route(heard);
    speakable_chunks(&outcome.spoken)
        .into_iter()
        .map(|chunk| chunk.text)
        .collect()
}
