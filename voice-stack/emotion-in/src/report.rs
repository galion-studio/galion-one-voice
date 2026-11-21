// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! JSON and timing helpers. No serde. The text is numbers and fixed labels.

use crate::map::{FEATURES, NOTES};
use crate::stream::Reading;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StageTimes {
    pub frames: u32,
    pub pitch_median_us: u32,
    pub pitch_p95_us: u32,
    pub pitch_sum_us: u32,
    pub pitch_batch_us: u32,
    pub energy_median_us: u32,
    pub energy_p95_us: u32,
    pub energy_sum_us: u32,
    pub energy_batch_us: u32,
    pub push_median_us: u32,
    pub push_p95_us: u32,
    pub push_sum_us: u32,
    pub push_batch_us: u32,
    pub tick_median_us: u32,
    pub tick_p95_us: u32,
    pub tick_sum_us: u32,
    pub tick_batch_us: u32,
}

/// Prosody reading plus the timing of the `push` calls that produced it.
/// `audio_retained_samples` is always 0.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Report {
    pub reading: Reading,
    pub frames: u32,
    pub dropped_tail_samples: u32,
    pub audio_retained_samples: u32,
    pub push_median_us: u32,
    pub push_p95_us: u32,
}

impl Report {
    pub fn from_reading(
        reading: Reading,
        frames: u32,
        dropped_tail_samples: u32,
        push_us: &[u32],
    ) -> Self {
        Self {
            reading,
            frames,
            dropped_tail_samples,
            audio_retained_samples: 0,
            push_median_us: percentile(push_us, 50),
            push_p95_us: percentile(push_us, 95),
        }
    }
}

pub fn elapsed_us(started: Instant) -> u32 {
    u32::try_from(started.elapsed().as_micros()).unwrap_or(u32::MAX)
}

/// `pct` is 0..=100. Empty input is 0. Index is `((n - 1) * pct) / 100`.
pub fn percentile(samples: &[u32], pct: u32) -> u32 {
    if samples.is_empty() {
        return 0;
    }
    let mut v = samples.to_vec();
    v.sort_unstable();
    let pct = u64::from(pct.min(100));
    // Nearest-rank: ceil(pct/100 * n), then back to an index.
    let rank = (pct * v.len() as u64 + 99) / 100;
    let i = rank.saturating_sub(1).min(v.len() as u64 - 1);
    v[i as usize]
}

fn f32_json(x: f32) -> String {
    if x.is_finite() {
        format!("{x:.4}")
    } else {
        "null".into()
    }
}

pub fn reading_json(reading: &Reading) -> String {
    let p = &reading.prosody;
    let m = &reading.mapped;
    let mut s = String::new();
    s.push('{');
    push_u(&mut s, "t_ms", reading.t_ms);
    push_u(&mut s, "frames", reading.frames);
    s.push_str("\"audio_retained_samples\":0,");
    s.push_str("\"prosody\":{");
    push_f(&mut s, "f0_hz", p.f0_hz);
    push_f(&mut s, "f0_range_hz", p.f0_range_hz);
    push_f(&mut s, "rms", p.rms);
    push_f(&mut s, "rms_dbfs", p.rms_dbfs);
    push_f(&mut s, "speech_rate_hz", p.speech_rate_hz);
    push_f(&mut s, "articulation_hz", p.articulation_hz);
    push_f(&mut s, "pause_ratio", p.pause_ratio);
    push_u(&mut s, "longest_pause_ms", p.longest_pause_ms);
    push_f(&mut s, "voiced_ratio", p.voiced_ratio);
    push_f(&mut s, "zcr", p.zcr);
    push_f(&mut s, "harmonicity", p.harmonicity);
    push_u(&mut s, "nuclei", p.nuclei);
    push_u(&mut s, "voiced_frames", p.voiced_frames);
    push_u(&mut s, "window_ms", p.window_ms);
    trim_comma(&mut s);
    s.push_str("},\"norm\":{");
    for (i, name) in FEATURES.iter().enumerate() {
        s.push('"');
        s.push_str(name);
        s.push_str("\":");
        s.push_str(&f32_json(m.norm[i]));
        s.push(',');
    }
    trim_comma(&mut s);
    s.push_str("},\"notes\":{");
    for (i, name) in FEATURES.iter().enumerate() {
        s.push('"');
        s.push_str(name);
        s.push_str("\":");
        push_str(&mut s, NOTES[i]);
        s.push(',');
    }
    trim_comma(&mut s);
    s.push_str("},\"anger_press\":");
    s.push_str(&f32_json(m.anger_press));
    s.push_str(",\"confidence\":");
    s.push_str(&f32_json(m.confidence));
    s.push_str(",\"classifier\":");
    push_str(&mut s, m.classifier.as_str());
    s.push_str(",\"estimate\":");
    pad_json(
        &mut s,
        m.estimate.arousal,
        m.estimate.valence,
        m.estimate.dominance,
    );
    s.push_str(",\"mood_target\":");
    pad_json(
        &mut s,
        m.target.arousal,
        m.target.valence,
        m.target.dominance,
    );
    s.push('}');
    s
}

