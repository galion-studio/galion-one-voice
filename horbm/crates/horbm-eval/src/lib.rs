// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Closed full-duplex turn-taking eval for Samantha.
//!
//! The loop is [`horbm_duplex::Duplex`] on the virtual 80 ms clock. User
//! speech is generated in memory when the eval runs. No audio file is written.
//!
//! Metrics follow Full-Duplex-Bench (Lin et al., arXiv:2503.04721,
//! <https://github.com/DanielLin94144/Full-Duplex-Bench>) and the overlap
//! timing in Full-Duplex-Bench v1.5 (arXiv:2507.23159). A stage that is still
//! an open pull request is reported as `stub` and is not gated.
//!
//! # Safety
//!
//! No `unsafe`.

#![forbid(unsafe_code)]

mod drive;
mod echo;
mod percentile;
mod scenario;
mod score;
mod speech;

pub use horbm_duplex::FRAME_SAMPLES;
pub use score::{Report, StageState};

/// Run the closed set with the in-memory stand-in. Piper is not invoked:
/// this tree has no voice model, and the eval does not download one.
#[must_use]
pub fn run_stand_in() -> Report {
    let gaps = scenario::pause_gaps_ms();
    assert!(
        gaps.len() == 6 && gaps.iter().all(|gap| (400..=1_000).contains(gap)),
        "pause gaps left the Full-Duplex-Bench 400–1000 ms band"
    );
    assert!(speech::rms_db(&speech::silence()) < speech::EAR_FLOOR_DB);
    assert!(speech::rms_db(&speech::tone(180.0)) > speech::EAR_FLOOR_DB);
    assert!(speech::rms_db(&speech::falling()) > speech::EAR_FLOOR_DB);
    let scenarios = scenario::catalog();
    let trials = drive::run_all(&scenarios);
    score::score(&trials, "stand-in")
}

#[cfg(test)]
mod tests {
    use horbm_duplex::open_persona;

    #[test]
    fn aria_is_refused() {
        assert!(open_persona("aria").is_err());
        assert!(open_persona("samantha").is_ok());
    }
}
