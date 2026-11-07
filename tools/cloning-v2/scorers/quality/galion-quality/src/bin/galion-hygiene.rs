//! galion-hygiene: pure-Rust signal hygiene per WAV.
//!  - sample peak, true peak (ITU-R BS.1770-4 Annex 2: 4x oversampling, 48-tap 4-phase FIR), per channel max
//!  - clipped samples (|x| >= --clip-thr, default 0.99, matches own-clone-analysis), clipped runs
//!  - clicks: |x[n]-x[n-1]| > --click-thr (default 0.35, own-clone-analysis definition), max jump
//!  - bandwidth f90/f99: Welch PSD (Hann 2048, 50% overlap, mean-detrend) on the channel mean, energy CDF
//!  - EBU R128 / BS.1770-4: integrated LUFS (abs -70 / rel -10 gates), LRA, max momentary / short-term
use galion_quality::{audio::{self, db}, cli::{self, r, Args}};
use rustfft::{num_complex::Complex, FftPlanner};
use serde_json::json;
use std::path::Path;
use std::time::Instant;

const USAGE: &str = "usage: galion-hygiene [--clip-thr 0.99] [--click-thr 0.35] [--nfft 2048] [--out out.json] file.wav...";

// BS.1770-4 Annex 2 polyphase interpolation filter (4 phases x 12 taps).
const TP: [[f64; 12]; 4] = [
    [0.0017089843750, 0.0109863281250, -0.0196533203125, 0.0332031250000, -0.0594482421875, 0.1373291015625,
     0.9721679687500, -0.1022949218750, 0.0476074218750, -0.0266113281250, 0.0148925781250, -0.0083007812500],
    [-0.0291748046875, 0.0292968750000, -0.0517578125000, 0.0891113281250, -0.1665039062500, 0.4650878906250,
     0.7797851562500, -0.2003173828125, 0.1015625000000, -0.0582275390625, 0.0330810546875, -0.0189208984375],
    [-0.0189208984375, 0.0330810546875, -0.0582275390625, 0.1015625000000, -0.2003173828125, 0.7797851562500,
     0.4650878906250, -0.1665039062500, 0.0891113281250, -0.0517578125000, 0.0292968750000, -0.0291748046875],
    [-0.0083007812500, 0.0148925781250, -0.0266113281250, 0.0476074218750, -0.1022949218750, 0.9721679687500,
     0.1373291015625, -0.0594482421875, 0.0332031250000, -0.0196533203125, 0.0109863281250, 0.0017089843750],
];

fn true_peak(x: &[f32]) -> f64 {
    let mut pk = 0f64;
    let n = x.len() as isize;
    for i in 0..n + 12 {
        for ph in &TP {
            let mut acc = 0f64;
            for (k, c) in ph.iter().enumerate() {
                let j = i - k as isize;
                if j >= 0 && j < n { acc += c * x[j as usize] as f64; }
            }
            pk = pk.max(acc.abs());
        }
    }
    pk.max(x.iter().fold(0f32, |m, v| m.max(v.abs())) as f64)
}

struct Biquad { b: [f64; 3], a: [f64; 3] }
fn kweight(fs: f64) -> [Biquad; 2] {
    // libebur128 analog-prototype re-derivation, valid for any fs (matches BS.1770 table at 48 kHz).
    let (f0, g, q) = (1681.974450955533, 3.999843853973347, 0.7071752369554196);
    let k = (std::f64::consts::PI * f0 / fs).tan();
    let vh = 10f64.powf(g / 20.0); let vb = vh.powf(0.4996667741545416);
    let a0 = 1.0 + k / q + k * k;
    let shelf = Biquad { b: [(vh + vb * k / q + k * k) / a0, 2.0 * (k * k - vh) / a0, (vh - vb * k / q + k * k) / a0],
        a: [1.0, 2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0] };
    let (f0, q) = (38.13547087602444, 0.5003270373238773);
    let k = (std::f64::consts::PI * f0 / fs).tan();
    let d = 1.0 + k / q + k * k;
    let hp = Biquad { b: [1.0, -2.0, 1.0], a: [1.0, 2.0 * (k * k - 1.0) / d, (1.0 - k / q + k * k) / d] };
    [shelf, hp]
}
fn filt(bq: &Biquad, x: &[f64]) -> Vec<f64> {
    let (mut z1, mut z2) = (0f64, 0f64);
    x.iter().map(|&v| { let y = bq.b[0] * v + z1; z1 = bq.b[1] * v - bq.a[1] * y + z2; z2 = bq.b[2] * v - bq.a[2] * y; y }).collect()
}

