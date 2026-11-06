//! Framing, intensity, MFCC, spectral moments.
use realfft::RealFftPlanner;
use std::sync::Arc;

pub fn hann(n: usize) -> Vec<f64> { (0..n).map(|i| 0.5 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / n as f64).cos()).collect() }
pub fn hamming(n: usize) -> Vec<f64> { (0..n).map(|i| 0.54 - 0.46 * (2.0 * std::f64::consts::PI * i as f64 / (n - 1) as f64).cos()).collect() }

/// Power spectra of windowed frames centred at `centers` (sample idx). Returns |X|^2 for bins 0..=nfft/2.
pub struct Stft { pub nfft: usize, fft: Arc<dyn realfft::RealToComplex<f64>>, win: Vec<f64> }
impl Stft {
    pub fn new(win: Vec<f64>, nfft: usize) -> Self {
        let mut p = RealFftPlanner::<f64>::new();
        Stft { nfft, fft: p.plan_fft_forward(nfft), win }
    }
    pub fn power(&self, x: &[f32], center: isize, out: &mut Vec<f64>, pre: f64) {
        let wl = self.win.len();
        let mut a = self.fft.make_input_vec();
        let st = center - (wl / 2) as isize;
        let get = |i: isize| -> f64 { if i >= 0 && (i as usize) < x.len() { x[i as usize] as f64 } else { 0.0 } };
        for k in 0..wl { let i = st + k as isize; let v = if pre > 0.0 { get(i) - pre * get(i - 1) } else { get(i) }; a[k] = v * self.win[k]; }
        let mut s = self.fft.make_output_vec();
        self.fft.process(&mut a, &mut s).unwrap();
        out.clear();
        out.extend(s.iter().map(|c| c.norm_sqr()));
    }
}

/// Frame intensity in dB (mean-square of Hann-windowed 32 ms frame, re full scale).
pub fn intensity_db(x: &[f32], fs: u32, centers: &[usize]) -> Vec<f64> {
    let wl = (0.032 * fs as f64) as usize;
    let w = hann(wl);
    let ws: f64 = w.iter().map(|v| v * v).sum();
    centers.iter().map(|&c| {
        let st = c as isize - (wl / 2) as isize;
        let mut e = 0.0;
        for k in 0..wl { let i = st + k as isize; if i >= 0 && (i as usize) < x.len() { let v = x[i as usize] as f64 * w[k]; e += v * v; } }
        10.0 * (e / ws + 1e-12).log10()
    }).collect()
}

fn hz2mel(f: f64) -> f64 { 2595.0 * (1.0 + f / 700.0).log10() }
fn mel2hz(m: f64) -> f64 { 700.0 * (10f64.powf(m / 2595.0) - 1.0) }

pub const NCEP: usize = 25;
/// Mel-cepstrum c0..c24: 25 ms Hamming, 512 FFT, 40 HTK mel bands 0..fs/2, ln energies, DCT-II with cepstral (1/N) scaling.
pub fn mfcc(x: &[f32], fs: u32, centers: &[usize]) -> Vec<[f64; NCEP]> {
    let wl = (0.025 * fs as f64) as usize;
    let mut nfft = 1; while nfft < wl { nfft <<= 1; }
    let st = Stft::new(hamming(wl), nfft);
    let nb = 40usize;
    let nbin = nfft / 2 + 1;
    let (ml, mh) = (hz2mel(0.0), hz2mel(fs as f64 / 2.0));
    let edges: Vec<f64> = (0..nb + 2).map(|i| mel2hz(ml + (mh - ml) * i as f64 / (nb + 1) as f64)).collect();
    let binf: Vec<f64> = (0..nbin).map(|k| k as f64 * fs as f64 / nfft as f64).collect();
    let fb: Vec<Vec<f64>> = (0..nb).map(|m| binf.iter().map(|&f| {
        let (l, c, h) = (edges[m], edges[m + 1], edges[m + 2]);
        if f > l && f <= c { (f - l) / (c - l) } else if f > c && f < h { (h - f) / (h - c) } else { 0.0 }
    }).collect()).collect();
    let dct: Vec<Vec<f64>> = (0..NCEP).map(|k| (0..nb).map(|n| {
        // cepstral scaling: c_k = (2/N) * sum_m ln|X_m| cos(..), with ln|X| = 0.5 ln P  => (1/N) sum ln P cos(..)
        // (matches real-cepstrum / SPTK mel-cepstrum magnitude so MCD lands on the usual dB scale)
        let s = if k == 0 { 0.5 / nb as f64 } else { 1.0 / nb as f64 };
        s * (std::f64::consts::PI * k as f64 * (n as f64 + 0.5) / nb as f64).cos()
    }).collect()).collect();
    let mut p = Vec::new();
    let mut lm = vec![0f64; nb];
    centers.iter().map(|&c| {
        st.power(x, c as isize, &mut p, 0.97);
        for m in 0..nb { let e: f64 = fb[m].iter().zip(p.iter()).map(|(a, b)| a * b).sum(); lm[m] = e.max(1e-10).ln(); }
        let mut o = [0f64; NCEP];
        for k in 0..NCEP { o[k] = dct[k].iter().zip(lm.iter()).map(|(a, b)| a * b).sum(); }
        o
    }).collect()
}

