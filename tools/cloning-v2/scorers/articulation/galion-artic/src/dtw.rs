//! DTW on mel-cepstra and mel-cepstral distortion.
use crate::dsp::NCEP;

pub const MCD_K: f64 = 10.0 / std::f64::consts::LN_10 * std::f64::consts::SQRT_2;

/// Frame MCD (dB) over c1..c24 (c0/energy excluded).
pub fn mcd_frame(a: &[f64; NCEP], b: &[f64; NCEP]) -> f64 {
    let mut s = 0.0; for k in 1..NCEP { let d = a[k] - b[k]; s += d * d; }
    MCD_K * s.sqrt()
}

/// Symmetric DTW (steps (1,0),(0,1),(1,1)), local cost = frame MCD. Returns path (i,j) and mean cost on path.
pub fn dtw(a: &[[f64; NCEP]], b: &[[f64; NCEP]]) -> (Vec<(usize, usize)>, f64) {
    let (n, m) = (a.len(), b.len());
    if n == 0 || m == 0 { return (vec![], f64::NAN); }
    let mut d = vec![f32::INFINITY; n * m];
    let mut cost = vec![0f32; n * m];
    for i in 0..n { for j in 0..m { cost[i * m + j] = mcd_frame(&a[i], &b[j]) as f32; } }
    for i in 0..n {
        for j in 0..m {
            let c = cost[i * m + j];
            let best = if i == 0 && j == 0 { 0.0 } else {
                let mut v = f32::INFINITY;
                if i > 0 { v = v.min(d[(i - 1) * m + j]); }
                if j > 0 { v = v.min(d[i * m + j - 1]); }
                if i > 0 && j > 0 { v = v.min(d[(i - 1) * m + j - 1]); }
                v
            };
            d[i * m + j] = best + c;
        }
    }
    let (mut i, mut j) = (n - 1, m - 1);
    let mut path = vec![(i, j)];
    while i > 0 || j > 0 {
        if i == 0 { j -= 1; } else if j == 0 { i -= 1; } else {
            let (dg, up, lf) = (d[(i - 1) * m + j - 1], d[(i - 1) * m + j], d[i * m + j - 1]);
            if dg <= up && dg <= lf { i -= 1; j -= 1; } else if up <= lf { i -= 1; } else { j -= 1; }
        }
        path.push((i, j));
    }
    path.reverse();
    let mean = path.iter().map(|&(i, j)| cost[i * m + j] as f64).sum::<f64>() / path.len() as f64;
    (path, mean)
}