/// Mean-square power of K-weighted sum over channels for blocks of `blk` samples hopped by `hop`.
fn block_powers(kw: &[Vec<f64>], blk: usize, hop: usize) -> Vec<f64> {
    let n = kw[0].len();
    if n < blk { return vec![]; }
    // prefix sums of summed channel squares (channel weights 1.0 for mono/L/R)
    let mut ps = vec![0f64; n + 1];
    for i in 0..n { ps[i + 1] = ps[i] + kw.iter().map(|c| c[i] * c[i]).sum::<f64>(); }
    (0..=(n - blk) / hop).map(|b| (ps[b * hop + blk] - ps[b * hop]) / blk as f64).collect()
}
fn lufs(z: f64) -> f64 { -0.691 + 10.0 * z.log10() }

fn loudness(chans: &[Vec<f32>], fs: u32) -> serde_json::Value {
    let f = kweight(fs as f64);
    let kw: Vec<Vec<f64>> = chans.iter().map(|c| { let x: Vec<f64> = c.iter().map(|&v| v as f64).collect(); filt(&f[1], &filt(&f[0], &x)) }).collect();
    let fs = fs as usize;
    let m = block_powers(&kw, fs * 4 / 10, fs / 10);
    let gated: Vec<f64> = m.iter().cloned().filter(|&z| lufs(z) > -70.0).collect();
    let integ = if gated.is_empty() { f64::NEG_INFINITY } else {
        let rel = lufs(gated.iter().sum::<f64>() / gated.len() as f64) - 10.0;
        let g2: Vec<f64> = gated.iter().cloned().filter(|&z| lufs(z) > rel).collect();
        if g2.is_empty() { f64::NEG_INFINITY } else { lufs(g2.iter().sum::<f64>() / g2.len() as f64) } };
    let st = block_powers(&kw, fs * 3, fs / 10);
    let sg: Vec<f64> = st.iter().cloned().filter(|&z| lufs(z) > -70.0).collect();
    let lra = if sg.is_empty() { f64::NAN } else {
        let rel = lufs(sg.iter().sum::<f64>() / sg.len() as f64) - 20.0;
        let mut l: Vec<f64> = sg.iter().map(|&z| lufs(z)).filter(|&v| v > rel).collect();
        l.sort_by(|a, b| a.partial_cmp(b).unwrap());
        if l.is_empty() { f64::NAN } else {
            let p = |q: f64| l[((l.len() - 1) as f64 * q).round() as usize]; p(0.95) - p(0.10) } };
    let mx = |v: &[f64]| v.iter().cloned().map(lufs).fold(f64::NEG_INFINITY, f64::max);
    json!({"integrated_lufs": r(integ, 2), "lra_lu": r(lra, 2), "max_momentary_lufs": r(mx(&m), 2), "max_short_term_lufs": r(mx(&st), 2),
           "note": if st.is_empty() { "clip < 3 s: LRA/short-term undefined" } else { "" }})
}

