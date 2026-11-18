// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Two-pass loop. Pass 1 writes the voice profile from the reference.
//! A renderer (the stand-in here, or an external command at the CLI) makes
//! a clip from that profile. Pass 2 decodes the clip with the same analyser
//! and nudges exaggeration and speed. Speaker F0 is checked and not steered.
//! At most `max_iter` corrections.

use crate::acoustic;
use crate::breath;
use crate::decode::{self, RenderParams, VoiceProfile};
use crate::select;
use crate::Articulation;
use serde::Serialize;

#[derive(Clone, Debug)]
pub struct Request<'a> {
    pub emotion: Option<&'a str>,
    pub env_name: Option<&'a str>,
    pub enrol: Option<&'a [f64]>,
    pub max_iter: usize,
    pub start_exaggeration: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct FieldDelta {
    pub field: String,
    pub target: f64,
    pub measured: f64,
    pub tol: f64,
    pub pass: bool,
    pub corrects: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct LoopReport {
    pub schema: u32,
    pub renderer: String,
    pub passed: bool,
    pub iterations: usize,
    pub profile: VoiceProfile,
    pub segments: Option<select::SegmentTable>,
    pub applied: Vec<RenderParams>,
    pub deltas: Vec<FieldDelta>,
}

#[derive(Clone, Copy, Debug)]
pub struct Observation {
    pub f0_hz: f64,
    pub rms_dbfs: f64,
    pub voiced_runs_per_s: f64,
}

fn observe(samples: &[f64], sr: f64) -> (Articulation, Observation) {
    let (mut art, _) = acoustic::analyse(samples, sr);
    art.finish();
    let lungs = breath::measure(&art, None);
    let f0 = art
        .summary
        .get("f0_hz")
        .and_then(|s| s.q.get(4).copied())
        .unwrap_or(0.0);
    (
        art,
        Observation {
            f0_hz: f0,
            rms_dbfs: decode::sample_rms_db(samples),
            voiced_runs_per_s: lungs.voiced_runs_per_s,
        },
    )
}

pub fn deltas(goal_f0: f64, goal_rms: f64, goal_rate: f64, obs: &Observation) -> Vec<FieldDelta> {
    let f0_err = if goal_f0 > 0.0 {
        (obs.f0_hz - goal_f0).abs() / goal_f0
    } else {
        0.0
    };
    let rate_err = if goal_rate > 0.0 {
        (obs.voiced_runs_per_s - goal_rate).abs() / goal_rate
    } else {
        0.0
    };
    vec![
        FieldDelta {
            field: "f0_hz".into(),
            target: goal_f0,
            measured: obs.f0_hz,
            tol: 0.08,
            pass: f0_err <= 0.08,
            corrects: None,
        },
        FieldDelta {
            field: "rms_dbfs".into(),
            target: goal_rms,
            measured: obs.rms_dbfs,
            tol: 1.5,
            pass: (obs.rms_dbfs - goal_rms).abs() <= 1.5,
            corrects: Some("exaggeration".into()),
        },
        FieldDelta {
            field: "voiced_runs_per_s".into(),
            target: goal_rate,
            measured: obs.voiced_runs_per_s,
            tol: 0.10,
            pass: rate_err <= 0.10 || goal_rate == 0.0,
            corrects: Some("speed".into()),
        },
    ]
}

pub fn correct(params: RenderParams, rows: &[FieldDelta]) -> RenderParams {
    let mut params = params;
    for row in rows {
        if row.pass {
            continue;
        }
        match row.corrects.as_deref() {
            Some("exaggeration") => {
                let db = row.target - row.measured;
                params.exaggeration = (params.exaggeration + 0.08 * db).clamp(0.0, 1.5);
            }
            Some("speed") if row.measured > 0.0 => {
                let ratio = row.target / row.measured;
                params.speed = (params.speed * (1.0 + 0.5 * (ratio - 1.0))).clamp(0.80, 1.25);
            }
            _ => {}
        }
    }
    params
}

pub fn run_standin(samples: &[f64], sr: f64, req: &Request) -> Result<LoopReport, String> {
    let (kept, segments) = if let Some(enrol) = req.enrol {
        let table = select::select(samples, sr, enrol, None)?;
        if table.n_kept == 0 {
            return Err("no clean target-speaker segment to write".into());
        }
        (select::concat_kept(samples, sr, &table), Some(table))
    } else {
        (samples.to_vec(), None)
    };
    let (art, obs) = observe(&kept, sr);
    let profile = decode::profile(
        &art,
        &kept,
        sr,
        &decode::Opts {
            emotion: req.emotion,
            env_name: req.env_name,
            emotion_json: None,
            egemaps: None,
            neutral_egemaps: None,
            labels: None,
            cpps_db: None,
            breath_cues: None,
            breath_source: None,
        },
    )?;
    let goal = profile.chatterbox;
    let goal_rms = obs.rms_dbfs + (goal.exaggeration - 0.5) * 12.0;
    let goal_rate = obs.voiced_runs_per_s * goal.speed;
    let mut params = goal;
    if let Some(ex) = req.start_exaggeration {
        params.exaggeration = ex.clamp(0.0, 1.5);
    }
    let max_iter = req.max_iter.max(1);
    let mut applied = Vec::new();
    let mut last = Vec::new();
    for _ in 0..max_iter {
        applied.push(params);
        let y = decode::apply_render(&kept, sr, params, &profile.trajectory);
        let (_, got) = observe(&y, sr);
        last = deltas(obs.f0_hz, goal_rms, goal_rate, &got);
        if last.iter().all(|d| d.pass) {
            break;
        }
        let next = correct(params, &last);
        if (next.exaggeration - params.exaggeration).abs() < 1e-3
            && (next.speed - params.speed).abs() < 1e-3
        {
            break;
        }
        params = next;
    }
    let passed = last.iter().all(|d| d.pass);
    Ok(LoopReport {
        schema: 1,
        renderer: "stand-in".into(),
        passed,
        iterations: applied.len(),
        profile,
        segments,
        applied,
        deltas: last,
    })
}
