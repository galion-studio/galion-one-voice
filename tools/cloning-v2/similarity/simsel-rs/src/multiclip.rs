use crate::prep::{declick, loudness_peak_normalize, speech_segments, trim_edges};
use crate::wav::{count_clicks, peak_dbfs, rms_dbfs, resample_linear, write_mono_s16, TARGET_SR};
use anyhow::Result;
use serde_json::json;
use std::path::Path;
use std::time::Instant;

fn crossfade_concat(parts: &[Vec<f32>], xf: usize) -> Vec<f32> {
    if parts.is_empty() {
        return Vec::new();
    }
    let mut out = parts[0].clone();
    for p in &parts[1..] {
        if out.len() < xf || p.len() < xf {
            out.extend_from_slice(p);
            continue;
        }
        let start = out.len() - xf;
        for i in 0..xf {
            let fo = 1.0 - (i as f32 / xf as f32);
            let fi = i as f32 / xf as f32;
            out[start + i] = out[start + i] * fo + p[i] * fi;
        }
        out.extend_from_slice(&p[xf..]);
    }
    out
}

fn peak_scale(part: &[f32], target_peak: f32) -> Vec<f32> {
    let peak = part.iter().map(|x| x.abs()).fold(0.0f32, f32::max) + 1e-9;
    let g = (target_peak / peak).min(1.0);
    part.iter().map(|x| x * g).collect()
}

/// Three overlapping windows (head/mid/tail) from the same original, crossfaded.
pub fn prep_multiclip_long(input: &Path, output: &Path) -> Result<serde_json::Value> {
    let t0 = Instant::now();
    let (raw, sr0) = crate::wav::read_mono_f32(input)?;
    let mono = if sr0 == TARGET_SR {
        raw
    } else {
        resample_linear(&raw, sr0, TARGET_SR)
    };
    let y0 = declick(&trim_edges(&mono, TARGET_SR, 0.05));
    let n = y0.len();
    let win = ((0.55 * n as f32) as usize).max((4.0 * TARGET_SR as f32) as usize).min(n);
    let mut starts = vec![
        0usize,
        ((0.20 * n as f32) as usize).min(n.saturating_sub(win)),
        n.saturating_sub(win),
    ];
    starts.sort_unstable();
    starts.dedup();
    let mut parts = Vec::new();
    let mut meta = Vec::new();
    for s in starts {
        let s = s.min(n.saturating_sub(win));
        let part = peak_scale(&y0[s..s + win], 0.75);
        meta.push(json!({
            "start": s as f64 / TARGET_SR as f64,
            "end": (s + win) as f64 / TARGET_SR as f64,
            "dur": win as f64 / TARGET_SR as f64,
        }));
        parts.push(part);
    }
    let xf = ((0.030 * TARGET_SR as f32) as usize).max(1);
    let concat = crossfade_concat(&parts, xf);
    let normed = loudness_peak_normalize(&concat, -20.0);
    write_mono_s16(output, &normed, TARGET_SR)?;
    let ms = t0.elapsed().as_secs_f64() * 1000.0;
    Ok(json!({
        "op": "prep_multiclip_long",
        "input": input.display().to_string(),
        "output": output.display().to_string(),
        "sr": TARGET_SR,
        "duration_s": normed.len() as f64 / TARGET_SR as f64,
        "peak_dbfs": peak_dbfs(&normed),
        "rms_dbfs": rms_dbfs(&normed),
        "clicks": count_clicks(&normed, 0.35),
        "segments": meta,
        "elapsed_ms": ms,
    }))
}

/// Phrase-based multi-clip: best SNR segments ≥1.2s in time order, append full if short.
pub fn prep_multiclip_phrases(input: &Path, output: &Path) -> Result<serde_json::Value> {
    let t0 = Instant::now();
    let (raw, sr0) = crate::wav::read_mono_f32(input)?;
    let mono = if sr0 == TARGET_SR {
        raw
    } else {
        resample_linear(&raw, sr0, TARGET_SR)
    };
    let y0 = declick(&trim_edges(&mono, TARGET_SR, 0.05));
    let segs = speech_segments(&y0, TARGET_SR, 1.2, 6.0);
    let mut chosen: Vec<(usize, usize)> = segs.iter().take(6).map(|(a, b, _)| (*a, *b)).collect();
    chosen.sort_by_key(|(a, _)| *a);
    let mut parts = Vec::new();
    let mut meta = Vec::new();
    let mut total = 0.0f64;
    for (a, b) in &chosen {
        let part = peak_scale(&y0[*a..*b], 0.75);
        let dur = (*b - *a) as f64 / TARGET_SR as f64;
        total += dur;
        meta.push(json!({"start": *a as f64 / TARGET_SR as f64, "end": *b as f64 / TARGET_SR as f64, "dur": dur}));
        parts.push(part);
    }
    if total < 12.0 {
        parts.push(peak_scale(&y0, 0.75));
        meta.push(json!({"start": 0.0, "end": y0.len() as f64 / TARGET_SR as f64, "dur": y0.len() as f64 / TARGET_SR as f64, "note": "full_append"}));
    }
    if parts.is_empty() {
        return prep_multiclip_long(input, output);
    }
    let xf = ((0.030 * TARGET_SR as f32) as usize).max(1);
    let concat = crossfade_concat(&parts, xf);
    let normed = loudness_peak_normalize(&concat, -20.0);
    write_mono_s16(output, &normed, TARGET_SR)?;
    let ms = t0.elapsed().as_secs_f64() * 1000.0;
    Ok(json!({
        "op": "prep_multiclip_phrases",
        "input": input.display().to_string(),
        "output": output.display().to_string(),
        "sr": TARGET_SR,
        "duration_s": normed.len() as f64 / TARGET_SR as f64,
        "peak_dbfs": peak_dbfs(&normed),
        "rms_dbfs": rms_dbfs(&normed),
        "clicks": count_clicks(&normed, 0.35),
        "segments": meta,
        "elapsed_ms": ms,
    }))
}
