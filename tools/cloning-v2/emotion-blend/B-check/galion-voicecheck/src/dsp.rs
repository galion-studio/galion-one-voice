//! Pure-Rust DSP for the two built-in fix stages:
//!   (a) builtin reference: pick the base recording's most expressive phrases
//!   (b) builtin light prosody layer: gentle energy expansion + pause lengthening
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::path::Path;

pub fn read_mono(path: &Path) -> Result<(Vec<f32>, u32)> {
    let mut r = hound::WavReader::open(path).with_context(|| format!("open {}", path.display()))?;
    let spec = r.spec();
    let ch = spec.channels.max(1) as usize;
    let raw: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => r.samples::<f32>().collect::<Result<_, _>>()?,
        hound::SampleFormat::Int => {
            let scale = (1i64 << (spec.bits_per_sample - 1)) as f32;
            r.samples::<i32>().map(|s| s.map(|v| v as f32 / scale)).collect::<Result<_, _>>()?
        }
    };
    let mono = raw.chunks(ch).map(|c| c.iter().sum::<f32>() / ch as f32).collect();
    Ok((mono, spec.sample_rate))
}

pub fn write_mono16(path: &Path, x: &[f32], sr: u32) -> Result<()> {
    let spec = hound::WavSpec { channels: 1, sample_rate: sr, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
    let mut w = hound::WavWriter::create(path, spec)?;
    for &s in x {
        w.write_sample((s.clamp(-1.0, 1.0) * 32767.0).round() as i16)?;
    }
    w.finalize()?;
    Ok(())
}

pub struct Frames {
    pub hop: usize,
    pub db: Vec<f64>,
    pub f0: Vec<Option<f64>>,
}

fn percentile(v: &[f64], p: f64) -> f64 {
    if v.is_empty() { return f64::NAN; }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let i = ((s.len() - 1) as f64 * p).round() as usize;
    s[i.min(s.len() - 1)]
}
fn mean(v: &[f64]) -> f64 { if v.is_empty() { f64::NAN } else { v.iter().sum::<f64>() / v.len() as f64 } }
fn sd(v: &[f64]) -> f64 {
    if v.len() < 2 { return 0.0; }
    let m = mean(v);
    (v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (v.len() - 1) as f64).sqrt()
}

/// 10 ms hop. Level in dBFS over 30 ms; F0 by normalised autocorrelation (60–500 Hz, 40 ms window).
pub fn analyse(x: &[f32], sr: u32, want_f0: bool) -> Frames {
    let hop = (sr as usize) / 100;
    let win = (sr as usize) * 3 / 100;
    let fwin = (sr as usize) * 4 / 100;
    let n = if x.len() > win { (x.len() - win) / hop + 1 } else { 1 };
    let mut db = Vec::with_capacity(n);
    for i in 0..n {
        let s = &x[i * hop..(i * hop + win).min(x.len())];
        let e = s.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / s.len().max(1) as f64;
        db.push(10.0 * (e + 1e-12).log10());
    }
    let floor = percentile(&db, 0.95) - 30.0;
    let lmin = sr as usize / 500;
    let lmax = sr as usize / 60;
    let mut f0 = vec![None; n];
    if want_f0 {
        for i in 0..n {
            if db[i] < floor { continue; }
            let st = i * hop;
            if st + fwin + lmax > x.len() { break; }
            let a = &x[st..st + fwin];
            let e0: f64 = a.iter().map(|v| (*v as f64).powi(2)).sum();
            let (mut best, mut bl) = (0.0f64, 0usize);
            for l in lmin..=lmax {
                let b = &x[st + l..st + l + fwin];
                let (mut c, mut e1) = (0.0f64, 0.0f64);
                for k in 0..fwin { c += a[k] as f64 * b[k] as f64; e1 += (b[k] as f64).powi(2); }
                let r = c / (e0 * e1 + 1e-12).sqrt();
                if r > best { best = r; bl = l; }
            }
            if best > 0.6 && bl > 0 { f0[i] = Some(sr as f64 / bl as f64); }
        }
    }
    Frames { hop, db, f0 }
}

fn st(v: f64) -> f64 { 12.0 * (v / 100.0).log2() }

#[derive(Clone, Debug)]
struct Seg { a: usize, b: usize, score: f64, f0sd: f64, f0rng: f64, esd: f64, lvl: f64 }

/// Speech segments between pauses (>= 0.2 s below floor). Frame indices [a,b).
fn segments(fr: &Frames, floor: f64, min_pause_frames: usize) -> Vec<(usize, usize)> {
    let n = fr.db.len();
    let mut out = Vec::new();
    let mut i = 0;
    while i < n {
        while i < n && fr.db[i] < floor { i += 1; }
        if i >= n { break; }
        let a = i;
        let mut last_voiced = i;
        let mut sil = 0;
        while i < n {
            if fr.db[i] < floor { sil += 1; if sil >= min_pause_frames { break; } } else { sil = 0; last_voiced = i; }
            i += 1;
        }
        out.push((a, last_voiced + 1));
    }
    out
}

/// Builtin fix (a): conditioning reference from the most expressive moments of the base.
pub fn select_expressive(x: &[f32], sr: u32, target_s: f64, max_seg_s: f64) -> (Vec<f32>, Value) {
    let mut fr = analyse(x, sr, true);
    // drop likely octave errors: keep F0 within ±10 st of the global median
    let all: Vec<f64> = fr.f0.iter().flatten().cloned().collect();
    let med = percentile(&all, 0.5);
    if med.is_finite() { for v in fr.f0.iter_mut() { if let Some(f) = *v { if (st(f) - st(med)).abs() > 10.0 { *v = None; } } } }
    let floor = percentile(&fr.db, 0.95) - 35.0;
    let mut raw = segments(&fr, floor, 20);
    // split long segments at the quietest frame in the middle half
    let maxf = (max_seg_s * 100.0) as usize;
    let mut i = 0;
    while i < raw.len() {
        let (a, b) = raw[i];
        if b - a > maxf {
            let lo = a + (b - a) / 4; let hi = b - (b - a) / 4;
            let cut = (lo..hi).min_by(|&p, &q| fr.db[p].partial_cmp(&fr.db[q]).unwrap()).unwrap();
            raw[i] = (a, cut); raw.insert(i + 1, (cut, b));
        } else { i += 1; }
    }
    let mut segs: Vec<Seg> = raw.into_iter().filter(|(a, b)| b - a >= 80).map(|(a, b)| {
        let f: Vec<f64> = fr.f0[a..b].iter().flatten().map(|v| st(*v)).collect();
        let e: Vec<f64> = fr.db[a..b].iter().cloned().filter(|v| *v >= floor).collect();
        Seg { a, b, score: 0.0, f0sd: sd(&f), f0rng: if f.len() > 5 { percentile(&f, 0.95) - percentile(&f, 0.05) } else { 0.0 }, esd: sd(&e), lvl: mean(&e) }
    }).collect();
    let total_speech: f64 = segs.iter().map(|s| (s.b - s.a) as f64 / 100.0).sum();
    // be selective on short bases: aim for 60 % of the speech, clamped to [6 s, target_s]
    let target_s = (0.6 * total_speech).clamp(6.0f64.min(total_speech), target_s);
    let z = |vals: Vec<f64>| -> Vec<f64> { let m = mean(&vals); let s = sd(&vals).max(1e-9); vals.iter().map(|v| (v - m) / s).collect() };
    let zf = z(segs.iter().map(|s| s.f0sd).collect());
    let zr = z(segs.iter().map(|s| s.f0rng).collect());
    let ze = z(segs.iter().map(|s| s.esd).collect());
    let zl = z(segs.iter().map(|s| s.lvl).collect());
    for (k, s) in segs.iter_mut().enumerate() { s.score = zf[k] + zr[k] + ze[k] + 0.5 * zl[k]; }
    let mut order: Vec<usize> = (0..segs.len()).collect();
    order.sort_by(|&p, &q| segs[q].score.partial_cmp(&segs[p].score).unwrap());
    let mut chosen = Vec::new(); let mut acc = 0.0;
    for &k in &order { if acc >= target_s { break; } chosen.push(k); acc += (segs[k].b - segs[k].a) as f64 / 100.0; }
    // most expressive phrase FIRST: the Turbo speech encoder only sees the first 6 s (audio_features)
    // / 10 s (tokens, speaker features) of the conditioning wav (A-global encoder-graph evidence)
    chosen.sort_by(|&p, &q| segs[q].score.partial_cmp(&segs[p].score).unwrap());
    let hop = fr.hop; let pad = sr as usize / 20; let fade = sr as usize * 15 / 1000; let gap = sr as usize * 12 / 100;
    let mut out: Vec<f32> = Vec::new();
    for &k in &chosen {
        let s0 = (segs[k].a * hop).saturating_sub(pad);
        let s1 = (segs[k].b * hop + pad + hop * 3).min(x.len());
        let mut piece = x[s0..s1].to_vec();
        let l = piece.len();
        for j in 0..fade.min(l / 2) { let g = j as f32 / fade as f32; piece[j] *= g; piece[l - 1 - j] *= g; }
        if !out.is_empty() { out.extend(std::iter::repeat(0.0).take(gap)); }
        out.extend(piece);
    }
    let meta = json!({
        "method": "builtin expressive-moments: phrases split on >=0.2 s pauses (floor p95-35 dB), long phrases split <= max_seg_s; score = z(F0 sd st)+z(F0 p5-p95 range st)+z(energy sd dB)+0.5*z(level); top phrases until target_s, ordered most-expressive-first (encoder crops 6 s/10 s), 15 ms fades, 120 ms gaps",
        "target_s": target_s, "max_seg_s": max_seg_s,
        "base_speech_s": total_speech, "n_phrases": segs.len(), "n_chosen": chosen.len(),
        "out_s": out.len() as f64 / sr as f64,
        "phrases": segs.iter().enumerate().map(|(k, s)| json!({"start_s": s.a as f64/100.0, "end_s": s.b as f64/100.0, "score": s.score, "f0_sd_st": s.f0sd, "f0_range_st": s.f0rng, "energy_sd_db": s.esd, "chosen": chosen.contains(&k)})).collect::<Vec<_>>(),
    });
    (out, meta)
}

#[derive(Clone, Debug)]
pub struct ProsodyParams {
    pub strength: f64,         // 0..1, fraction of the base-vs-render gap to close
    pub max_expand: f64,       // max energy expansion factor
    pub max_gain_db: f64,      // per-frame gain clamp
    pub max_gap_add_s: f64,    // max silence added per pause
    pub max_total_add_frac: f64,
}
impl Default for ProsodyParams {
    fn default() -> Self { ProsodyParams { strength: 0.5, max_expand: 1.2, max_gain_db: 3.0, max_gap_add_s: 0.15, max_total_add_frac: 0.10 } }
}

/// Builtin fix (b): light prosody layer on one rendered line.
/// `render_dr` / `render_pause` = this line's energy dynamic range (dB) and pause ratio;
/// `base_dr` / `base_pause` = the base recording's. No pitch / spectral change.
pub fn prosody_layer(x: &[f32], sr: u32, p: &ProsodyParams, render_dr: f64, base_dr: f64, render_pause: f64, base_pause: f64) -> (Vec<f32>, Value) {
    let fr = analyse(x, sr, false);
    let hop = fr.hop;
    let n = fr.db.len();
    let floor = percentile(&fr.db, 0.95) - 30.0;
    // 1. energy expansion
    let k = if render_dr.is_finite() && base_dr.is_finite() && render_dr > 1.0 {
        (1.0 + p.strength * (base_dr / render_dr - 1.0)).clamp(1.0, p.max_expand)
    } else { 1.0 };
    let act: Vec<f64> = fr.db.iter().cloned().filter(|v| *v >= floor).collect();
    let lref = percentile(&act, 0.5);
    let mut g: Vec<f64> = fr.db.iter().map(|d| ((k - 1.0) * (d.max(floor) - lref)).clamp(-p.max_gain_db, p.max_gain_db)).collect();
    // smooth 5 frames
    let g0 = g.clone();
    for i in 0..n { let a = i.saturating_sub(2); let b = (i + 3).min(n); g[i] = g0[a..b].iter().sum::<f64>() / (b - a) as f64; }
    let mut y: Vec<f32> = x.iter().enumerate().map(|(s, v)| {
        let fpos = (s as f64 / hop as f64).min((n - 1) as f64);
        let i0 = fpos.floor() as usize; let i1 = (i0 + 1).min(n - 1); let t = fpos - i0 as f64;
        let gdb = g[i0] * (1.0 - t) + g[i1] * t;
        v * 10f64.powf(gdb / 20.0) as f32
    }).collect();
    // 2. pause lengthening (internal gaps >= 80 ms)
    let mut gaps: Vec<(usize, usize)> = Vec::new();
    let first = fr.db.iter().position(|d| *d >= floor).unwrap_or(0);
    let last = fr.db.iter().rposition(|d| *d >= floor).unwrap_or(n - 1);
    let mut i = first;
    while i <= last {
        if fr.db[i] < floor { let a = i; while i <= last && fr.db[i] < floor { i += 1; } if i - a >= 8 { gaps.push((a, i)); } } else { i += 1; }
    }
    let dur = x.len() as f64 / sr as f64;
    let mut added = 0.0;
    let mut extra_per_gap = vec![0.0f64; gaps.len()];
    if render_pause.is_finite() && base_pause.is_finite() && base_pause > render_pause && !gaps.is_empty() {
        let r_goal = render_pause + p.strength * (base_pause - render_pause);
        let pz = render_pause * dur;
        let need = ((r_goal * dur - pz) / (1.0 - r_goal)).clamp(0.0, p.max_total_add_frac * dur);
        let gl: Vec<f64> = gaps.iter().map(|(a, b)| (b - a) as f64 / 100.0).collect();
        let tot: f64 = gl.iter().sum();
        for (j, l) in gl.iter().enumerate() { extra_per_gap[j] = (need * l / tot).min(p.max_gap_add_s).min(*l); added += extra_per_gap[j]; }
    }
    if added > 0.0 {
        let fade = sr as usize * 5 / 1000;
        let mut out = Vec::with_capacity(y.len() + (added * sr as f64) as usize + 16);
        let mut cur = 0usize;
        for (j, (a, b)) in gaps.iter().enumerate() {
            let add = (extra_per_gap[j] * sr as f64) as usize;
            if add < 2 * fade { continue; }
            let mid = ((a + b) / 2) * hop;
            out.extend_from_slice(&y[cur..mid]);
            // fill = copy of this gap's own (quiet) centre material, faded at both ends
            let src0 = (a * hop + (b - a) * hop / 2).saturating_sub(add / 2);
            let mut fill: Vec<f32> = (0..add).map(|q| y[(src0 + q).min(y.len() - 1)]).collect();
            for q in 0..fade { let gq = q as f32 / fade as f32; fill[q] *= gq; let l = fill.len(); fill[l - 1 - q] *= gq; }
            out.extend(fill);
            cur = mid;
        }
        out.extend_from_slice(&y[cur..]);
        y = out;
    }
    // 3. peak safety (<= -1 dBFS)
    let peak = y.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    let lim = 0.891f32;
    let scaled = if peak > lim { let s = lim / peak; y.iter_mut().for_each(|v| *v *= s); true } else { false };
    let meta = json!({"expand_factor": k, "lref_db": lref, "gaps": gaps.len(), "added_s": added, "peak_scaled": scaled,
        "render_dr_db": render_dr, "base_dr_db": base_dr, "render_pause_ratio": render_pause, "base_pause_ratio": base_pause});
    (y, meta)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tone(sr: u32, f: f64, secs: f64, amp: f32) -> Vec<f32> { (0..(sr as f64 * secs) as usize).map(|i| amp * (2.0 * std::f64::consts::PI * f * i as f64 / sr as f64).sin() as f32).collect() }
    #[test]
    fn f0_tracks_tone() {
        let x = tone(24000, 200.0, 0.5, 0.3);
        let fr = analyse(&x, 24000, true);
        let v: Vec<f64> = fr.f0.iter().flatten().cloned().collect();
        assert!(!v.is_empty());
        assert!((mean(&v) - 200.0).abs() < 5.0);
    }
    #[test]
    fn prosody_adds_pause_and_keeps_peak() {
        let sr = 24000;
        let mut x = tone(sr, 180.0, 0.6, 0.5);
        x.extend(vec![0.0; (sr as f64 * 0.2) as usize]);
        x.extend(tone(sr, 180.0, 0.6, 0.5));
        let (y, m) = prosody_layer(&x, sr, &ProsodyParams::default(), 20.0, 28.0, 0.1, 0.2);
        assert!(y.len() > x.len(), "{m}");
        assert!(y.iter().all(|v| v.abs() <= 0.8915));
    }
    #[test]
    fn prosody_noop_when_render_already_expressive() {
        let sr = 24000;
        let x = tone(sr, 180.0, 1.0, 0.5);
        let (y, m) = prosody_layer(&x, sr, &ProsodyParams::default(), 30.0, 28.0, 0.3, 0.2);
        assert_eq!(y.len(), x.len());
        assert_eq!(m["expand_factor"].as_f64().unwrap(), 1.0);
    }
}
