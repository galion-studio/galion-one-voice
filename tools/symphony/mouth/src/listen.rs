// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Closed look at a plan against a V1 read.
//!
//! The F1 residual is `(measured - baseline) - target_shift`. It stays null
//! when the target magnitude is unknown or the baseline read is absent.
//! There is no pass threshold. This is not TractPrint.

use serde::{Deserialize, Serialize};

use crate::plan::MouthPlan;
use crate::read::MouthRead;
use crate::TICK_MS;

pub const SCHEMA: &str = "galion-mouth-listen/1";

pub const NOTE: &str = "Shift residual against a baseline read of the same clock. Null when the plan has no hertz or the baseline is missing. Viseme match is categorical. SPARC lip aperture is reported in template units and is not scored. Not a gate.";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ListenTick {
    pub t_ms: u32,
    pub target_f1_hz: Option<f64>,
    pub measured_f1_hz: Option<f64>,
    pub baseline_f1_hz: Option<f64>,
    pub f1_error_hz: Option<f64>,
    pub target_viseme: String,
    pub measured_viseme: Option<String>,
    pub viseme_match: Option<bool>,
    pub measured_lip_aperture: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MouthListen {
    pub schema: String,
    pub tick_ms: u32,
    pub plan_ticks: usize,
    pub read_ticks: usize,
    pub compared_ticks: usize,
    pub ticks: Vec<ListenTick>,
    pub note: String,
}

pub fn listen(plan: &MouthPlan, read: &MouthRead, baseline: Option<&MouthRead>) -> MouthListen {
    let n = plan.ticks.len().min(read.n_ticks);
    let mut ticks = Vec::with_capacity(n);
    for i in 0..n {
        let target = plan.ticks[i].f1.amount;
        let measured = read.f1_hz.get(i).copied().flatten();
        let base = baseline.and_then(|b| b.f1_hz.get(i).copied().flatten());
        let error = match (target, measured, base) {
            (Some(t), Some(m), Some(b)) => Some((m - b) - t),
            _ => None,
        };
        let measured_viseme = read.visemes.get(i).and_then(|v| v.clone());
        let viseme_match = measured_viseme.as_ref().map(|m| m == &plan.ticks[i].viseme);
        ticks.push(ListenTick {
            t_ms: i as u32 * TICK_MS,
            target_f1_hz: target,
            measured_f1_hz: measured,
            baseline_f1_hz: base,
            f1_error_hz: error,
            target_viseme: plan.ticks[i].viseme.clone(),
            measured_viseme,
            viseme_match,
            measured_lip_aperture: read.lip_aperture.get(i).copied().flatten(),
        });
    }
    MouthListen {
        schema: SCHEMA.into(),
        tick_ms: TICK_MS,
        plan_ticks: plan.ticks.len(),
        read_ticks: read.n_ticks,
        compared_ticks: n,
        ticks,
        note: NOTE.into(),
    }
}
