// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Packets the downstream layers already accept.
//!
//! `controls` is the decoder tick PR #192 reads (`t_ms`, `arousal`, `valence`,
//! `dominance`, `intensity`, `blend[{id,w}]`, optional `track`).
//! `lungs` is the moment PR #195 plans from (`at_s`, `category`, `arousal`,
//! `energy`, `fatigue`). Approach stays on the feeling axes. It is not one of
//! those two packets.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::config::PinPole;
use crate::CORE_AXES;

#[derive(Clone, Debug, PartialEq)]
pub struct AppliedPin {
    pub category: String,
    pub w: f64,
    pub alias_from: Option<String>,
    pub placed: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct NamedValue {
    pub name: String,
    pub value: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct BlendEntry {
    pub id: String,
    pub w: f64,
}

/// The object `TargetTick::from_decoder_value` in galion-emotion-render accepts.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ControlsTick {
    pub t_ms: u32,
    pub track: String,
    pub arousal: f64,
    pub valence: f64,
    pub dominance: f64,
    pub intensity: f64,
    pub blend: Vec<BlendEntry>,
}

/// One row of the lungs `trajectory`. Field names match `lungs::plan::Moment`.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct LungsMoment {
    pub at_s: f64,
    pub category: String,
    pub arousal: f64,
    pub energy: f64,
    pub fatigue: f64,
}

/// Top-level lungs request fields plus the per-tick trajectory.
/// Text is not here. The lungs layer owns the phrase.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct LungsRequest {
    pub category: String,
    pub arousal: f64,
    pub energy: f64,
    pub fatigue: f64,
    pub trajectory: Vec<LungsMoment>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PinView {
    pub id: String,
    pub w: f64,
    pub placed: bool,
    pub alias_from: Option<String>,
}

