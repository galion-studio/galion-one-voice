// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Ceilings the harness flags. None of these numbers were measured in this crate.
//!
//! `ceiling_us` is a limit, not a result. `None` means this table does not
//! put that stage under the 80 ms streaming bar (a wait, or off the live path).

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StageBudget {
    pub name: &'static str,
    /// Set when the stage's own compute has a ceiling. Microseconds.
    pub ceiling_us: Option<u32>,
    pub note: &'static str,
}

/// Hot path only. Clone, eval, install, and packaging are absent on purpose.
pub const STAGE_BUDGETS: &[StageBudget] = &[
    StageBudget {
        name: "audio_in",
        ceiling_us: Some(60_000),
        note: "On the heartbeat thread. LOCKS frame_budget_p95_ms = 60. The frame itself is 80 ms of audio.",
    },
    StageBudget {
        name: "vad_compute",
        ceiling_us: Some(80_000),
        note: "Decision on one 20 ms frame. Not the silence wait.",
    },
    StageBudget {
        name: "endpoint_hang",
        ceiling_us: None,
        note: "Wait, not compute. endpoint.rs default commit is 160 ms. BUDGET.md lists a 60 ms target for end-of-speech. This table does not change the default and does not claim the 60 ms target was met.",
    },
    StageBudget {
        name: "asr_final",
        ceiling_us: Some(80_000),
        note: "Swarm streaming bar. BUDGET.md budget for ASR final is 15 ms, baseline p50 19.9 ms, on an earlier stack. Those figures are not a result for this path.",
    },
    StageBudget {
        name: "horbm_turn",
        ceiling_us: Some(80_000),
        note: "Wall time of Brain::turn, beside TurnOut.took_us. Unmeasured here. Crate horbm is not in this tree.",
    },
    StageBudget {
        name: "horbm_lookup",
        ceiling_us: None,
        note: "Report TurnOut.lookup_us. R1's ≤5 ms p95 is an opinion on branch cursor/horbm-r1-retrieval-core-690c, not a lock and not a measurement.",
    },
    StageBudget {
        name: "prosody",
        ceiling_us: Some(80_000),
        note: "Pure map on the tick. LOCKS affect_to_voice_max_ms stays 160.",
    },
    StageBudget {
        name: "emotion_in",
        ceiling_us: Some(2_400),
        note: "LOCKS affect_overhead_max_pct = 3 of an 80 ms frame. Stricter than the swarm bar.",
    },
    StageBudget {
        name: "tts_first",
        ceiling_us: Some(80_000),
        note: "First PCM chunk after the clause is handed to the warm Piper. BUDGET.md p50 71.3 ms and budget 30 ms are earlier-stack figures.",
    },
    StageBudget {
        name: "playback",
        ceiling_us: Some(80_000),
        note: "Queue to device. BUDGET.md records playback handoff as 0 on that bench.",
    },
    StageBudget {
        name: "ws_write",
        ceiling_us: Some(80_000),
        note: "Loopback write of one socket message. Unmeasured here.",
    },
    StageBudget {
        name: "dot_present",
        ceiling_us: Some(1_000),
        note: "LOCKS present_max_ms = 1.",
    },
];

pub fn over_ceiling(name: &str, elapsed_us: u32) -> Option<bool> {
    let stage = STAGE_BUDGETS.iter().find(|s| s.name == name)?;
    Some(match stage.ceiling_us {
        Some(limit) => elapsed_us > limit,
        None => false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heartbeat_work_is_tighter_than_the_swarm_bar() {
        let audio = STAGE_BUDGETS.iter().find(|s| s.name == "audio_in").unwrap();
        assert_eq!(audio.ceiling_us, Some(60_000));
        let dot = STAGE_BUDGETS
            .iter()
            .find(|s| s.name == "dot_present")
            .unwrap();
        assert_eq!(dot.ceiling_us, Some(1_000));
        let hang = STAGE_BUDGETS
            .iter()
            .find(|s| s.name == "endpoint_hang")
            .unwrap();
        assert_eq!(hang.ceiling_us, None);
        assert_eq!(over_ceiling("endpoint_hang", 1_000_000), Some(false));
        assert_eq!(over_ceiling("tts_first", 80_000), Some(false));
        assert_eq!(over_ceiling("tts_first", 80_001), Some(true));
        assert_eq!(over_ceiling("no-such-stage", 1), None);
    }
}
