// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Pure comparisons against the threshold file. The live run fills the samples.

use crate::thresholds::{
    EmotionThresholds, GroundingThresholds, LatencyThresholds, PersonaThresholds,
};
use serde::Serialize;

#[derive(Clone, Debug)]
pub struct Spoken {
    pub query: String,
    pub reply: String,
    pub sources: Vec<String>,
    pub sideband_posted: bool,
    pub expect_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct CardFact {
    pub id: String,
    pub gist: String,
}

#[derive(Clone, Debug)]
pub struct VoiceTurn {
    pub persona: String,
    pub rate: f32,
    pub energy: f32,
    pub pitch_st: f32,
    pub neutral: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct VoiceWindow {
    pub pitch_max: f32,
    pub rate_lo: f32,
    pub rate_hi: f32,
    pub energy_lo: f32,
    pub energy_hi: f32,
}

/// Rest (rate, energy, pitch) of the two voices. Neutral lines must sit
/// closer to Samantha than to Jarvis.
#[derive(Clone, Copy, Debug)]
pub struct VoiceAnchors {
    pub samantha: (f32, f32, f32),
    pub jarvis: (f32, f32, f32),
}

#[derive(Clone, Debug)]
pub struct EmotionSample {
    pub fatigue_drop: f32,
    pub valence_rise: f32,
    pub step_p95_us: u32,
    pub frame_ms: u32,
    pub off_clock_refused: bool,
    pub baseline_held: bool,
    pub cue_follows_moment: bool,
}

#[derive(Clone, Debug)]
pub struct LatencySample {
    pub eager_to_voice_ms: Vec<u32>,
    pub barge_in_ms: u32,
    pub backchannel_cancel: bool,
    pub clock_advanced_ms: u32,
    pub wall_sleep_ms: u32,
}

#[derive(Clone, Debug, Serialize)]
pub struct Check {
    pub pass: bool,
    pub failures: Vec<String>,
}

impl Check {
    fn ok() -> Self {
        Self {
            pass: true,
            failures: Vec::new(),
        }
    }