/// Closest placed pin. Informational. `assigned` is always false: sitting
/// near a pin does not turn the state into that category.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct NearestPin {
    pub id: String,
    pub distance: f64,
    pub cite: String,
    pub assigned: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct FeelingOut {
    pub schema: String,
    pub tick_ms: u32,
    pub t_ms: u32,
    pub axes: Vec<NamedValue>,
    pub mood: Vec<NamedValue>,
    pub goal: Vec<NamedValue>,
    pub energy: f64,
    pub fatigue: f64,
    pub environment: Option<String>,
    pub listener_distance_m: Option<f64>,
    pub outer_applied: bool,
    pub inner_applied: bool,
    pub track: String,
    pub pins: Vec<PinView>,
    pub nearest: Option<NearestPin>,
    pub open: Vec<String>,
    pub citations: Vec<String>,
    pub controls: ControlsTick,
    pub lungs: LungsMoment,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Run {
    pub schema: String,
    pub tick_ms: u32,
    pub contract: String,
    pub frames: Vec<FeelingOut>,
    pub lungs: LungsRequest,
    pub directions: Vec<String>,
}

pub struct Snapshot {
    pub t_ms: u32,
    pub t0_ms: u32,
    pub track: String,
    pub now: BTreeMap<String, f64>,
    pub mood: BTreeMap<String, f64>,
    pub goal: BTreeMap<String, f64>,
    pub energy: f64,
    pub fatigue: f64,
    pub environment: Option<String>,
    pub listener_distance_m: Option<f64>,
    pub outer_applied: bool,
    pub inner_applied: bool,
    pub applied: Vec<AppliedPin>,
    pub open: Vec<String>,
    pub citations: Vec<String>,
    pub category_tie: bool,
}

impl FeelingOut {
    pub fn axis(&self, name: &str) -> f64 {
        lookup(&self.axes, name)
    }

    pub fn mood_axis(&self, name: &str) -> f64 {
        lookup(&self.mood, name)
    }

    pub fn goal_axis(&self, name: &str) -> f64 {
        lookup(&self.goal, name)
    }
}

fn lookup(rows: &[NamedValue], name: &str) -> f64 {
    rows.iter()
        .find(|row| row.name == name)
        .map(|row| row.value)
        .unwrap_or(0.0)
}

pub fn assemble(snap: Snapshot, pins: &BTreeMap<String, PinPole>) -> (FeelingOut, LungsMoment) {
    let intensity = CORE_AXES
        .iter()
        .map(|name| snap.now.get(*name).copied().unwrap_or(0.0).abs())
        .fold(0.0_f64, f64::max);
    let mut blend_w: BTreeMap<String, f64> = BTreeMap::new();
    let mut views = Vec::new();
    for pin in &snap.applied {
        *blend_w.entry(pin.category.clone()).or_insert(0.0) += pin.w;
        views.push(PinView {
            id: pin.category.clone(),
            w: pin.w,
            placed: pin.placed,
            alias_from: pin.alias_from.clone(),
        });
    }
    let blend: Vec<BlendEntry> = blend_w
        .into_iter()
        .map(|(id, w)| BlendEntry { id, w })
        .collect();
    let category = if snap.category_tie {
        "neutral".to_string()
    } else {
        top_category(&blend)
    };
    let at_s = (snap.t_ms.saturating_sub(snap.t0_ms)) as f64 / 1000.0;
    let lungs = LungsMoment {
        at_s,
        category,
        arousal: snap.now.get("arousal").copied().unwrap_or(0.0),
        energy: snap.energy,
        fatigue: snap.fatigue,
    };
    let controls = ControlsTick {
        t_ms: snap.t_ms,
        track: snap.track.clone(),
        arousal: snap.now.get("arousal").copied().unwrap_or(0.0),
        valence: snap.now.get("valence").copied().unwrap_or(0.0),
        dominance: snap.now.get("dominance").copied().unwrap_or(0.0),
        intensity,
        blend,
    };
    let out = FeelingOut {
        schema: crate::SCHEMA.to_string(),
        tick_ms: crate::TICK_MS,
        t_ms: snap.t_ms,
        axes: named(&snap.now),
        mood: named(&snap.mood),
        goal: named(&snap.goal),
        energy: snap.energy,
        fatigue: snap.fatigue,
        environment: snap.environment,
        listener_distance_m: snap.listener_distance_m,
        outer_applied: snap.outer_applied,
        inner_applied: snap.inner_applied,
        track: snap.track,
        pins: views,
        nearest: nearest(&snap.now, pins),
        open: snap.open,
        citations: snap.citations,
        controls,
        lungs: lungs.clone(),
    };
    (out, lungs)
}

fn top_category(blend: &[BlendEntry]) -> String {
    let mut best: Option<&BlendEntry> = None;
    for row in blend {
        match best {
            None => best = Some(row),
            Some(cur) if row.w > cur.w => best = Some(row),
            _ => {}
        }
    }
    best.map(|row| row.id.clone())
        .unwrap_or_else(|| "neutral".to_string())
}

fn named(map: &BTreeMap<String, f64>) -> Vec<NamedValue> {
    let mut rows = Vec::new();
    for name in CORE_AXES {
        rows.push(NamedValue {
            name: (*name).to_string(),
            value: map.get(name).copied().unwrap_or(0.0),
        });
    }
    for (name, value) in map {
        if CORE_AXES.contains(&name.as_str()) {
            continue;
        }
        rows.push(NamedValue {
            name: name.clone(),
            value: *value,
        });
    }
    rows
}

/// Mean absolute distance to the pole, over the axes that pole places.
/// At the origin every pole is distance 1, so a pin is reported only when
/// the state is strictly closer than that.
fn nearest(now: &BTreeMap<String, f64>, pins: &BTreeMap<String, PinPole>) -> Option<NearestPin> {
    let mut best: Option<NearestPin> = None;
    for (id, pole) in pins {
        let mut n = 0.0;
        let mut acc = 0.0;
        for axis in CORE_AXES {
            if let Some(sign) = pole.sign(axis) {
                let v = now.get(axis).copied().unwrap_or(0.0);
                acc += (v - f64::from(sign)).abs();
                n += 1.0;
            }
        }
        if n == 0.0 {
            continue;
        }
        let distance = acc / n;
        if distance >= 1.0 {
            continue;
        }
        let take = match &best {
            None => true,
            Some(cur) if distance < cur.distance - 1e-12 => true,
            Some(cur)
                if (distance - cur.distance).abs() <= 1e-12 && id.as_str() < cur.id.as_str() =>
            {
                true
            }
            _ => false,
        };
        if take {
            best = Some(NearestPin {
                id: id.clone(),
                distance,
                cite: pole.cite.clone(),
                assigned: false,
            });
        }
    }
    best
}
