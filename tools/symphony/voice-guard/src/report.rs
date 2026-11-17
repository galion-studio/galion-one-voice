// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Deterministic `galion-voice-guard/1` report.

use serde::Serialize;

pub const SCHEMA: &str = "galion-voice-guard/1";

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct FrameSpan {
    /// Inclusive 80 ms frame index.
    pub start: u32,
    /// Exclusive frame index.
    pub end: u32,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Check {
    pub name: &'static str,
    /// `pass`, `fail`, `skipped`, or `stub`.
    pub status: &'static str,
    /// 1 is clean. Absent when the check did not run.
    pub score: Option<f64>,
    pub value: Option<f64>,
    pub limit: Option<f64>,
    pub unit: &'static str,
    pub reason: String,
    pub frames: Vec<FrameSpan>,
    pub method: &'static str,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Report {
    pub schema: &'static str,
    /// `pass` or `fail`.
    pub verdict: &'static str,
    /// True when the clip must not be played or handed to listening-back.
    pub bad: bool,
    pub reason: String,
    pub sample_rate_hz: u32,
    pub channels: u16,
    pub tick_ms: u32,
    pub frames: u32,
    pub samples: usize,
    /// Union of the frame spans from failed checks.
    pub bad_frames: Vec<FrameSpan>,
    pub checks: Vec<Check>,
}

impl Report {
    pub fn from_checks(
        checks: Vec<Check>,
        sample_rate_hz: u32,
        channels: u16,
        tick_ms: u32,
        frames: u32,
        samples: usize,
    ) -> Self {
        let bad = checks.iter().any(|c| c.status == "fail");
        let reason = checks
            .iter()
            .filter(|c| c.status == "fail")
            .map(|c| format!("{}: {}", c.name, c.reason))
            .collect::<Vec<_>>()
            .join("; ");
        let mut idxs = Vec::new();
        for c in checks.iter().filter(|c| c.status == "fail") {
            for span in &c.frames {
                for i in span.start..span.end {
                    idxs.push(i);
                }
            }
        }
        Self {
            schema: SCHEMA,
            verdict: if bad { "fail" } else { "pass" },
            bad,
            reason,
            sample_rate_hz,
            channels,
            tick_ms,
            frames,
            samples,
            bad_frames: merge_frames(&idxs),
            checks,
        }
    }
}

pub fn merge_frames(idxs: &[u32]) -> Vec<FrameSpan> {
    if idxs.is_empty() {
        return Vec::new();
    }
    let mut idxs = idxs.to_vec();
    idxs.sort_unstable();
    idxs.dedup();
    let mut out = Vec::new();
    let mut start = idxs[0];
    let mut prev = idxs[0];
    for i in idxs.into_iter().skip(1) {
        if i == prev + 1 {
            prev = i;
        } else {
            out.push(FrameSpan {
                start,
                end: prev + 1,
            });
            start = i;
            prev = i;
        }
    }
    out.push(FrameSpan {
        start,
        end: prev + 1,
    });
    out
}

pub fn round6(x: f64) -> f64 {
    if !x.is_finite() {
        return x;
    }
    (x * 1_000_000.0).round() / 1_000_000.0
}
