// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

use std::time::Instant;

use crate::engine::Asr;
use crate::error::AsrError;
use crate::event::{AsrEvent, AsrKind};

/// Wall-clock record of one streaming pass. Model load is not included.
#[derive(Debug, Clone)]
pub struct StreamReport {
    pub events: Vec<AsrEvent>,
    /// One entry per `push_pcm` call. `finish` is `finish_us`, not a chunk.
    pub chunk_wall_us: Vec<u64>,
    pub finish_us: u64,
    pub wall_us: u64,
    pub audio_samples: usize,
    pub first_partial_wall_us: Option<u64>,
    pub first_partial_audio_ms: Option<u32>,
}

/// Push `pcm` in pieces of `chunk_samples`, then finish.
/// Real-time factor for this report is `wall_us` over the audio duration.
pub fn run_stream(
    engine: &mut dyn Asr,
    pcm: &[i16],
    chunk_samples: usize,
) -> Result<StreamReport, AsrError> {
    if chunk_samples == 0 {
        return Err(AsrError::new("chunk_samples is 0"));
    }
    let started = Instant::now();
    let mut events = Vec::new();
    let mut chunk_wall_us = Vec::new();
    let mut first_partial_wall_us = None;
    let mut first_partial_audio_ms = None;
    let mut off = 0;
    while off < pcm.len() {
        let end = (off + chunk_samples).min(pcm.len());
        let tick = Instant::now();
        let batch = engine.push_pcm(&pcm[off..end])?;
        chunk_wall_us.push(tick.elapsed().as_micros() as u64);
        note_partial(
            &batch,
            started,
            &mut first_partial_wall_us,
            &mut first_partial_audio_ms,
        );
        events.extend(batch);
        off = end;
    }
    let tick = Instant::now();
    let batch = engine.finish()?;
    let finish_us = tick.elapsed().as_micros() as u64;
    note_partial(
        &batch,
        started,
        &mut first_partial_wall_us,
        &mut first_partial_audio_ms,
    );
    events.extend(batch);
    Ok(StreamReport {
        events,
        chunk_wall_us,
        finish_us,
        wall_us: started.elapsed().as_micros() as u64,
        audio_samples: pcm.len(),
        first_partial_wall_us,
        first_partial_audio_ms,
    })
}

fn note_partial(
    batch: &[AsrEvent],
    started: Instant,
    wall: &mut Option<u64>,
    audio: &mut Option<u32>,
) {
    if wall.is_some() {
        return;
    }
    for ev in batch {
        if ev.kind == AsrKind::Partial && !ev.text.is_empty() {
            *wall = Some(started.elapsed().as_micros() as u64);
            *audio = Some(ev.audio_ms);
            return;
        }
    }
}

/// Non-empty finals, joined by a space. Partials are not included.
pub fn final_text(events: &[AsrEvent]) -> String {
    events
        .iter()
        .filter(|e| e.kind == AsrKind::Final && !e.text.is_empty())
        .map(|e| e.text.as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn real_time_factor(wall_us: u64, audio_samples: usize, sample_rate: u32) -> Option<f64> {
    if audio_samples == 0 || sample_rate == 0 {
        return None;
    }
    let audio_s = audio_samples as f64 / f64::from(sample_rate);
    Some((wall_us as f64 / 1_000_000.0) / audio_s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::HEARTBEAT_SAMPLES;
    use crate::fake::FakeAsr;

    #[test]
    fn fake_stream_emits_a_partial_before_the_final() {
        let mut fake = FakeAsr::new("one two");
        let pcm = vec![0i16; HEARTBEAT_SAMPLES * 2];
        let report = run_stream(&mut fake, &pcm, HEARTBEAT_SAMPLES).unwrap();
        assert_eq!(report.chunk_wall_us.len(), 2);
        assert_eq!(final_text(&report.events), "one two");
        let partials: Vec<_> = report
            .events
            .iter()
            .filter(|e| e.kind == AsrKind::Partial)
            .map(|e| e.text.as_str())
            .collect();
        assert_eq!(partials, vec!["one", "one two"]);
        assert!(report.first_partial_wall_us.is_some());
        assert_eq!(report.first_partial_audio_ms, Some(80));
        assert_eq!(real_time_factor(1_000_000, 16_000, 16_000), Some(1.0));
    }
}
