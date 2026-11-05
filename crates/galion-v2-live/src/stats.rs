// Galion Voice — Perpetual Open MIT License
pub fn pct(v: &[f64], p: f64) -> f64 {
    if v.is_empty() { return f64::NAN; }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let r = p / 100.0 * (s.len() - 1) as f64;
    let (lo, hi) = (r.floor() as usize, r.ceil() as usize);
    s[lo] + (s[hi] - s[lo]) * (r - lo as f64)
}
pub fn summary(name: &str, v: &[f64]) -> serde_json::Value {
    let mean = v.iter().sum::<f64>() / v.len().max(1) as f64;
    serde_json::json!({"name": name, "n": v.len(), "p50": r1(pct(v,50.0)), "p95": r1(pct(v,95.0)),
        "min": r1(v.iter().cloned().fold(f64::INFINITY, f64::min)), "max": r1(v.iter().cloned().fold(f64::NEG_INFINITY, f64::max)), "mean": r1(mean)})
}
pub fn r1(x: f64) -> f64 { (x * 10.0).round() / 10.0 }