/// Spectral moments (centroid, sd, skewness, excess kurtosis) of power spectrum within [lo,hi] Hz.
pub fn moments(p: &[f64], fs: u32, nfft: usize, lo: f64, hi: f64) -> [f64; 4] {
    let df = fs as f64 / nfft as f64;
    let (a, b) = ((lo / df).ceil() as usize, ((hi / df).floor() as usize).min(p.len() - 1));
    let tot: f64 = p[a..=b].iter().sum();
    if tot <= 0.0 { return [f64::NAN; 4]; }
    let mut m1 = 0.0; for k in a..=b { m1 += k as f64 * df * p[k]; } m1 /= tot;
    let (mut m2, mut m3, mut m4) = (0.0, 0.0, 0.0);
    for k in a..=b { let d = k as f64 * df - m1; let w = p[k] / tot; m2 += d * d * w; m3 += d * d * d * w; m4 += d * d * d * d * w; }
    let sd = m2.sqrt();
    [m1, sd, m3 / sd.powi(3), m4 / (m2 * m2) - 3.0]
}

pub fn band_energy(p: &[f64], fs: u32, nfft: usize, lo: f64, hi: f64) -> f64 {
    let df = fs as f64 / nfft as f64;
    let (a, b) = ((lo / df).ceil() as usize, ((hi / df).floor() as usize).min(p.len() - 1));
    if a > b { return 0.0; }
    p[a..=b].iter().sum()
}

/// Zero-phase 4th-order Butterworth low-pass (two cascaded RBJ biquads, forward-backward).
pub fn lowpass_zp(x: &[f32], fs: u32, fc: f64) -> Vec<f32> {
    let qs = [0.541196100146197, 1.306562964876377];
    let mut y: Vec<f64> = x.iter().map(|&v| v as f64).collect();
    for pass in 0..2 {
        for &q in &qs {
            let w0 = 2.0 * std::f64::consts::PI * fc / fs as f64;
            let (c, s) = (w0.cos(), w0.sin());
            let al = s / (2.0 * q);
            let a0 = 1.0 + al;
            let (b0, b1, b2) = ((1.0 - c) / 2.0 / a0, (1.0 - c) / a0, (1.0 - c) / 2.0 / a0);
            let (a1, a2) = (-2.0 * c / a0, (1.0 - al) / a0);
            let (mut x1, mut x2, mut y1, mut y2) = (0.0, 0.0, 0.0, 0.0);
            for v in y.iter_mut() {
                let xn = *v;
                let yn = b0 * xn + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;
                x2 = x1; x1 = xn; y2 = y1; y1 = yn; *v = yn;
            }
        }
        y.reverse();
        let _ = pass;
    }
    y.into_iter().map(|v| v as f32).collect()
}