fn bandwidth(x: &[f32], fs: u32, nfft: usize) -> serde_json::Value {
    let nfft = nfft.min(x.len().next_power_of_two()).max(16);
    let hop = nfft / 2;
    let win: Vec<f64> = (0..nfft).map(|i| 0.5 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / nfft as f64).cos()).collect();
    let fft = FftPlanner::<f64>::new().plan_fft_forward(nfft);
    let mut psd = vec![0f64; nfft / 2 + 1];
    let mut buf = vec![Complex::new(0f64, 0f64); nfft];
    let mut segs = 0;
    let mut s = 0;
    while s + nfft <= x.len() {
        let seg = &x[s..s + nfft];
        let mu = seg.iter().map(|&v| v as f64).sum::<f64>() / nfft as f64;
        for i in 0..nfft { buf[i] = Complex::new((seg[i] as f64 - mu) * win[i], 0.0); }
        fft.process(&mut buf);
        for (k, p) in psd.iter_mut().enumerate() { *p += buf[k].norm_sqr(); }
        segs += 1; s += hop;
    }
    if segs == 0 { return json!({"f90_hz": null, "f99_hz": null}); }
    let tot: f64 = psd.iter().sum();
    let df = fs as f64 / nfft as f64;
    let fq = |q: f64| { let mut c = 0.0; for (k, p) in psd.iter().enumerate() { c += p; if c >= q * tot { return k as f64 * df; } } fs as f64 / 2.0 };
    let above = |hz: f64| psd.iter().enumerate().filter(|(k, _)| *k as f64 * df > hz).map(|(_, p)| p).sum::<f64>() / tot.max(1e-300);
    json!({"f90_hz": r(fq(0.90), 1), "f99_hz": r(fq(0.99), 1), "energy_frac_gt_4k": r(above(4000.0), 5), "energy_frac_gt_8k": r(above(8000.0), 6),
           "method": format!("welch hann nfft={nfft} hop={hop} mean-detrend, channel mean")})
}

fn main() {
    let a = Args::parse(&["help"]);
    if a.has("help") || a.pos.is_empty() { cli::die(USAGE) }
    let clip_thr: f32 = a.get("clip-thr").and_then(|s| s.parse().ok()).unwrap_or(0.99);
    let click_thr: f32 = a.get("click-thr").and_then(|s| s.parse().ok()).unwrap_or(0.35);
    let nfft: usize = a.get("nfft").and_then(|s| s.parse().ok()).unwrap_or(2048);
    let t0 = Instant::now();
    let mut rows = vec![];
    for f in &a.pos {
        let tf = Instant::now();
        let w = match audio::load(Path::new(f)) { Ok(w) => w, Err(e) => { rows.push(json!({"path": f, "error": e})); continue } };
        let mut sp = 0f32; let mut tp = 0f64; let (mut clipped, mut runs, mut clicks) = (0usize, 0usize, 0usize); let mut maxj = 0f32;
        let mut ss = 0f64;
        for c in &w.chans {
            let mut prev_clip = false;
            for (i, &v) in c.iter().enumerate() {
                sp = sp.max(v.abs()); ss += (v as f64) * (v as f64);
                let cl = v.abs() >= clip_thr; if cl { clipped += 1; if !prev_clip { runs += 1 } } prev_clip = cl;
                if i > 0 { let j = (v - c[i - 1]).abs(); maxj = maxj.max(j); if j > click_thr { clicks += 1 } }
            }
            tp = tp.max(true_peak(c));
        }
        let rms = (ss / (w.len() * w.chans.len()).max(1) as f64).sqrt();
        rows.push(json!({
            "path": f, "sr": w.sr, "channels": w.chans.len(), "bits": w.bits, "float": w.float, "duration_s": r(w.duration_s(), 3),
            "sample_peak_dbfs": r(db(sp as f64), 2), "true_peak_dbtp": r(db(tp), 2), "rms_dbfs": r(db(rms), 2),
            "clipped_samples": clipped, "clipped_runs": runs, "clip_thr": clip_thr,
            "clicks": clicks, "click_thr": click_thr, "max_sample_jump": r(maxj as f64, 4),
            "bandwidth": bandwidth(&w.mono(), w.sr, nfft),
            "loudness": loudness(&w.chans, w.sr),
            "ms": r(tf.elapsed().as_secs_f64() * 1e3, 2)
        }));
    }
    let out = json!({"tool": "galion-hygiene", "files": rows, "timing": {"total_s": r(t0.elapsed().as_secs_f64(), 4)}});
    let s = serde_json::to_string_pretty(&out).unwrap();
    match a.get("out") { Some(p) => std::fs::write(p, s + "\n").unwrap_or_else(|e| cli::die(&e.to_string())), None => println!("{s}") }
}
