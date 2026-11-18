// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! One control packet per 80 ms lungs tick.
//!
//! Voiced only while the lungs segment is `speech` and a subglottal pressure
//! is present. Inhale, hold and exhale stay unvoiced: the folds are not driven.
//!
//! Pitch is the lungs `f0_st` plus, on the prosody track only, a feeling
//! term. The feeling term is the published sign times the configured step
//! times intensity. An unknown step is not added. Burst does not take the
//! speech signs.
//!
//! Rd starts at the speaker value, or at the config reference when the
//! speaker has none. High-frequency energy moves it only when `rd_db` is
//! configured: more high-frequency energy, lower Rd (Fant and Kruckenberg
//! 1996: lower Rd, higher Fa). Fant's effort covariation then moves it from
//! the lungs loudness offset, and only when `ks` is configured.
//! Open quotient and spectral tilt are computed from that Rd. They are not
//! a second table of emotion targets.

use serde::Serialize;

use crate::config::Config;
use crate::contract::{Feeling, FeelingDrive, Speaker};
use crate::direction::{self, Sign};
use crate::inventory::Track;
use crate::lf::{self, Shape};
use crate::lungs_in::{self, LungsTick};
use crate::{DIRECTIONS, REUSES, SCHEMA};

#[derive(Clone, Debug)]
pub struct Request {
    pub speaker: Speaker,
    pub feeling: FeelingDrive,
    pub lungs: Vec<LungsTick>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct TickOut {
    pub t_ms: u32,
    pub segment: String,
    pub voiced: bool,
    pub phonation: String,
    pub category: Option<String>,
    pub track: String,
    pub prosody_tier: String,
    pub pole: Option<String>,
    pub pitch_sign: String,
    pub hf_sign: String,
    pub feeling_pitch_applied: bool,
    pub feeling_rd_applied: bool,
    pub effort_applied: bool,
    pub subglottal_pressure_rel: Option<f64>,
    pub f0_st_from_lungs: Option<f64>,
    pub f0_st_from_feeling: Option<f64>,
    pub f0_st_vs_baseline: Option<f64>,
    pub f0_hz: Option<f64>,
    pub f0_range_st: Option<f64>,
    pub rd: Option<f64>,
    pub rd_clamped: bool,
    pub ra: Option<f64>,
    pub rk: Option<f64>,
    pub rg: Option<f64>,
    pub open_quotient: Option<f64>,
    pub fa_hz: Option<f64>,
    pub spectral_tilt_db_oct: Option<f64>,
    pub ee_rel: Option<f64>,
    pub ee_source: Option<String>,
    pub jitter: Option<f64>,
    pub shimmer: Option<f64>,
    pub loudness_db_from_lungs: Option<f64>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Patch {
    pub schema: String,
    pub tick_ms: u32,
    pub baseline_f0_hz: f64,
    pub t_ms: Vec<u32>,
    pub voiced: Vec<bool>,
    pub phonation: Vec<String>,
    pub pitch_st: Vec<Option<f64>>,
    pub open_quotient: Vec<Option<f64>>,
    pub spectral_tilt_db_oct: Vec<Option<f64>>,
    pub rd: Vec<Option<f64>>,
    pub ee_rel: Vec<Option<f64>>,
    /// One value for the utterance. Null means the control is not applied.
    /// Not a per-category effect: Juslin and Laukka (2003, p. 796) do not
    /// give a jitter sign this layer will use.
    pub jitter: Option<f64>,
    pub shimmer: Option<f64>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct ReferenceNote {
    pub generated: bool,
    pub kind: String,
    pub sample_rate: Option<u32>,
    /// Samples stay out of the JSON. A wav is written only when the CLI is
    /// asked, and that wav is peak-scaled so it is not an Ee calibration.
    pub embedded: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Utterance {
    pub schema: String,
    pub tick_ms: u32,
    pub baseline_f0_hz: f64,
    pub n_ticks: usize,
    pub patch: Patch,
    pub ticks: Vec<TickOut>,
    pub directions: Vec<String>,
    pub reuses: Vec<String>,
    pub reference: ReferenceNote,
}

pub fn render(
    req: &Request,
    cfg: &Config,
    with_source: bool,
) -> Result<(Utterance, Option<Vec<f32>>), String> {
    if req.lungs.is_empty() {
        return Err("no lungs ticks".into());
    }
    let jitter = cfg.scale(None, "jitter");
    let shimmer = cfg.scale(None, "shimmer");
    let mut ticks = Vec::with_capacity(req.lungs.len());
    for lung in &req.lungs {
        let feeling = req.feeling.at(lung.t_ms);
        ticks.push(one_tick(
            lung,
            &feeling,
            &req.speaker,
            cfg,
            jitter,
            shimmer,
        )?);
    }
    let patch = patch_of(&ticks, req.speaker.f0_hz, jitter, shimmer);
    let mut utterance = Utterance {
        schema: SCHEMA.into(),
        tick_ms: crate::TICK_MS,
        baseline_f0_hz: req.speaker.f0_hz,
        n_ticks: ticks.len(),
        patch,
        ticks,
        directions: DIRECTIONS.iter().map(|s| (*s).to_string()).collect(),
        reuses: REUSES.iter().map(|s| (*s).to_string()).collect(),
        reference: ReferenceNote {
            generated: false,
            kind: "not_requested".into(),
            sample_rate: None,
            embedded: false,
        },
    };
    let source = if with_source {
        let samples = crate::source::render_source(&utterance.ticks, cfg)?;
        utterance.reference = ReferenceNote {
            generated: true,
            kind: "lf_flow_derivative".into(),
            sample_rate: Some(cfg.source_hz),
            embedded: false,
        };
        Some(samples)
    } else {
        None
    };
    Ok((utterance, source))
}

fn one_tick(
    lung: &LungsTick,
    feeling: &Feeling,
    speaker: &Speaker,
    cfg: &Config,
    jitter: Option<f64>,
    shimmer: Option<f64>,
) -> Result<TickOut, String> {
    let voiced = lungs_in::is_phonation(lung);
    let category = feeling.category.map(|c| c.as_str().to_string());
    let tier = match feeling.category {
        Some(c) => c.prosody_tier().to_string(),
        None => "baseline".into(),
    };
    if !voiced {
        return Ok(TickOut {
            t_ms: lung.t_ms,
            segment: lung.segment.clone(),
            voiced: false,
            phonation: "unvoiced".into(),
            category,
            track: feeling.track.as_str().into(),
            prosody_tier: tier,
            pole: None,
            pitch_sign: "unvoiced".into(),
            hf_sign: "unvoiced".into(),
            feeling_pitch_applied: false,
            feeling_rd_applied: false,
            effort_applied: false,
            subglottal_pressure_rel: lung.subglottal_pressure_rel,
            f0_st_from_lungs: None,
            f0_st_from_feeling: None,
            f0_st_vs_baseline: None,
            f0_hz: None,
            f0_range_st: None,
            rd: None,
            rd_clamped: false,
            ra: None,
            rk: None,
            rg: None,
            open_quotient: None,
            fa_hz: None,
            spectral_tilt_db_oct: None,
            ee_rel: None,
            ee_source: None,
            jitter: None,
            shimmer: None,
            loudness_db_from_lungs: lung.loudness_db,
        });
    }

    let FeelingTerms {
        pitch_sign,
        hf_sign,
        pole,
        f0_feeling,
        f0_range,
        rd_db,
        feeling_pitch,
        feeling_rd,
    } = feeling_terms(feeling, cfg);

    let mut rd = speaker.rd.unwrap_or(cfg.rd_baseline);
    if let Some(db) = rd_db {
        rd *= 10_f64.powf(db / 20.0);
    }
    let (ee_rel, ee_source) = match lung.loudness_db {
        Some(db) => (10_f64.powf(db / 20.0), "lungs_loudness_db"),
        None => (1.0, "unity"),
    };
    let mut effort_applied = false;
    if let (Some(ks), Some(db)) = (cfg.ks, lung.loudness_db) {
        if ks != 0.0 && db != 0.0 {
            rd *= 10_f64.powf(-ks * db / 20.0);
            effort_applied = true;
        }
    }
    let st_lungs = lung.f0_st.unwrap_or(0.0);
    let st = st_lungs + f0_feeling.unwrap_or(0.0);
    let f0 = speaker.f0_hz * 2_f64.powf(st / 12.0);
    if !f0.is_finite() || f0 <= 0.0 {
        return Err("pitch contour left a non-positive F0".into());
    }
    let shape: Shape = lf::shape_for(rd, f0, ee_rel.max(0.0), cfg)?;
    let label = lf::phonation_label(shape.rd, feeling.creaky, cfg);
    Ok(TickOut {
        t_ms: lung.t_ms,
        segment: lung.segment.clone(),
        voiced: true,
        phonation: label.into(),
        category,
        track: feeling.track.as_str().into(),
        prosody_tier: tier,
        pole: pole.map(|s| s.to_string()),
        pitch_sign,
        hf_sign,
        feeling_pitch_applied: feeling_pitch,
        feeling_rd_applied: feeling_rd,
        effort_applied,
        subglottal_pressure_rel: lung.subglottal_pressure_rel,
        f0_st_from_lungs: lung.f0_st,
        f0_st_from_feeling: f0_feeling,
        f0_st_vs_baseline: Some(st),
        f0_hz: Some(f0),
        f0_range_st: f0_range,
        rd: Some(shape.rd),
        rd_clamped: shape.rd_clamped,
        ra: Some(shape.ra),
        rk: Some(shape.rk),
        rg: Some(shape.rg),
        open_quotient: Some(shape.open_quotient),
        fa_hz: Some(shape.fa_hz),
        spectral_tilt_db_oct: Some(shape.spectral_tilt_db_oct),
        ee_rel: Some(ee_rel),
        ee_source: Some(ee_source.into()),
        jitter,
        shimmer,
        loudness_db_from_lungs: lung.loudness_db,
    })
}

struct FeelingTerms {
    pitch_sign: String,
    hf_sign: String,
    pole: Option<&'static str>,
    f0_feeling: Option<f64>,
    f0_range: Option<f64>,
    rd_db: Option<f64>,
    feeling_pitch: bool,
    feeling_rd: bool,
}

fn feeling_terms(feeling: &Feeling, cfg: &Config) -> FeelingTerms {
    let Some(category) = feeling.category else {
        return FeelingTerms {
            pitch_sign: "baseline".into(),
            hf_sign: "baseline".into(),
            pole: None,
            f0_feeling: None,
            f0_range: None,
            rd_db: None,
            feeling_pitch: false,
            feeling_rd: false,
        };
    };
    if feeling.track == Track::Burst {
        return FeelingTerms {
            pitch_sign: "burst".into(),
            hf_sign: "burst".into(),
            pole: None,
            f0_feeling: None,
            f0_range: None,
            rd_db: None,
            feeling_pitch: false,
            feeling_rd: false,
        };
    }
    let dir = direction::speech_directions(category, feeling.dominance);
    let pitch_scale = cfg.scale(dir.pole, "pitch_st");
    let range_scale = cfg.scale(dir.pole, "pitch_range_st");
    let rd_scale = cfg.scale(dir.pole, "rd_db");
    let f0_feeling = apply(dir.pitch, pitch_scale, feeling.intensity);
    let f0_range = apply(dir.pitch_range, range_scale, feeling.intensity);
    // More high-frequency energy → lower Rd. The minus is Fant's direction
    // (lower Rd, higher Fa), not a second emotion sign.
    let rd_db = apply(dir.hf, rd_scale, feeling.intensity).map(|v| -v);
    FeelingTerms {
        pitch_sign: dir.pitch.as_str().into(),
        hf_sign: dir.hf.as_str().into(),
        pole: dir.pole,
        f0_feeling,
        f0_range,
        rd_db,
        feeling_pitch: f0_feeling.is_some(),
        feeling_rd: rd_db.is_some(),
    }
}

fn apply(sign: Sign, scale: Option<f64>, intensity: f64) -> Option<f64> {
    let pole = sign.pole()?;
    let step = scale?;
    Some(pole * step * intensity)
}

fn patch_of(
    ticks: &[TickOut],
    baseline_f0_hz: f64,
    jitter: Option<f64>,
    shimmer: Option<f64>,
) -> Patch {
    Patch {
        schema: SCHEMA.into(),
        tick_ms: crate::TICK_MS,
        baseline_f0_hz,
        t_ms: ticks.iter().map(|t| t.t_ms).collect(),
        voiced: ticks.iter().map(|t| t.voiced).collect(),
        phonation: ticks.iter().map(|t| t.phonation.clone()).collect(),
        pitch_st: ticks.iter().map(|t| t.f0_st_vs_baseline).collect(),
        open_quotient: ticks.iter().map(|t| t.open_quotient).collect(),
        spectral_tilt_db_oct: ticks.iter().map(|t| t.spectral_tilt_db_oct).collect(),
        rd: ticks.iter().map(|t| t.rd).collect(),
        ee_rel: ticks.iter().map(|t| t.ee_rel).collect(),
        jitter,
        shimmer,
    }
}
