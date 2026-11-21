// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

use crate::event::SAMPLE_RATE;
use crate::json::{to_pretty, Json};
use crate::stream::real_time_factor;
use crate::wer::{corpus_counts, Wer};

#[derive(Debug, Clone)]
pub struct HostInfo {
    pub os: String,
    pub arch: String,
    pub cpu: String,
    pub cpus: usize,
    pub rustc: String,
}

#[derive(Debug, Clone)]
pub struct ClipScore {
    pub id: String,
    pub reference: String,
    pub hypothesis: String,
    pub wer: Wer,
    pub source_hz: u32,
    pub source_channels: u16,
    pub audio_samples: usize,
    pub wall_us: u64,
    pub finish_us: u64,
    pub chunk_wall_us: Vec<u64>,
    pub first_partial_wall_us: Option<u64>,
    pub first_partial_audio_ms: Option<u32>,
    pub partials: usize,
    pub finals: usize,
}

#[derive(Debug, Clone)]
pub struct RunScore {
    pub measured_at_utc: String,
    pub host: HostInfo,
    pub test_set: String,
    pub license: String,
    pub backend: String,
    pub model: String,
    pub threads: u32,
    pub chunk_ms: u32,
    pub load_us: u64,
    pub notes: Vec<String>,
    pub clips: Vec<ClipScore>,
}

/// Nearest-rank percentile. `percent` is 0 to 100. The series is sorted here.
pub fn percentile_us(samples: &[u64], percent: f64) -> Option<u64> {
    if samples.is_empty() || !(0.0..=100.0).contains(&percent) {
        return None;
    }
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let rank = ((percent / 100.0) * sorted.len() as f64).ceil() as usize;
    let idx = rank.saturating_sub(1).min(sorted.len() - 1);
    Some(sorted[idx])
}

impl RunScore {
    pub fn totals(&self) -> Wer {
        corpus_counts(&self.clips.iter().map(|c| c.wer).collect::<Vec<_>>())
    }

    pub fn audio_samples(&self) -> usize {
        self.clips.iter().map(|c| c.audio_samples).sum()
    }

    pub fn wall_us(&self) -> u64 {
        self.clips.iter().map(|c| c.wall_us).sum()
    }

    pub fn all_chunk_us(&self) -> Vec<u64> {
        self.clips
            .iter()
            .flat_map(|c| c.chunk_wall_us.iter().copied())
            .collect()
    }

    pub fn to_pretty_json(&self) -> String {
        to_pretty(&self.to_json())
    }

