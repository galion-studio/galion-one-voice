//! Small statistics helpers.
pub fn median(v: &[f64]) -> f64 {
    quantile(v, 0.5)
}
pub fn quantile(v: &[f64], q: f64) -> f64 {
    let mut s: Vec<f64> = v.iter().cloned().filter(|x| x.is_finite()).collect();
    if s.is_empty() {
        return f64::NAN;
    }
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p = q * (s.len() - 1) as f64;
    let i = p.floor() as usize;
    let f = p - i as f64;
    if i + 1 < s.len() { s[i] * (1.0 - f) + s[i + 1] * f } else { s[i] }
}
pub fn mean(v: &[f64]) -> f64 {
    let s: Vec<f64> = v.iter().cloned().filter(|x| x.is_finite()).collect();
    if s.is_empty() { return f64::NAN; }
    s.iter().sum::<f64>() / s.len() as f64
}
pub fn sd(v: &[f64]) -> f64 {
    let s: Vec<f64> = v.iter().cloned().filter(|x| x.is_finite()).collect();
    if s.len() < 2 { return f64::NAN; }
    let m = s.iter().sum::<f64>() / s.len() as f64;
    (s.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (s.len() - 1) as f64).sqrt()
}
/// Convex hull area (monotone chain) of 2-D points.
pub fn hull_area(pts: &[(f64, f64)]) -> f64 {
    let mut p: Vec<(f64, f64)> = pts.iter().cloned().filter(|a| a.0.is_finite() && a.1.is_finite()).collect();
    if p.len() < 3 { return 0.0; }
    p.sort_by(|a, b| a.partial_cmp(b).unwrap());
    p.dedup();
    if p.len() < 3 { return 0.0; }
    let cross = |o: (f64, f64), a: (f64, f64), b: (f64, f64)| (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0);
    let mut h: Vec<(f64, f64)> = Vec::new();
    for &q in &p {
        while h.len() >= 2 && cross(h[h.len() - 2], h[h.len() - 1], q) <= 0.0 { h.pop(); }
        h.push(q);
    }
    let lo = h.len() + 1;
    for &q in p.iter().rev().skip(1) {
        while h.len() >= lo && cross(h[h.len() - 2], h[h.len() - 1], q) <= 0.0 { h.pop(); }
        h.push(q);
    }
    h.pop();
    let mut a = 0.0;
    for i in 0..h.len() {
        let j = (i + 1) % h.len();
        a += h[i].0 * h[j].1 - h[j].0 * h[i].1;
    }
    a.abs() * 0.5
}
pub fn hz_to_bark(f: f64) -> f64 { 26.81 * f / (1960.0 + f) - 0.53 }
