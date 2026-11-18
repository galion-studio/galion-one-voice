// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Read wordless events. Detection stays in draft PR 191.
//!
//! Two JSON shapes, either or both:
//! - The laughter worker (`workers/laughter_worker.py`): `source`, `gap`, `events`.
//!   An empty `events` array with a `gap` is a real result. It is not a laugh.
//! - The decode burst track (`decode.rs` `BurstTrack`): `weights`, `top`, `source`.
//!   A `TickState` list (`burst` weights per tick) is averaged the same way
//!   `burst_summary` does. Weights are the feeling of the burst track. They
//!   are not a time span, and this function does not turn them into a gesture.

use serde::Serialize;
use serde_json::Value;

use crate::feeling::Weight;
use crate::gestures::{self, SIGH_CANDIDATES};

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct ReadEvent {
    pub gesture: String,
    pub start_s: f64,
    pub end_s: f64,
    pub category: Option<String>,
    pub candidates: Vec<String>,
    pub source: String,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Reading {
    pub source: String,
    pub gap: Option<String>,
    pub events: Vec<ReadEvent>,
    pub burst_top: Option<String>,
    pub weights: Vec<Weight>,
    pub notes: Vec<String>,
}

impl Reading {
    pub fn empty() -> Self {
        Reading {
            source: String::new(),
            gap: None,
            events: Vec::new(),
            burst_top: None,
            weights: Vec::new(),
            notes: Vec::new(),
        }
    }
}

pub fn parse_reading(value: &Value) -> Result<Reading, String> {
    let obj = value
        .as_object()
        .ok_or("burst reading JSON must be an object")?;
    let mut reading = Reading::empty();
    if obj.contains_key("events") && !obj.contains_key("weights") {
        apply_worker(&mut reading, value)?;
    }
    if let Some(w) = obj.get("laughter") {
        apply_worker(&mut reading, w)?;
    }
    if let Some(b) = obj.get("burst") {
        if b.get("weights").is_some() || b.get("top").is_some() {
            apply_burst_track(&mut reading, b)?;
        } else if b.get("events").is_some() {
            apply_worker(&mut reading, b)?;
        }
    } else if obj.contains_key("weights") {
        apply_burst_track(&mut reading, value)?;
    }
    if let Some(ticks) = obj
        .get("ticks")
        .or_else(|| value.pointer("/target/ticks"))
        .or_else(|| value.pointer("/measured/ticks"))
    {
        if reading.weights.is_empty() {
            apply_ticks(&mut reading, ticks)?;
        }
    }
    if reading.source.is_empty() {
        reading.source = "none".into();
    }
    Ok(reading)
}

fn apply_worker(reading: &mut Reading, value: &Value) -> Result<(), String> {
    let source = value
        .get("source")
        .and_then(|s| s.as_str())
        .unwrap_or("laughter-worker")
        .to_string();
    if let Some(gap) = value.get("gap").and_then(|g| g.as_str()) {
        if !gap.is_empty() {
            reading.gap = Some(gap.to_string());
        }
    }
    let events = value
        .get("events")
        .and_then(|e| e.as_array())
        .cloned()
        .unwrap_or_default();
    for (i, ev) in events.iter().enumerate() {
        let start = time_of(ev, &["start_s", "t0", "start"])
            .ok_or_else(|| format!("laughter event {i} needs start_s"))?;
        let end = time_of(ev, &["end_s", "t1", "end"])
            .ok_or_else(|| format!("laughter event {i} needs end_s"))?;
        if !(start.is_finite() && end.is_finite()) || end <= start {
            return Err(format!("laughter event {i} has a bad span"));
        }
        let kind = ev
            .get("kind")
            .or_else(|| ev.get("label"))
            .and_then(|s| s.as_str())
            .unwrap_or("laugh");
        let (gesture, category, candidates, note) = classify(kind);
        if let Some(note) = note {
            reading.notes.push(note);
        }
        reading.events.push(ReadEvent {
            gesture,
            start_s: start,
            end_s: end,
            category,
            candidates,
            source: source.clone(),
        });
    }
    reading.source = if reading.source.is_empty() {
        source
    } else {
        format!("{}; {source}", reading.source)
    };
    Ok(())
}

fn classify(kind: &str) -> (String, Option<String>, Vec<String>, Option<String>) {
    match gestures::canonical(kind) {
        Some("laugh") => (
            "laugh".into(),
            Some("amusement".into()),
            Vec::new(),
            None,
        ),
        Some("sigh") => (
            "sigh".into(),
            None,
            SIGH_CANDIDATES.iter().map(|s| (*s).to_string()).collect(),
            Some(
                "sigh is a detector across relief, sadness, disappointment, and contentment; no category was assigned"
                    .into(),
            ),
        ),
        Some(other) => (other.into(), None, Vec::new(), None),
        None => (
            kind.to_string(),
            None,
            Vec::new(),
            Some(format!(
                "event kind {kind} is not a gesture this layer plans"
            )),
        ),
    }
}

fn apply_burst_track(reading: &mut Reading, value: &Value) -> Result<(), String> {
    if let Some(w) = value.get("weights") {
        reading.weights = decode_weights(w)?;
    }
    if let Some(top) = value.get("top").and_then(|s| s.as_str()) {
        if top != "none" && !top.is_empty() {
            reading.burst_top = Some(top.to_string());
        }
    }
    let source = value
        .get("source")
        .and_then(|s| s.as_str())
        .unwrap_or("burst-track");
    push_source(reading, source);
    Ok(())
}

fn apply_ticks(reading: &mut Reading, ticks: &Value) -> Result<(), String> {
    let arr = ticks.as_array().ok_or("ticks must be an array")?;
    if arr.is_empty() {
        return Ok(());
    }
    let mut acc: Vec<(String, f64)> = Vec::new();
    let mut n = 0.0;
    for tick in arr {
        let burst = match tick.get("burst") {
            Some(b) => decode_weights(b)?,
            None => continue,
        };
        n += 1.0;
        for w in burst {
            if let Some(slot) = acc.iter_mut().find(|(id, _)| *id == w.id) {
                slot.1 += w.w;
            } else {
                acc.push((w.id, w.w));
            }
        }
    }
    if n == 0.0 {
        return Ok(());
    }
    reading.weights = acc
        .into_iter()
        .map(|(id, w)| Weight { id, w: w / n })
        .collect();
    reading.burst_top = reading
        .weights
        .iter()
        .filter(|w| w.w > 0.0)
        .max_by(|a, b| a.w.partial_cmp(&b.w).unwrap_or(std::cmp::Ordering::Equal))
        .map(|w| w.id.clone());
    push_source(reading, "tick-burst-mean");
    Ok(())
}

fn decode_weights(v: &Value) -> Result<Vec<Weight>, String> {
    let arr = v.as_array().ok_or("weights must be an array")?;
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        let id = item
            .get("id")
            .and_then(|s| s.as_str())
            .ok_or("a weight needs an id")?
            .to_string();
        let w = item
            .get("w")
            .and_then(|n| n.as_f64())
            .filter(|n| n.is_finite())
            .ok_or_else(|| format!("weight {id} needs a finite w"))?;
        out.push(Weight { id, w });
    }
    Ok(out)
}

fn time_of(v: &Value, keys: &[&str]) -> Option<f64> {
    for k in keys {
        if let Some(n) = v.get(*k).and_then(|x| x.as_f64()) {
            return Some(n);
        }
    }
    None
}

fn push_source(reading: &mut Reading, source: &str) {
    if reading.source.is_empty() {
        reading.source = source.into();
    } else if !reading.source.split("; ").any(|s| s == source) {
        reading.source = format!("{}; {source}", reading.source);
    }
}