    pub fn summary(&self) -> String {
        let totals = self.totals();
        let chunks = self.all_chunk_us();
        let over = chunks.iter().filter(|us| **us > 80_000).count();
        let rtf = real_time_factor(self.wall_us(), self.audio_samples(), SAMPLE_RATE);
        let finish: Vec<u64> = self.clips.iter().map(|c| c.finish_us).collect();
        let first_wall: Vec<u64> = self
            .clips
            .iter()
            .filter_map(|c| c.first_partial_wall_us)
            .collect();
        let first_audio: Vec<u64> = self
            .clips
            .iter()
            .filter_map(|c| c.first_partial_audio_ms.map(|ms| u64::from(ms) * 1000))
            .collect();
        let mut lines = Vec::new();
        lines.push(format!("measured_at_utc: {}", self.measured_at_utc));
        lines.push(format!(
            "host: {} {} cpus={} cpu={}",
            self.host.os, self.host.arch, self.host.cpus, self.host.cpu
        ));
        lines.push(format!("rustc: {}", self.host.rustc));
        lines.push(format!("test_set: {}", self.test_set));
        lines.push(format!("license: {}", self.license));
        lines.push(format!("backend: {}", self.backend));
        lines.push(format!("model: {}", self.model));
        lines.push(format!("threads: {}", self.threads));
        lines.push(format!("chunk_ms: {}", self.chunk_ms));
        lines.push(format!("load_ms: {:.3}", self.load_us as f64 / 1000.0));
        lines.push(format!("clips: {}", self.clips.len()));
        lines.push(format!(
            "reference_words: {} errors: {} (sub {} del {} ins {})",
            totals.reference_words,
            totals.errors(),
            totals.substitutions,
            totals.deletions,
            totals.insertions
        ));
        lines.push(format!("micro_wer: {}", fmt_opt(totals.rate())));
        lines.push(format!(
            "audio_s: {:.3}",
            self.audio_samples() as f64 / f64::from(SAMPLE_RATE)
        ));
        lines.push(format!(
            "wall_s: {:.3}",
            self.wall_us() as f64 / 1_000_000.0
        ));
        lines.push(format!("rtf: {}", fmt_opt(rtf)));
        lines.push(format!("chunks: {}", chunks.len()));
        lines.push(format!(
            "chunk_wall_ms_p50: {}",
            fmt_us(percentile_us(&chunks, 50.0))
        ));
        lines.push(format!(
            "chunk_wall_ms_p95: {}",
            fmt_us(percentile_us(&chunks, 95.0))
        ));
        lines.push(format!("chunks_over_80ms: {over}"));
        lines.push(format!(
            "finish_ms_p50: {}",
            fmt_us(percentile_us(&finish, 50.0))
        ));
        lines.push(format!(
            "first_partial_wall_ms_p50: {}",
            fmt_us(percentile_us(&first_wall, 50.0))
        ));
        lines.push(format!(
            "first_partial_audio_ms_p50: {}",
            fmt_us(percentile_us(&first_audio, 50.0))
        ));
        for clip in &self.clips {
            let clip_rtf = real_time_factor(clip.wall_us, clip.audio_samples, SAMPLE_RATE);
            lines.push(format!(
                "{} wer={} ({}/{}) audio_ms={} wall_ms={:.1} rtf={} first_partial_wall_ms={} first_partial_audio_ms={} hyp={}",
                clip.id,
                fmt_opt(clip.wer.rate()),
                clip.wer.errors(),
                clip.wer.reference_words,
                crate::event::samples_to_ms(clip.audio_samples),
                clip.wall_us as f64 / 1000.0,
                fmt_opt(clip_rtf),
                fmt_us(clip.first_partial_wall_us),
                clip.first_partial_audio_ms.map(|n| n.to_string()).unwrap_or_else(|| "none".into()),
                clip.hypothesis
            ));
        }
        lines.join("\n")
    }

    fn to_json(&self) -> Json {
        let totals = self.totals();
        let chunks = self.all_chunk_us();
        let over = chunks.iter().filter(|us| **us > 80_000).count();
        Json::Obj(vec![
            (
                "measured_at_utc".into(),
                Json::Str(self.measured_at_utc.clone()),
            ),
            (
                "host".into(),
                Json::Obj(vec![
                    ("os".into(), Json::Str(self.host.os.clone())),
                    ("arch".into(), Json::Str(self.host.arch.clone())),
                    ("cpu".into(), Json::Str(self.host.cpu.clone())),
                    ("cpus".into(), Json::Num(self.host.cpus as f64)),
                    ("rustc".into(), Json::Str(self.host.rustc.clone())),
                ]),
            ),
            ("test_set".into(), Json::Str(self.test_set.clone())),
            ("license".into(), Json::Str(self.license.clone())),
            ("backend".into(), Json::Str(self.backend.clone())),
            ("model".into(), Json::Str(self.model.clone())),
            ("threads".into(), Json::Num(self.threads as f64)),
            ("chunk_ms".into(), Json::Num(self.chunk_ms as f64)),
            ("sample_rate".into(), Json::Num(SAMPLE_RATE as f64)),
            ("load_us".into(), Json::Num(self.load_us as f64)),
            (
                "reference_words".into(),
                Json::Num(totals.reference_words as f64),
            ),
            (
                "substitutions".into(),
                Json::Num(totals.substitutions as f64),
            ),
            ("deletions".into(), Json::Num(totals.deletions as f64)),
            ("insertions".into(), Json::Num(totals.insertions as f64)),
            ("errors".into(), Json::Num(totals.errors() as f64)),
            ("micro_wer".into(), opt_num(totals.rate())),
            (
                "audio_samples".into(),
                Json::Num(self.audio_samples() as f64),
            ),
            ("wall_us".into(), Json::Num(self.wall_us() as f64)),
            (
                "rtf".into(),
                opt_num(real_time_factor(
                    self.wall_us(),
                    self.audio_samples(),
                    SAMPLE_RATE,
                )),
            ),
            ("chunks".into(), Json::Num(chunks.len() as f64)),
            (
                "chunk_wall_us_p50".into(),
                opt_u64(percentile_us(&chunks, 50.0)),
            ),
            (
                "chunk_wall_us_p95".into(),
                opt_u64(percentile_us(&chunks, 95.0)),
            ),
            ("chunks_over_80ms".into(), Json::Num(over as f64)),
            (
                "notes".into(),
                Json::Arr(self.notes.iter().cloned().map(Json::Str).collect()),
            ),
            (
                "clips".into(),
                Json::Arr(self.clips.iter().map(clip_json).collect()),
            ),
        ])
    }
}

