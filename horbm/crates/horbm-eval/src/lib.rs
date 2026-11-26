// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Closed evaluation gate for the HORBM layers that are on main.
//!
//! The default mode is the closed fake gate: a scripted turn is text plus,
//! where the ear or the mouth needs a frame, a generated 16 kHz tone. No
//! voice recording and no dataset text. [`horbm_voice::FakeEar`] and
//! [`horbm_voice::FakeMouth`] stand in behind [`horbm_voice::Ear`] and
//! [`horbm_voice::Mouth`]. Time for that latency score is the virtual frame
//! clock.
//!
//! [`Mode::Real`] is opt-in (`--features real`, then `--real`). On Linux it
//! puts `galion-asr-vosk` behind the ear trait and the Piper/Flite worker in
//! `galion-ml-voice` behind the mouth trait. The mouth renders the same seven
//! scripted lines, the ear hears that PCM, and the word error rate is the
//! round trip. Latency on that path is wall clock. Models download into a
//! cache directory and are not committed. A missing model skips the run.
//!
//! Answers and citations come from the truth-root cards only.
//! `GALION_HORBM_RESEARCH_DIR` must be unset; the runner does not open it.
//! Research stays the conductor's offline client. The persona is loaded at
//! run time from `--persona`, `Persona::load_development`, or an explicit
//! persona file. Ear prosody sensing stays on unless `--no-prosody` or
//! `GALION_EAR_PROSODY=0`.
//!
//! The emotion frame budget is the 80 ms lock, measured around
//! [`horbm::Brain::step_moment`].
//!
//! [`run_stand_in`] is the separate full-duplex turn-taking eval. It drives
//! [`horbm_duplex::Duplex`] on the virtual 80 ms clock. User speech is
//! generated in memory. Metrics follow Full-Duplex-Bench (arXiv:2503.04721)
//! and the overlap timing in v1.5 (arXiv:2507.23159). A stage that is still
//! an open pull request is reported as `stub` and is not gated.

#![forbid(unsafe_code)]

mod drive;
mod echo;
mod gate;
mod percentile;
mod prosody;
mod real;
mod scenario;
mod score;
mod speech;
mod thresholds;
mod turn;

pub use gate::{run, run_with, Config, Mode, Report};
pub use horbm_duplex::FRAME_SAMPLES;
pub use prosody::ear_prosody_enabled;
pub use score::{
    score_emotion, score_grounding, score_latency, score_persona, score_unknown, score_wer,
    EmotionSample, LatencySample, Spoken, VoiceTurn,
};
pub use thresholds::Thresholds;
pub use turn::{Report as TurnReport, StageState};

/// Process environment. A non-empty value fails the gate. The path is not opened.
pub fn research_dir_from_env() -> Option<String> {
    std::env::var("GALION_HORBM_RESEARCH_DIR")
        .ok()
        .filter(|value| !value.trim().is_empty())
}

pub fn research_dir_is_unset(value: Option<&str>) -> bool {
    value.map(|raw| raw.trim().is_empty()).unwrap_or(true)
}

/// Run the closed turn-taking set with the in-memory stand-in.
/// Piper is not invoked: this tree has no voice model, and the eval does not download one.
///
/// The report names [`stand_in_persona_label`]. The duplex engine in `drive`
/// is unchanged.
#[must_use]
pub fn run_stand_in() -> TurnReport {
    run_stand_in_for(None)
}

/// Label for a stand-in turn report that was not given a persona file.
///
/// `GALION_HORBM_DEV_PERSONA` when that value is non-empty. Otherwise
/// `configured`. The id inside `development.toml` is not copied into the
/// report: that string is a persona-file stem, and the no-leak check forbids
/// those stems.
#[must_use]
pub fn stand_in_persona_label() -> String {
    match std::env::var("GALION_HORBM_DEV_PERSONA") {
        Ok(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                "configured".to_string()
            } else {
                trimmed.to_string()
            }
        }
        Err(_) => "configured".to_string(),
    }
}

/// [`run_stand_in`] naming `persona_file`, or [`stand_in_persona_label`] when
/// `persona_file` is `None`.
///
/// The duplex engine in `drive` still uses its built-in speaker. This only
/// names the report. A file that does not load is `unconfigured` and a miss.
#[must_use]
pub fn run_stand_in_for(persona_file: Option<&std::path::Path>) -> TurnReport {
    let (persona_id, load_error) = match gate::resolve_persona(persona_file) {
        Ok(persona) => {
            let id = match persona_file {
                Some(_) => persona.id().to_string(),
                None => stand_in_persona_label(),
            };
            (id, None)
        }
        Err(err) => ("unconfigured".to_string(), Some(err)),
    };
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
    let mut report = turn::score(&trials, "stand-in", &persona_id);
    if let Some(err) = load_error {
        report.misses.push(err);
    }
    report
}

#[cfg(test)]
mod tests {
    use horbm_duplex::{open_persona, Persona};

    #[test]
    fn another_id_is_refused() {
        let configured = horbm_duplex::ConfiguredPersona::development().expect("configured");
        assert!(open_persona(configured.id()).is_ok());
        assert!(open_persona("northwind").is_err());
    }
}
