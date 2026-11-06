//! Conditioning plan. Carries a measured expression profile into a later
//! clone stage. It does not select reference clips, and it does not run
//! unless a consumer reads `apply: true`.
//!
//! Crying and yelling are dropped for every persona. Seductive is kept only
//! when the persona file lists it (Samantha). Love/warmth is a second opt-in
//! and stays off for Jarvis, whose judged set is neutral + logic.

use crate::love::{
    Observation, REF_LOVE_MEAN_COS, REF_NEU_MEAN_COS, ALIGN_MIN, GATE,
};
use crate::profile::{ChannelProfile, ExpressionProfile};
use crate::prosody::ProsodyStats;
use galion_persona::Persona;
use serde::{Deserialize, Serialize};

pub const SCHEMA: &str = "galion-expression-plan/1";
pub const EXCLUDED: &[&str] = &["crying", "yelling"];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChannelCondition {
    pub emotion: String,
    pub judged: bool,
    pub f0_mean_hz: f64,
    pub f0_median_hz: f64,
    pub f0_sd_st: f64,
    pub f0_range_st_p5_p95: f64,
    pub rms_db: f64,
    pub spectral_tilt_db_oct: f64,
    pub singing: bool,
    pub vibrato_rate_hz: f64,
    pub vibrato_extent_cents: f64,
    pub e2v: std::collections::BTreeMap<String, f64>,
    pub clap_love: Option<f64>,
    pub clap_seductive: Option<f64>,
    pub love_mean_cos: Option<f64>,
    pub neutral_mean_cos: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SingingNote {
    pub judged: bool,
    pub phrases: usize,
    pub rate_hz: f64,
    pub extent_cents: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LoveWarmthLayer {
    pub id: String,
    pub opt_in: bool,
    pub apply: bool,
    pub reason: String,
    pub persona: String,
    pub align_min: f64,
    pub gate: f64,
    pub target_love_mean_cos: Option<f64>,
    pub target_margin: Option<f64>,
    pub prompt_banks: Vec<String>,
    pub acoustic: Option<ProsodyStats>,
    pub keeps: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConditioningPlan {
    pub schema: String,
    pub voice: String,
    pub persona_id: String,
    /// Master switch. Default false. A file on disk is a candidate until this is true.
    pub apply: bool,
    pub reference_selection: bool,
    pub excluded: Vec<String>,
    pub seductive: String,
    pub channels: Vec<ChannelCondition>,
    pub singing: Option<SingingNote>,
    pub love_warmth: LoveWarmthLayer,
    pub dropped: Vec<String>,
}

pub struct PlanRequest<'a> {
    pub profile: &'a ExpressionProfile,
    pub persona: &'a Persona,
    /// Second opt-in. Jarvis still refuses the layer.
    pub love_warmth: bool,
    /// Use the measured w03 love cosine and margin when the profile has no love channel.
    pub published_love_prior: bool,
}

pub fn build_plan(req: PlanRequest<'_>) -> ConditioningPlan {
    let persona = req.persona;
    let seductive_in = persona.in_set("seductive");
    let mut channels = Vec::new();
    let mut dropped = Vec::new();
    let mut sung_phrases = 0usize;
    let mut best_rate = 0.0;
    let mut best_extent = 0.0;
    for ch in &req.profile.channels {
        sung_phrases += ch.prosody.sung_phrases.len();
        if ch.prosody.vibrato_extent_cents > best_extent {
            best_extent = ch.prosody.vibrato_extent_cents;
            best_rate = ch.prosody.vibrato_rate_hz;
        }
        if !channel_allowed(persona, &ch.emotion) {
            dropped.push(ch.emotion.clone());
            continue;
        }
        channels.push(condition_from(ch, true));
    }
    let singing = if sung_phrases > 0 {
        Some(SingingNote {
            judged: false,
            phrases: sung_phrases,
            rate_hz: best_rate,
            extent_cents: best_extent,
        })
    } else {
        None
    };
    let love_channel = req
        .profile
        .channels
        .iter()
        .find(|c| c.emotion == "love" && channel_allowed(persona, "love"));
    let love_warmth = love_layer(persona, req.love_warmth, req.published_love_prior, love_channel);
    ConditioningPlan {
        schema: SCHEMA.into(),
        voice: req.profile.voice.clone(),
        persona_id: persona.id.clone(),
        apply: false,
        reference_selection: false,
        excluded: EXCLUDED.iter().map(|s| (*s).to_string()).collect(),
        seductive: if seductive_in {
            "samantha-only".into()
        } else {
            "n/a".into()
        },
        channels,
        singing,
        love_warmth,
        dropped,
    }
}

pub fn channel_allowed(persona: &Persona, emotion: &str) -> bool {
    if EXCLUDED.iter().any(|e| *e == emotion) {
        return false;
    }
    if emotion == "seductive" && !persona.in_set("seductive") {
        return false;
    }
    persona.in_set(emotion)
}

fn condition_from(ch: &ChannelProfile, judged: bool) -> ChannelCondition {
    ChannelCondition {
        emotion: ch.emotion.clone(),
        judged,
        f0_mean_hz: ch.prosody.f0_mean_hz,
        f0_median_hz: ch.prosody.f0_median_hz,
        f0_sd_st: ch.prosody.f0_sd_st,
        f0_range_st_p5_p95: ch.prosody.f0_range_st_p5_p95,
        rms_db: ch.prosody.rms_db,
        spectral_tilt_db_oct: ch.prosody.spectral_tilt_db_oct,
        singing: ch.singing,
        vibrato_rate_hz: ch.prosody.vibrato_rate_hz,
        vibrato_extent_cents: ch.prosody.vibrato_extent_cents,
        e2v: ch.e2v.clone(),
        clap_love: ch.clap_love,
        clap_seductive: ch.clap_seductive,
        love_mean_cos: ch.love_mean_cos,
        neutral_mean_cos: ch.neutral_mean_cos,
    }
}

fn love_layer(
    persona: &Persona,
    opt_in: bool,
    published_prior: bool,
    love_channel: Option<&ChannelProfile>,
) -> LoveWarmthLayer {
    let keeps = vec![
        "0.83 gate stays".into(),
        "align floor stays at 0.20".into(),
        "crying and yelling stay excluded".into(),
        "seductive stays Samantha-only".into(),
        "reference clip selection stays with the other branch".into(),
        "Piper stays the TTS".into(),
    ];
    if !persona.in_set("love") {
        return LoveWarmthLayer {
            id: "love-warmth".into(),
            opt_in,
            apply: false,
            reason: format!(
                "{} excludes love. Judged emotions stay unchanged and the passing gate is left alone.",
                persona.id
            ),
            persona: persona.id.clone(),
            align_min: ALIGN_MIN,
            gate: GATE,
            target_love_mean_cos: None,
            target_margin: None,
            prompt_banks: Vec::new(),
            acoustic: None,
            keeps,
        };
    }
    let (target_cos, target_margin, acoustic) = if let Some(ch) = love_channel {
        let margin = match (ch.love_mean_cos, ch.neutral_mean_cos) {
            (Some(l), Some(n)) => Some(l - n),
            _ => None,
        };
        (
            ch.love_mean_cos.or(Some(REF_LOVE_MEAN_COS)),
            margin.or(Some(REF_LOVE_MEAN_COS - REF_NEU_MEAN_COS)),
            Some(ch.prosody.clone()),
        )
    } else if published_prior {
        (
            Some(REF_LOVE_MEAN_COS),
            Some(REF_LOVE_MEAN_COS - REF_NEU_MEAN_COS),
            None,
        )
    } else {
        (None, None, None)
    };
    let have_target = target_cos.is_some();
    let apply = opt_in && have_target;
    let reason = if !opt_in {
        "love/warmth layer is opt-in; pass --love-warmth to mark it apply".into()
    } else if !have_target {
        "no love channel on the profile and no published prior; layer stays off".into()
    } else {
        "candidate carries the reference love cosine, margin, and acoustic colour into the clone".into()
    };
    LoveWarmthLayer {
        id: "love-warmth".into(),
        opt_in,
        apply,
        reason,
        persona: persona.id.clone(),
        align_min: ALIGN_MIN,
        gate: GATE,
        target_love_mean_cos: target_cos,
        target_margin,
        prompt_banks: vec!["love_shared".into(), "love_female".into()],
        acoustic,
        keeps,
    }
}

/// Reference observation used when a plan is built from the published w03 numbers.
pub fn published_love_observation() -> Observation {
    crate::love::published_reference()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::{analyze_channels, ChannelInput, ScoreOverlay};
    use crate::prosody::{sine, vibrato_tone};
    use galion_persona::load_persona;
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    fn personas_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../pipeline/personas")
    }

    fn persona(name: &str) -> Persona {
        load_persona(&personas_dir().join(name)).unwrap()
    }

    fn profile_for(voice: &str) -> ExpressionProfile {
        let mut e2v = BTreeMap::new();
        e2v.insert("neutral".into(), 1.0);
        analyze_channels(
            voice,
            &[
                ChannelInput {
                    emotion: "love".into(),
                    samples_16k: sine(190.0, 0.8, 0.08),
                    scores: ScoreOverlay {
                        e2v: e2v.clone(),
                        clap_love: Some(0.4),
                        clap_seductive: Some(0.1),
                        love_mean_cos: Some(REF_LOVE_MEAN_COS),
                        neutral_mean_cos: Some(REF_NEU_MEAN_COS),
                        sed_mean_cos: None,
                    },
                },
                ChannelInput {
                    emotion: "seductive".into(),
                    samples_16k: vibrato_tone(210.0, 5.5, 45.0, 1.0, 0.1),
                    scores: ScoreOverlay {
                        clap_seductive: Some(0.3),
                        ..ScoreOverlay::default()
                    },
                },
                ChannelInput {
                    emotion: "crying".into(),
                    samples_16k: sine(160.0, 0.6, 0.08),
                    scores: ScoreOverlay::default(),
                },
                ChannelInput {
                    emotion: "yelling".into(),
                    samples_16k: sine(240.0, 0.6, 0.2),
                    scores: ScoreOverlay::default(),
                },
                ChannelInput {
                    emotion: "neutral".into(),
                    samples_16k: sine(140.0, 0.6, 0.08),
                    scores: ScoreOverlay {
                        e2v,
                        ..ScoreOverlay::default()
                    },
                },
                ChannelInput {
                    emotion: "logic".into(),
                    samples_16k: sine(130.0, 0.6, 0.08),
                    scores: ScoreOverlay::default(),
                },
            ],
        )
    }

    #[test]
    fn samantha_keeps_seductive_and_drops_cry_yell() {
        let profile = profile_for("own-samantha");
        let plan = build_plan(PlanRequest {
            profile: &profile,
            persona: &persona("own-samantha.toml"),
            love_warmth: true,
            published_love_prior: false,
        });
        assert!(!plan.apply);
        assert!(!plan.reference_selection);
        let ids: Vec<_> = plan.channels.iter().map(|c| c.emotion.as_str()).collect();
        assert!(ids.contains(&"love"));
        assert!(ids.contains(&"seductive"));
        assert!(ids.contains(&"neutral"));
        assert!(!ids.contains(&"crying"));
        assert!(!ids.contains(&"yelling"));
        assert!(plan.dropped.iter().any(|d| d == "crying"));
        assert!(plan.dropped.iter().any(|d| d == "yelling"));
        assert_eq!(plan.seductive, "samantha-only");
        assert!(plan.love_warmth.apply);
        assert!((plan.love_warmth.target_love_mean_cos.unwrap() - REF_LOVE_MEAN_COS).abs() < 1e-12);
        let margin = REF_LOVE_MEAN_COS - REF_NEU_MEAN_COS;
        assert!((plan.love_warmth.target_margin.unwrap() - margin).abs() < 1e-12);
        assert_eq!(plan.love_warmth.prompt_banks, vec!["love_shared", "love_female"]);
        assert!(plan.love_warmth.acoustic.is_some());
        assert!(plan.singing.is_some());
        assert!(!plan.singing.as_ref().unwrap().judged);
        let love = plan.channels.iter().find(|c| c.emotion == "love").unwrap();
        assert!((love.f0_median_hz - 190.0).abs() < 3.0);
    }

    #[test]
    fn jarvis_love_layer_stays_off_and_seductive_is_na() {
        let profile = profile_for("own-jarvis");
        let plan = build_plan(PlanRequest {
            profile: &profile,
            persona: &persona("own-jarvis.toml"),
            love_warmth: true,
            published_love_prior: true,
        });
        let ids: Vec<_> = plan.channels.iter().map(|c| c.emotion.as_str()).collect();
        assert!(ids.contains(&"neutral"));
        assert!(ids.contains(&"logic"));
        assert!(!ids.contains(&"love"));
        assert!(!ids.contains(&"seductive"));
        assert!(!ids.contains(&"crying"));
        assert!(!ids.contains(&"yelling"));
        assert_eq!(plan.seductive, "n/a");
        assert!(!plan.love_warmth.apply);
        assert!(plan.love_warmth.reason.contains("excludes love"));
        assert!(plan.love_warmth.target_love_mean_cos.is_none());
    }

    #[test]
    fn samantha_without_flag_does_not_apply_the_layer() {
        let profile = profile_for("own-samantha");
        let plan = build_plan(PlanRequest {
            profile: &profile,
            persona: &persona("own-samantha.toml"),
            love_warmth: false,
            published_love_prior: true,
        });
        assert!(!plan.love_warmth.apply);
        assert!(plan.love_warmth.opt_in == false);
    }
}
