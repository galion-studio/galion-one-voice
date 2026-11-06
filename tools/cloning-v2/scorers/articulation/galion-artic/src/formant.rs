//! Praat-compatible Burg LPC formant tracker (To Formant (burg) equivalent):
//! resample to 2*ceiling, pre-emphasis from 50 Hz, Gaussian window of 2x effective length,
//! Burg LPC of order 2*nformants, polynomial roots (Aberth-Ehrlich), keep 50 Hz < F < Nyquist-50.
use rustfft::num_complex::Complex64 as C;

/// Numerical Recipes `memcof` (Burg). Returns coefficients d with x[n] ~ sum_j d[j] x[n-1-j].
pub fn burg(x: &[f64], m: usize) -> Option<Vec<f64>> {
    let n = x.len();
    if n <= m + 1 { return None; }
    let mut wk1 = x[..n - 1].to_vec();
    let mut wk2 = x[1..].to_vec();
    let mut d = vec![0f64; m];
    let mut wkm = vec![0f64; m];
    for k in 1..=m {
        let mut num = 0.0; let mut den = 0.0;
        for j in 0..(n - k) { num += wk1[j] * wk2[j]; den += wk1[j] * wk1[j] + wk2[j] * wk2[j]; }
        if den <= 0.0 { return None; }
        d[k - 1] = 2.0 * num / den;
        for i in 0..(k - 1) { d[i] = wkm[i] - d[k - 1] * wkm[k - 2 - i]; }
        if k == m { return Some(d); }
        wkm[..k].copy_from_slice(&d[..k]);
        for j in 0..(n - k - 1) {
            wk1[j] -= wkm[k - 1] * wk2[j];
            wk2[j] = wk2[j + 1] - wkm[k - 1] * wk1[j + 1];
        }
    }
    Some(d)
}

/// Roots of monic polynomial z^m + c[0] z^{m-1} + ... + c[m-1] (Aberth-Ehrlich).
pub fn roots(c: &[f64]) -> Vec<C> {
    let m = c.len();
    let eval = |z: C| -> (C, C) {
        let mut p = C::new(1.0, 0.0); let mut dp = C::new(0.0, 0.0);
        for &ci in c { dp = dp * z + p; p = p * z + C::new(ci, 0.0); }
        (p, dp)
    };
    let mut z: Vec<C> = (0..m).map(|k| C::from_polar(0.9, 2.0 * std::f64::consts::PI * (k as f64 + 0.25) / m as f64 + 0.4)).collect();
    for _ in 0..200 {
        let mut maxstep: f64 = 0.0;
        for i in 0..m {
            let (p, dp) = eval(z[i]);
            if p.norm() == 0.0 { continue; }
            let ratio = p / dp;
            let mut s = C::new(0.0, 0.0);
            for j in 0..m { if j != i { let df = z[i] - z[j]; if df.norm() > 1e-14 { s += C::new(1.0, 0.0) / df; } } }
            let w = ratio / (C::new(1.0, 0.0) - ratio * s);
            if w.is_finite() { z[i] -= w; maxstep = maxstep.max(w.norm()); }
        }
        if maxstep < 1e-12 { break; }
    }
    z
}

pub struct FormantFrame { pub f: [f64; 3], pub bw: [f64; 3] }

pub struct FormantTrack { pub frames: Vec<FormantFrame>, pub ceiling: f64 }

/// `xr` must be sampled at fs = 2*ceiling. `centers_t` = frame centre times (s).
pub fn track(xr: &[f32], fs: u32, centers_t: &[f64], nformants: usize, ceiling: f64) -> FormantTrack {
    let fsf = fs as f64;
    let dt = 1.0 / fsf;
    // pre-emphasis from 50 Hz
    let a = (-2.0 * std::f64::consts::PI * 50.0 * dt).exp();
    let mut y = vec![0f64; xr.len()];
    for i in (1..xr.len()).rev() { y[i] = xr[i] as f64 - a * xr[i - 1] as f64; }
    if !xr.is_empty() { y[0] = xr[0] as f64; }
    let nw = (2.0 * 0.025 * fsf).floor() as usize; // physical window = 2 x effective 25 ms
    let edge = (-12.0f64).exp();
    let imid = 0.5 * (nw as f64 + 1.0);
    let win: Vec<f64> = (1..=nw).map(|i| { let q = i as f64 - imid; ((-48.0 * q * q / ((nw as f64 + 1.0).powi(2))).exp() - edge) / (1.0 - edge) }).collect();
    let order = 2 * nformants;
    let mut frames = Vec::with_capacity(centers_t.len());
    let mut buf = vec![0f64; nw];
    for &t in centers_t {
        let c = (t * fsf).round() as isize;
        let st = c - (nw / 2) as isize;
        let mut energy = 0.0;
        for k in 0..nw {
            let idx = st + k as isize;
            let v = if idx >= 0 && (idx as usize) < y.len() { y[idx as usize] } else { 0.0 };
            buf[k] = v * win[k]; energy += buf[k] * buf[k];
        }
        let mut ff = FormantFrame { f: [f64::NAN; 3], bw: [f64::NAN; 3] };
        if energy > 1e-12 {
            if let Some(d) = burg(&buf, order) {
                let cpoly: Vec<f64> = d.iter().map(|v| -v).collect();
                let rts = roots(&cpoly);
                let mut fm: Vec<(f64, f64)> = rts.iter().filter(|r| r.im >= 0.0).map(|r| {
                    let mut r = *r; if r.norm() > 1.0 { r = C::new(1.0, 0.0) / r.conj(); } // fix into unit circle
                    let f = r.arg().abs() * fsf / (2.0 * std::f64::consts::PI);
                    let bw = -r.norm().ln() * fsf / std::f64::consts::PI;
                    (f, bw)
                }).filter(|&(f, _)| f > 50.0 && f < ceiling - 50.0).collect();
                fm.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
                for k in 0..3.min(fm.len()) { ff.f[k] = fm[k].0; ff.bw[k] = fm[k].1; }
            }
        }
        frames.push(ff);
    }
    FormantTrack { frames, ceiling }
}