fn clip_json(clip: &ClipScore) -> Json {
    Json::Obj(vec![
        ("id".into(), Json::Str(clip.id.clone())),
        ("reference".into(), Json::Str(clip.reference.clone())),
        ("hypothesis".into(), Json::Str(clip.hypothesis.clone())),
        (
            "reference_words".into(),
            Json::Num(clip.wer.reference_words as f64),
        ),
        (
            "substitutions".into(),
            Json::Num(clip.wer.substitutions as f64),
        ),
        ("deletions".into(), Json::Num(clip.wer.deletions as f64)),
        ("insertions".into(), Json::Num(clip.wer.insertions as f64)),
        ("wer".into(), opt_num(clip.wer.rate())),
        ("source_hz".into(), Json::Num(clip.source_hz as f64)),
        (
            "source_channels".into(),
            Json::Num(clip.source_channels as f64),
        ),
        ("audio_samples".into(), Json::Num(clip.audio_samples as f64)),
        ("wall_us".into(), Json::Num(clip.wall_us as f64)),
        ("finish_us".into(), Json::Num(clip.finish_us as f64)),
        (
            "rtf".into(),
            opt_num(real_time_factor(
                clip.wall_us,
                clip.audio_samples,
                SAMPLE_RATE,
            )),
        ),
        (
            "first_partial_wall_us".into(),
            opt_u64(clip.first_partial_wall_us),
        ),
        (
            "first_partial_audio_ms".into(),
            match clip.first_partial_audio_ms {
                Some(ms) => Json::Num(ms as f64),
                None => Json::Null,
            },
        ),
        ("partials".into(), Json::Num(clip.partials as f64)),
        ("finals".into(), Json::Num(clip.finals as f64)),
        (
            "chunk_wall_us".into(),
            Json::Arr(
                clip.chunk_wall_us
                    .iter()
                    .copied()
                    .map(|n| Json::Num(n as f64))
                    .collect(),
            ),
        ),
    ])
}

fn opt_num(value: Option<f64>) -> Json {
    match value {
        Some(n) => Json::Num(n),
        None => Json::Null,
    }
}

fn opt_u64(value: Option<u64>) -> Json {
    match value {
        Some(n) => Json::Num(n as f64),
        None => Json::Null,
    }
}

fn fmt_opt(value: Option<f64>) -> String {
    match value {
        Some(n) => format!("{n:.6}"),
        None => "none".into(),
    }
}

fn fmt_us(value: Option<u64>) -> String {
    match value {
        Some(us) => format!("{:.3}", us as f64 / 1000.0),
        None => "none".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_rank_percentile() {
        let samples = [10u64, 40, 20, 30];
        assert_eq!(percentile_us(&samples, 50.0), Some(20));
        assert_eq!(percentile_us(&samples, 95.0), Some(40));
        assert_eq!(percentile_us(&[], 50.0), None);
    }
}