    fn push(&mut self, failure: impl Into<String>) {
        self.pass = false;
        self.failures.push(failure.into());
    }
}

pub fn score_grounding(
    turns: &[Spoken],
    pack: &[CardFact],
    forbidden: &[String],
    thresholds: &GroundingThresholds,
) -> Check {
    let mut check = Check::ok();
    let hits: Vec<_> = turns
        .iter()
        .filter(|turn| turn.expect_id.is_some())
        .collect();
    if hits.is_empty() {
        check.push("no grounded turn was scripted");
    }
    for turn in hits {
        let id = turn.expect_id.as_deref().unwrap_or("");
        let Some(card) = pack.iter().find(|card| card.id == id) else {
            check.push(format!(
                "{} expected card {id}, which is not in the truth root",
                turn.query
            ));
            continue;
        };
        let source = format!("{}{id}", thresholds.card_prefix);
        if turn.sources != vec![source] {
            check.push(format!(
                "{} sources {:?}, expected one real card",
                turn.query, turn.sources
            ));
        }
        if turn.reply != card.gist {
            check.push(format!(
                "{} reply {:?} is not the stored gist {:?}",
                turn.query, turn.reply, card.gist
            ));
        }
        if turn.sideband_posted {
            check.push(format!("{} posted a side band", turn.query));
        }
        note_forbidden(&mut check, turn, forbidden);
        note_invented(&mut check, turn, pack, thresholds);
    }
    check
}

pub fn score_unknown(
    turns: &[Spoken],
    pack: &[CardFact],
    forbidden: &[String],
    thresholds: &GroundingThresholds,
) -> Check {
    let mut check = Check::ok();
    let misses: Vec<_> = turns
        .iter()
        .filter(|turn| turn.expect_id.is_none())
        .collect();
    if misses.is_empty() {
        check.push("no miss was scripted");
    }
    for turn in misses {
        if turn.reply != thresholds.unknown_reply {
            check.push(format!(
                "{} reply {:?} is not {}",
                turn.query, turn.reply, thresholds.unknown_reply
            ));
        }
        if turn.sources != vec![thresholds.honest_source.clone()] {
            check.push(format!(
                "{} sources {:?}, expected {}",
                turn.query, turn.sources, thresholds.honest_source
            ));
        }
        if turn.sideband_posted {
            check.push(format!("{} posted a side band", turn.query));
        }
        note_forbidden(&mut check, turn, forbidden);
        note_invented(&mut check, turn, pack, thresholds);
    }
    check
}

fn note_forbidden(check: &mut Check, turn: &Spoken, forbidden: &[String]) {
    if forbidden.iter().any(|snippet| snippet == &turn.reply) {
        check.push(format!(
            "{} spoke a stand-in memory line, not a truth-root card",
            turn.query
        ));
    }
}

fn note_invented(
    check: &mut Check,
    turn: &Spoken,
    pack: &[CardFact],
    thresholds: &GroundingThresholds,
) {
    for source in &turn.sources {
        if source.starts_with(&thresholds.research_prefix) {
            check.push(format!("{} cited research ({source})", turn.query));
        }
        if let Some(id) = source.strip_prefix(&thresholds.card_prefix) {
            if !pack.iter().any(|card| card.id == id) {
                check.push(format!("{} invented card {id}", turn.query));
            }
        }
    }
}

pub fn score_persona(
    turns: &[VoiceTurn],
    window: VoiceWindow,
    anchors: VoiceAnchors,
    thresholds: &PersonaThresholds,
) -> Check {
    let mut check = Check::ok();
    if turns.is_empty() {
        check.push("no voice turn");
        return check;
    }
    let mut neutral: Option<(u32, u32, u32)> = None;
    for turn in turns {
        if turn.persona != thresholds.id {
            check.push(format!("persona {} is not {}", turn.persona, thresholds.id));
        }
        if turn.persona.eq_ignore_ascii_case("jarvis") {
            check.push("jarvis spoke");
        }
        if !turn.pitch_st.is_finite()
            || !turn.rate.is_finite()
            || !turn.energy.is_finite()
            || turn.pitch_st.abs() > window.pitch_max
            || !(window.rate_lo..=window.rate_hi).contains(&turn.rate)
            || !(window.energy_lo..=window.energy_hi).contains(&turn.energy)
        {
            check.push(format!(
                "voice left the Samantha window (rate {} energy {} pitch {})",
                turn.rate, turn.energy, turn.pitch_st
            ));
        }
        if turn.neutral {
            let here = (turn.rate, turn.energy, turn.pitch_st);
            if distance(here, anchors.samantha) >= distance(here, anchors.jarvis) {
                check.push("neutral voice is closer to jarvis than to samantha");
            }
            let bits = (
                turn.rate.to_bits(),
                turn.energy.to_bits(),
                turn.pitch_st.to_bits(),
            );
            match neutral {
                None => neutral = Some(bits),
                Some(first) if first != bits => {
                    check.push("neutral turns changed voice");
                }
                Some(_) => {}
            }
        }
    }
    check
}

fn distance(sample: (f32, f32, f32), anchor: (f32, f32, f32)) -> f32 {
    (sample.0 - anchor.0).abs() + (sample.1 - anchor.1).abs() + (sample.2 - anchor.2).abs()
}

pub fn score_emotion(sample: &EmotionSample, thresholds: &EmotionThresholds) -> Check {
    let mut check = Check::ok();
    if sample.frame_ms != thresholds.frame_ms {
        check.push(format!(
            "emotion frame {} ms is not {}",
            sample.frame_ms, thresholds.frame_ms
        ));
    }
    if sample.fatigue_drop < thresholds.fatigue_arousal_drop_min {
        check.push(format!(
            "fatigue arousal drop {} is under {}",
            sample.fatigue_drop, thresholds.fatigue_arousal_drop_min
        ));
    }
    if sample.valence_rise < thresholds.valence_rise_min {
        check.push(format!(
            "valence rise {} is under {}",
            sample.valence_rise, thresholds.valence_rise_min
        ));
    }
    if sample.step_p95_us > thresholds.frame_budget_us {
        check.push(format!(
            "emotion step p95 {} us is over the {} us frame",
            sample.step_p95_us, thresholds.frame_budget_us
        ));
    }
    if !sample.off_clock_refused {
        check.push("a step off the 80 ms grid was accepted");
    }
    if !sample.baseline_held {
        check.push("the voice baseline moved");
    }
    if !sample.cue_follows_moment {
        check.push("the conductor cue did not follow the moment");
    }
    check
}

pub fn score_latency(sample: &LatencySample, thresholds: &LatencyThresholds) -> Check {
    let mut check = Check::ok();
    if sample.eager_to_voice_ms.is_empty() {
        check.push("no eager-to-voice sample");
        return check;
    }
    let p50 = percentile(&sample.eager_to_voice_ms, 50);
    let p95 = percentile(&sample.eager_to_voice_ms, 95);
    if p50 > thresholds.eager_to_voice_p50_max_ms {
        check.push(format!(
            "eager-to-voice p50 {p50} ms is over {}",
            thresholds.eager_to_voice_p50_max_ms
        ));
    }
    if p95 > thresholds.eager_to_voice_p95_max_ms {
        check.push(format!(
            "eager-to-voice p95 {p95} ms is over {}",
            thresholds.eager_to_voice_p95_max_ms
        ));
    }
    if sample.barge_in_ms > thresholds.barge_in_max_ms {
        check.push(format!(
            "barge-in {} ms is over {}",
            sample.barge_in_ms, thresholds.barge_in_max_ms
        ));
    }
    if sample.backchannel_cancel != thresholds.backchannel_cancels {
        check.push(format!(
            "backchannel cancel {} does not match the threshold",
            sample.backchannel_cancel
        ));
    }
    if sample.clock_advanced_ms != thresholds.frame_ms {
        check.push(format!(
            "virtual clock advanced {} ms after a {} ms wall sleep",
            sample.clock_advanced_ms, sample.wall_sleep_ms
        ));
    }
    check
}

/// Nearest rank, the same index as `galion_emotion_in::report::percentile`.
pub fn percentile(samples: &[u32], pct: u32) -> u32 {
    galion_emotion_in::report::percentile(samples, pct)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Thresholds;

    fn thresholds() -> Thresholds {
        Thresholds::embedded()
    }

    fn pack() -> Vec<CardFact> {
        vec![CardFact {
            id: "kettle".into(),
            gist: "The kettle is on the hob.".into(),
        }]
    }

    #[test]
    fn an_invented_card_fails_grounding() {
        let turn = Spoken {
            query: "where is the kettle".into(),
            reply: "The kettle is on the hob.".into(),
            sources: vec!["card:made-up".into()],
            sideband_posted: false,
            expect_id: Some("kettle".into()),
        };
        let check = score_grounding(&[turn], &pack(), &[], &thresholds().grounding);
        assert!(!check.pass);
        assert!(check
            .failures
            .iter()
            .any(|line| line.contains("invented") || line.contains("sources")));
    }

    #[test]
    fn a_research_citation_fails_the_miss() {
        let turn = Spoken {
            query: "why is the sky blue?".into(),
            reply: "I don't know.".into(),
            sources: vec!["research:local".into()],
            sideband_posted: false,
            expect_id: None,
        };
        let check = score_unknown(&[turn], &pack(), &[], &thresholds().grounding);
        assert!(!check.pass);
        assert!(check.failures.iter().any(|line| line.contains("research")));
    }

    #[test]
    fn jarvis_fails_the_persona_window() {
        let window = VoiceWindow {
            pitch_max: 3.0,
            rate_lo: 0.8,
            rate_hi: 1.3,
            energy_lo: 0.6,
            energy_hi: 1.2,
        };
        let turn = VoiceTurn {
            persona: "jarvis".into(),
            rate: 1.04,
            energy: 0.92,
            pitch_st: -0.3,
            neutral: true,
        };
        let anchors = VoiceAnchors {
            samantha: (1.0, 1.0, 0.0),
            jarvis: (1.04, 0.92, -0.3),
        };
        let check = score_persona(&[turn], window, anchors, &thresholds().persona);
        assert!(!check.pass);
    }

    #[test]
    fn a_slow_eager_path_fails_the_r4_budget() {
        let sample = LatencySample {
            eager_to_voice_ms: vec![80, 320],
            barge_in_ms: 160,
            backchannel_cancel: false,
            clock_advanced_ms: 80,
            wall_sleep_ms: 30,
        };
        let check = score_latency(&sample, &thresholds().latency);
        assert!(!check.pass);
        assert!(check.failures.iter().any(|line| line.contains("p95")));
    }
}
