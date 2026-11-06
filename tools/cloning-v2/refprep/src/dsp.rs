//! DSP helpers. Loudness/true-peak/bandwidth ported from galion-hygiene so refprep and the
//! scorer measure identically; declick/trim/crossfade adapted from simsel-rs.
use rustfft::{num_complex::Complex, FftPlanner};

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

pub fn true_peak(x: &[f32]) -> f64 {
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
pub fn db(x: f64) -> f64 { 20.0 * x.max(1e-12).log10() }

#[derive(Clone, Copy)]
pub struct Biquad { pub b: [f64; 3], pub a: [f64; 3] }
pub fn filt(bq: &Biquad, x: &[f64]) -> Vec<f64> {
    let (mut z1, mut z2) = (0f64, 0f64);
    x.iter().map(|&v| { let y = bq.b[0] * v + z1; z1 = bq.b[1] * v - bq.a[1] * y + z2; z2 = bq.b[2] * v - bq.a[2] * y; y }).collect()
}
fn kweight(fs: f64) -> [Biquad; 2] {
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
fn lufs(z: f64) -> f64 { -0.691 + 10.0 * z.log10() }
pub fn integrated_lufs(x: &[f32], fs: u32) -> f64 {
    let f = kweight(fs as f64);
    let xd: Vec<f64> = x.iter().map(|&v| v as f64).collect();
    let kw = filt(&f[1], &filt(&f[0], &xd));
    let (blk, hop) = (fs as usize * 4 / 10, fs as usize / 10);
    if kw.len() < blk { return f64::NEG_INFINITY; }
    let mut ps = vec![0f64; kw.len() + 1];
    for i in 0..kw.len() { ps[i + 1] = ps[i] + kw[i] * kw[i]; }
    let m: Vec<f64> = (0..=(kw.len() - blk) / hop).map(|b| (ps[b * hop + blk] - ps[b * hop]) / blk as f64).collect();
    let g: Vec<f64> = m.into_iter().filter(|&z| lufs(z) > -70.0).collect();
    if g.is_empty() { return f64::NEG_INFINITY; }
    let rel = lufs(g.iter().sum::<f64>() / g.len() as f64) - 10.0;
    let g2: Vec<f64> = g.into_iter().filter(|&z| lufs(z) > rel).collect();
    if g2.is_empty() { f64::NEG_INFINITY } else { lufs(g2.iter().sum::<f64>() / g2.len() as f64) }
}

/// Welch PSD (Hann 2048, 50%) energy CDF: (f90, f99, frac>4k) — same method as galion-hygiene.
pub fn bandwidth(x: &[f32], fs: u32) -> (f64, f64, f64) {
    let nfft = 2048usize.min(x.len().next_power_of_two()).max(16);
    let hop = nfft / 2;
    let win: Vec<f64> = (0..nfft).map(|i| 0.5 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / nfft as f64).cos()).collect();
    let fft = FftPlanner::<f64>::new().plan_fft_forward(nfft);
    let mut psd = vec![0f64; nfft / 2 + 1];
    let mut buf = vec![Complex::new(0f64, 0f64); nfft];
    let mut s = 0;
    while s + nfft <= x.len() {
        let seg = &x[s..s + nfft];
        let mu = seg.iter().map(|&v| v as f64).sum::<f64>() / nfft as f64;
        for i in 0..nfft { buf[i] = Complex::new((seg[i] as f64 - mu) * win[i], 0.0); }
        fft.process(&mut buf);
        for (k, p) in psd.iter_mut().enumerate() { *p += buf[k].norm_sqr(); }
        s += hop;
    }
    let tot: f64 = psd.iter().sum::<f64>().max(1e-300);
    let df = fs as f64 / nfft as f64;
    let fq = |q: f64| { let mut c = 0.0; for (k, p) in psd.iter().enumerate() { c += p; if c >= q * tot { return k as f64 * df; } } fs as f64 / 2.0 };
    let above = psd.iter().enumerate().filter(|(k, _)| *k as f64 * df > 4000.0).map(|(_, p)| p).sum::<f64>() / tot;
    (fq(0.90), fq(0.99), above)
}

/// RBJ cookbook high shelf.
pub fn high_shelf(fs: f64, f0: f64, gain_db: f64, q: f64) -> Biquad {
    let a = 10f64.powf(gain_db / 40.0);
    let w0 = 2.0 * std::f64::consts::PI * f0 / fs;
    let (c, s) = (w0.cos(), w0.sin());
    let alpha = s / (2.0 * q);
    let sa = 2.0 * a.sqrt() * alpha;
    let b0 = a * ((a + 1.0) + (a - 1.0) * c + sa);
    let b1 = -2.0 * a * ((a - 1.0) + (a + 1.0) * c);
    let b2 = a * ((a + 1.0) + (a - 1.0) * c - sa);
    let a0 = (a + 1.0) - (a - 1.0) * c + sa;
    let a1 = 2.0 * ((a - 1.0) - (a + 1.0) * c);
    let a2 = (a + 1.0) - (a - 1.0) * c - sa;
    Biquad { b: [b0 / a0, b1 / a0, b2 / a0], a: [1.0, a1 / a0, a2 / a0] }
}
/// RBJ 2nd-order high-pass.
pub fn highpass(fs: f64, f0: f64, q: f64) -> Biquad {
    let w0 = 2.0 * std::f64::consts::PI * f0 / fs;
    let (c, s) = (w0.cos(), w0.sin());
    let alpha = s / (2.0 * q);
    let a0 = 1.0 + alpha;
    Biquad { b: [(1.0 + c) / 2.0 / a0, -(1.0 + c) / a0, (1.0 + c) / 2.0 / a0], a: [1.0, -2.0 * c / a0, (1.0 - alpha) / a0] }
}

/// RBJ 2nd-order low-pass.
pub fn lowpass(fs: f64, f0: f64, q: f64) -> Biquad {
    let w0 = 2.0 * std::f64::consts::PI * f0 / fs;
    let (c, s) = (w0.cos(), w0.sin());
    let alpha = s / (2.0 * q);
    let a0 = 1.0 + alpha;
    Biquad { b: [(1.0 - c) / 2.0 / a0, (1.0 - c) / a0, (1.0 - c) / 2.0 / a0], a: [1.0, -2.0 * c / a0, (1.0 - alpha) / a0] }
}

/// E-mild recipe (port of hf-samantha enhance_refs.py): RBJ high shelf, then harmonic exciter =
/// tanh(1.8 * bandpass 1.2-4.5 kHz) high-passed at 2.2 kHz, RMS-matched to `mix` x signal RMS, added.
pub fn hf_shelf_exciter(x: &[f32], fs: u32, f0: f64, gain_db: f64, q: f64, mix: f64) -> Vec<f32> {
    let fsd = fs as f64;
    let xd: Vec<f64> = x.iter().map(|&v| v as f64).collect();
    let y = filt(&high_shelf(fsd, f0, gain_db, q), &xd);
    if mix <= 0.0 { return y.iter().map(|&v| v as f32).collect(); }
    let band = filt(&lowpass(fsd, 4500f64.min(fsd / 2.0 - 100.0), 0.707), &filt(&highpass(fsd, 1200.0, 0.707), &y));
    let driven: Vec<f64> = band.iter().map(|v| (1.8 * v).tanh()).collect();
    let air = filt(&highpass(fsd, 2200.0, 0.707), &driven);
    let rms = |v: &[f64]| (v.iter().map(|a| a * a).sum::<f64>() / v.len().max(1) as f64).sqrt() + 1e-12;
    let g = rms(&y) / rms(&air) * mix;
    y.iter().zip(&air).map(|(a, b)| (a + g * b) as f32).collect()
}

pub fn frame_rms(y: &[f32], n: usize, h: usize) -> Vec<f32> {
    let mut v = Vec::new();
    let mut i = 0;
    while i + n <= y.len() { v.push((y[i..i + n].iter().map(|x| x * x).sum::<f32>() / n as f32).sqrt()); i += h; }
    v
}
pub fn quantile(v: &[f32], q: f32) -> f32 {
    if v.is_empty() { return 0.0; }
    let mut s = v.to_vec(); s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    s[(((s.len() - 1) as f32) * q).round() as usize]
}

pub fn declick(y: &[f32], thr: f32) -> (Vec<f32>, usize) {
    let mut out = y.to_vec(); let mut n = 0;
    for i in 1..out.len() {
        if (out[i] - out[i - 1]).abs() > thr {
            let lo = i.saturating_sub(2); let hi = (i + 3).min(out.len());
            let mut w = out[lo..hi].to_vec(); w.sort_by(|a, b| a.partial_cmp(b).unwrap());
            out[i] = w[w.len() / 2]; n += 1;
        }
    }
    (out, n)
}
pub fn count_clicks(y: &[f32], thr: f32) -> usize { y.windows(2).filter(|w| (w[1] - w[0]).abs() > thr).count() }

pub fn crossfade_concat(parts: &[Vec<f32>], xf: usize) -> Vec<f32> {
    let mut out: Vec<f32> = Vec::new();
    for p in parts {
        if out.len() < xf || p.len() < xf { out.extend_from_slice(p); continue; }
        let st = out.len() - xf;
        for i in 0..xf { let t = i as f32 / xf as f32; out[st + i] = out[st + i] * (1.0 - t) + p[i] * t; }
        out.extend_from_slice(&p[xf..]);
    }
    out
}

/// Stationary spectral gate (port of noisereduce `stationary=True` defaults used by hf-samantha E-mild):
/// STFT hann n_fft/hop; per-bin noise stats (dB mean + n_std*std over the whole clip); binary mask
/// smoothed over +-smooth_hz / +-smooth_ms; gain = mask*prop + (1-prop) ... i.e. noise bins x (1-prop).
pub fn spectral_gate(x: &[f32], fs: u32, n_fft: usize, hop: usize, n_std: f64, prop: f64, smooth_hz: f64, smooth_ms: f64) -> Vec<f32> {
    if x.len() < n_fft { return x.to_vec(); }
    let win: Vec<f64> = (0..n_fft).map(|i| 0.5 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / n_fft as f64).cos()).collect();
    let mut planner = FftPlanner::<f64>::new();
    let fwd = planner.plan_fft_forward(n_fft); let inv = planner.plan_fft_inverse(n_fft);
    let pad = n_fft / 2;
    let mut xp = vec![0f64; x.len() + 2 * pad + n_fft];
    for (i, v) in x.iter().enumerate() { xp[pad + i] = *v as f64; }
    let nfr = (xp.len() - n_fft) / hop + 1; let nb = n_fft / 2 + 1;
    let mut spec: Vec<Vec<Complex<f64>>> = Vec::with_capacity(nfr);
    for f in 0..nfr {
        let mut b: Vec<Complex<f64>> = (0..n_fft).map(|i| Complex::new(xp[f * hop + i] * win[i], 0.0)).collect();
        fwd.process(&mut b); spec.push(b);
    }
    let dbm: Vec<Vec<f64>> = spec.iter().map(|b| (0..nb).map(|k| 20.0 * (b[k].norm() + 1e-10).log10()).collect()).collect();
    let mut thr = vec![0f64; nb];
    for k in 0..nb {
        let m = dbm.iter().map(|r| r[k]).sum::<f64>() / nfr as f64;
        let sd = (dbm.iter().map(|r| (r[k] - m).powi(2)).sum::<f64>() / nfr as f64).sqrt();
        thr[k] = m + n_std * sd;
    }
    let mask: Vec<Vec<f64>> = dbm.iter().map(|r| (0..nb).map(|k| if r[k] > thr[k] { 1.0 } else { 0.0 }).collect()).collect();
    let rf = ((smooth_hz / (fs as f64 / n_fft as f64)).round() as isize).max(0);
    let rt = ((smooth_ms / 1000.0 * fs as f64 / hop as f64).round() as isize).max(0);
    let blur = |m: &Vec<Vec<f64>>| -> Vec<Vec<f64>> {
        let a: Vec<Vec<f64>> = m.iter().map(|r| (0..nb as isize).map(|k| { let (lo, hi) = ((k - rf).max(0), (k + rf).min(nb as isize - 1));
            (lo..=hi).map(|j| r[j as usize]).sum::<f64>() / (hi - lo + 1) as f64 }).collect()).collect();
        (0..nfr as isize).map(|f| { let (lo, hi) = ((f - rt).max(0), (f + rt).min(nfr as isize - 1));
            (0..nb).map(|k| (lo..=hi).map(|j| a[j as usize][k]).sum::<f64>() / (hi - lo + 1) as f64).collect() }).collect()
    };
    let sm = blur(&mask);
    let mut out = vec![0f64; xp.len()]; let mut wsum = vec![0f64; xp.len()];
    for f in 0..nfr {
        let b = &mut spec[f];
        for k in 0..nb { let g = sm[f][k] * prop + (1.0 - prop); b[k] *= g; if k > 0 && k < n_fft - k { b[n_fft - k] = b[k].conj(); } }
        inv.process(b);
        for i in 0..n_fft { out[f * hop + i] += b[i].re / n_fft as f64 * win[i]; wsum[f * hop + i] += win[i] * win[i]; }
    }
    (0..x.len()).map(|i| { let w = wsum[pad + i]; (if w > 1e-8 { out[pad + i] / w } else { 0.0 }) as f32 }).collect()
}