pub fn report_json(report: &Report) -> String {
    let mut inner = reading_json(&report.reading);
    // reading_json ends with '}'. Insert the file-level counters before it.
    inner.pop();
    inner.push_str(",\"analysed_frames\":");
    inner.push_str(&report.frames.to_string());
    inner.push_str(",\"dropped_tail_samples\":");
    inner.push_str(&report.dropped_tail_samples.to_string());
    inner.push_str(",\"push_median_us\":");
    inner.push_str(&report.push_median_us.to_string());
    inner.push_str(",\"push_p95_us\":");
    inner.push_str(&report.push_p95_us.to_string());
    inner.push('}');
    inner
}

fn pad_json(s: &mut String, arousal: f32, valence: f32, dominance: f32) {
    s.push_str("{\"arousal\":");
    s.push_str(&f32_json(arousal));
    s.push_str(",\"valence\":");
    s.push_str(&f32_json(valence));
    s.push_str(",\"dominance\":");
    s.push_str(&f32_json(dominance));
    s.push('}');
}

fn push_f(s: &mut String, key: &str, v: f32) {
    s.push('"');
    s.push_str(key);
    s.push_str("\":");
    s.push_str(&f32_json(v));
    s.push(',');
}

fn push_u(s: &mut String, key: &str, v: u32) {
    s.push('"');
    s.push_str(key);
    s.push_str("\":");
    s.push_str(&v.to_string());
    s.push(',');
}

fn push_str(s: &mut String, v: &str) {
    s.push('"');
    for c in v.chars() {
        match c {
            '"' => s.push_str("\\\""),
            '\\' => s.push_str("\\\\"),
            '\n' => s.push_str("\\n"),
            c => s.push(c),
        }
    }
    s.push('"');
}

fn trim_comma(s: &mut String) {
    if s.ends_with(',') {
        s.pop();
    }
}

/// One-dot view. The dot colour stays `#F5F5F7` scaled by `brightness`
/// (same rule as `galion-render` `palette::DOT`). `wash` is background only.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DotView {
    pub state: &'static str,
    pub radius: f32,
    pub brightness: f32,
    pub wash: &'static str,
    pub level: f32,
}

pub fn dot_view(reading: &Reading) -> DotView {
    let listening = reading.mapped.confidence >= 0.20;
    let level = if reading.prosody.rms.is_finite() {
        reading.prosody.rms.clamp(0.0, 1.0)
    } else {
        0.0
    };
    if listening {
        DotView {
            state: "listening",
            radius: 0.120 + 0.004 * level,
            brightness: 0.96,
            wash: wash_name(reading.mapped.target.valence),
            level,
        }
    } else {
        DotView {
            state: "idle",
            radius: 0.100,
            brightness: 0.84,
            wash: "#0A0A0C",
            level,
        }
    }
}

fn wash_name(valence: f32) -> &'static str {
    if valence >= 0.15 {
        "#2A2118"
    } else if valence <= -0.15 {
        "#1A1424"
    } else {
        "#0A0A0C"
    }
}

pub fn dot_json(reading: &Reading, report_extra: Option<&Report>) -> String {
    let dot = dot_view(reading);
    let mut body = reading_json(reading);
    body.pop();
    body.push_str(",\"state\":");
    push_str(&mut body, dot.state);
    body.push_str(",\"radius\":");
    body.push_str(&f32_json(dot.radius));
    body.push_str(",\"brightness\":");
    body.push_str(&f32_json(dot.brightness));
    body.push_str(",\"wash\":");
    push_str(&mut body, dot.wash);
    body.push_str(",\"level\":");
    body.push_str(&f32_json(dot.level));
    if let Some(report) = report_extra {
        body.push_str(",\"push_median_us\":");
        body.push_str(&report.push_median_us.to_string());
        body.push_str(",\"push_p95_us\":");
        body.push_str(&report.push_p95_us.to_string());
        body.push_str(",\"dropped_tail_samples\":");
        body.push_str(&report.dropped_tail_samples.to_string());
    }
    body.push('}');
    body
}

pub fn stage_text(t: &StageTimes) -> String {
    format!(
        "frames {n}\n\
         pitch    median {pmed} us  p95 {pp95} us  sum_of_calls {psum} us  batch {pbatch} us\n\
         energy   median {emed} us  p95 {ep95} us  sum_of_calls {esum} us  batch {ebatch} us\n\
         push     median {umed} us  p95 {up95} us  sum_of_calls {usum} us  batch {ubatch} us\n\
         moodtick median {tmed} us  p95 {tp95} us  sum_of_calls {tsum} us  batch {tbatch} us\n\
         stage bar {bar} us\n\
         per-call medians below 1 us record as 0; batch is one clock around the loop",
        n = t.frames,
        pmed = t.pitch_median_us,
        pp95 = t.pitch_p95_us,
        psum = t.pitch_sum_us,
        pbatch = t.pitch_batch_us,
        emed = t.energy_median_us,
        ep95 = t.energy_p95_us,
        esum = t.energy_sum_us,
        ebatch = t.energy_batch_us,
        umed = t.push_median_us,
        up95 = t.push_p95_us,
        usum = t.push_sum_us,
        ubatch = t.push_batch_us,
        tmed = t.tick_median_us,
        tp95 = t.tick_p95_us,
        tsum = t.tick_sum_us,
        tbatch = t.tick_batch_us,
        bar = crate::clock::STAGE_BUDGET_US,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentile_edges() {
        assert_eq!(percentile(&[], 95), 0);
        assert_eq!(percentile(&[5, 1, 9, 3], 50), 3);
        assert_eq!(percentile(&[5, 1, 9, 3], 95), 9);
    }
}
