use crate::wav::{count_clicks, peak_dbfs, rms_dbfs, resample_linear, write_mono_s16, TARGET_SR};
use anyhow::Result;
use serde_json::json;
use std::path::Path;
use std::time::Instant;

const TP_LIM: f32 = 0.89125094; // 10^(-1/20) ≈ -1 dBTP
const CLICK_THR: f32 = 0.35;

fn frame_rms(y: &[f32], sr: u32, frame_s: f32, hop_s: f32) -> (Vec<f32>, usize) {
    let n = ((sr as f32) * frame_s) as usize;
    let h = ((sr as f32) * hop_s) as usize;
    let h = h.max(1);
    let mut rms = Vec::new();
    let mut i = 0;
    while i + n <= y.len() {
        let seg = &y[i..i + n];
        let m = seg.iter().map(|x| x * x).sum::<f32>() / n as f32;
        rms.push(m.sqrt());
        i += h;
    }
    if rms.is_empty() && !y.is_empty() {
        let m = y.iter().map(|x| x * x).sum::<f32>() / y.len() as f32;
        rms.push(m.sqrt());
    }
    (rms, h)
}

fn quantile(v: &mut [f32], q: f32) -> f32 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let idx = ((v.len() - 1) as f32 * q).round() as usize;
    v[idx.min(v.len() - 1)]
}

pub fn trim_edges(y: &[f32], sr: u32, pad_s: f32) -> Vec<f32> {
    let (rms, hop) = frame_rms(y, sr, 0.025, 0.010);
    let mut tmp = rms.clone();
    let q = quantile(&mut tmp, 0.15);
    let thr = (q * 3.0).max(1e-5);
    let speech: Vec<usize> = rms
        .iter()
        .enumerate()
        .filter(|(_, r)| **r > thr)
        .map(|(i, _)| i)
        .collect();
    if speech.is_empty() {
        return y.to_vec();
    }
    let pad = (pad_s * sr as f32) as usize;
    let a = speech[0].saturating_mul(hop).saturating_sub(pad);
    let b = (speech[speech.len() - 1] * hop + pad).min(y.len());
    y[a..b].to_vec()
}

pub fn declick(y: &[f32]) -> Vec<f32> {
    let mut out = y.to_vec();
    for i in 1..out.len() {
        if (out[i] - out[i - 1]).abs() > CLICK_THR {
            let lo = i.saturating_sub(2);
            let hi = (i + 3).min(out.len());
            let mut w = out[lo..hi].to_vec();
            w.sort_by(|a, b| a.partial_cmp(b).unwrap());
            out[i] = w[w.len() / 2];
        }
    }
    out
}

/// Loudness-ish normalize: RMS target then true-peak (peak) ≤ -1 dBFS.
pub fn loudness_peak_normalize(y: &[f32], target_rms_db: f32) -> Vec<f32> {
    let rms = (y.iter().map(|x| x * x).sum::<f32>() / y.len().max(1) as f32).sqrt() + 1e-12;
    let target_rms = 10f32.powf(target_rms_db / 20.0);
    let mut out: Vec<f32> = y.iter().map(|x| x * (target_rms / rms)).collect();
    let peak = out.iter().map(|x| x.abs()).fold(0.0f32, f32::max) + 1e-12;
    if peak > TP_LIM {
        let g = TP_LIM / peak;
        for x in &mut out {
            *x *= g;
        }
    }
    out
}

pub fn prep_cleaned(input: &Path, output: &Path) -> Result<serde_json::Value> {
    let t0 = Instant::now();
    let (raw, sr0) = crate::wav::read_mono_f32(input)?;
    let mono = if sr0 == TARGET_SR {
        raw
    } else {
        resample_linear(&raw, sr0, TARGET_SR)
    };
    let trimmed = trim_edges(&mono, TARGET_SR, 0.05);
    let cleaned = declick(&trimmed);
    let normed = loudness_peak_normalize(&cleaned, -20.0);
    write_mono_s16(output, &normed, TARGET_SR)?;
    let ms = t0.elapsed().as_secs_f64() * 1000.0;
    Ok(json!({
        "op": "prep_cleaned",
        "input": input.display().to_string(),
        "output": output.display().to_string(),
        "sr": TARGET_SR,
        "duration_s": normed.len() as f64 / TARGET_SR as f64,
        "peak_dbfs": peak_dbfs(&normed),
        "rms_dbfs": rms_dbfs(&normed),
        "clicks": count_clicks(&normed, CLICK_THR),
        "elapsed_ms": ms,
    }))
}

pub fn speech_segments(y: &[f32], sr: u32, min_dur: f32, max_dur: f32) -> Vec<(usize, usize, f32)> {
    let (rms, hop) = frame_rms(y, sr, 0.025, 0.010);
    let mut tmp = rms.clone();
    let q = quantile(&mut tmp, 0.15);
    let thr = (q * 4.0).max(1e-5);
    let mut segs = Vec::new();
    let mut i = 0;
    while i < rms.len() {
        if rms[i] <= thr {
            i += 1;
            continue;
        }
        let mut j = i;
        while j < rms.len() && rms[j] > thr {
            j += 1;
        }
        let pad = (0.04 * sr as f32) as usize;
        let mut a = (i * hop).saturating_sub(pad);
        let mut b = (j * hop + pad).min(y.len());
        let dur = (b - a) as f32 / sr as f32;
        if dur >= min_dur {
            if dur > max_dur {
                let win = (max_dur * sr as f32) as usize;
                let mut best = -1.0f32;
                let mut best_s = a;
                let step = hop * 5;
                let mut s = a;
                while s + win <= b {
                    let e: f32 = y[s..s + win].iter().map(|x| x * x).sum::<f32>() / win as f32;
                    if e > best {
                        best = e;
                        best_s = s;
                    }
                    s += step.max(1);
                }
                a = best_s;
                b = best_s + win;
            }
            let snr = {
                let seg_rms = (y[a..b].iter().map(|x| x * x).sum::<f32>() / (b - a) as f32).sqrt();
                20.0 * ((seg_rms + 1e-12) / (q + 1e-12)).log10()
            };
            segs.push((a, b, snr));
        }
        i = j;
    }
    segs.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());
    segs
}
