// Galion Voice — Perpetual Open MIT License
// G0 env probe: std-only, no deps. Measures memory bandwidth 3x (CV gate) and
// runs scalar reference op self-checks (G1-ref). Output: one JSON line.
use std::time::Instant;

fn bw_gbs(buf: &mut [f32]) -> f64 {
    let n = buf.len();
    let t = Instant::now();
    let mut acc = 0.0f32;
    for _ in 0..4 {
        for i in 0..n { buf[i] = buf[i] * 1.000001 + 1.0; }
        acc += buf[n / 2];
    }
    let s = t.elapsed().as_secs_f64();
    std::hint::black_box(acc);
    (4.0 * 2.0 * (n * 4) as f64) / s / 1e9 // read+write bytes
}

fn matmul(a: &[f32], b: &[f32], m: usize, k: usize, n: usize) -> Vec<f32> {
    let mut c = vec![0.0; m * n];
    for i in 0..m { for p in 0..k { let av = a[i * k + p];
        for j in 0..n { c[i * n + j] += av * b[p * n + j]; } } }
    c
}
fn softmax(x: &[f32]) -> Vec<f32> {
    let m = x.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let e: Vec<f32> = x.iter().map(|v| (v - m).exp()).collect();
    let s: f32 = e.iter().sum();
    e.iter().map(|v| v / s).collect()
}
fn rmsnorm(x: &[f32], eps: f32) -> Vec<f32> {
    let ms = x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32;
    let r = 1.0 / (ms + eps).sqrt();
    x.iter().map(|v| v * r).collect()
}
fn silu(x: f32) -> f32 { x / (1.0 + (-x).exp()) }

fn ref_checks() -> Vec<(&'static str, bool, f64)> {
    let mut out = vec![];
    // matmul vs hand-computed
    let a = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0]; // 2x3
    let b = [7.0, 8.0, 9.0, 10.0, 11.0, 12.0]; // 3x2
    let c = matmul(&a, &b, 2, 3, 2);
    let exp = [58.0, 64.0, 139.0, 154.0];
    let e = c.iter().zip(exp).map(|(x, y)| (x - y).abs() as f64).fold(0.0, f64::max);
    out.push(("matmul_2x3x2", e <= 1e-5, e));
    // matmul identity on odd shape 7x13
    let (m, k) = (7, 13);
    let a: Vec<f32> = (0..m * k).map(|i| ((i * 37 % 101) as f32 - 50.0) / 17.0).collect();
    let mut id = vec![0.0; k * k]; for i in 0..k { id[i * k + i] = 1.0; }
    let c = matmul(&a, &id, m, k, k);
    let e = c.iter().zip(&a).map(|(x, y)| (x - y).abs() as f64).fold(0.0, f64::max);
    out.push(("matmul_identity_7x13", e <= 1e-6, e));
    // softmax sums to 1, shift-invariant, numerically stable at large logits
    let x = [1000.0, 1001.0, 1002.0];
    let s = softmax(&x);
    let s2 = softmax(&[0.0, 1.0, 2.0]);
    let e = (s.iter().sum::<f32>() - 1.0).abs().max(
        s.iter().zip(&s2).map(|(a, b)| (a - b).abs()).fold(0.0, f32::max)) as f64;
    out.push(("softmax_stable", e <= 1e-6 && s.iter().all(|v| v.is_finite()), e));
    // rmsnorm: output RMS == 1
    let r = rmsnorm(&[3.0, -4.0, 12.0, 0.5], 0.0);
    let rms = (r.iter().map(|v| v * v).sum::<f32>() / 4.0).sqrt();
    let e = (rms - 1.0).abs() as f64;
    out.push(("rmsnorm_unit_rms", e <= 1e-6, e));
    // silu known values
    let e = ((silu(0.0)).abs() + (silu(1.0) - 0.7310586).abs()) as f64;
    out.push(("silu_values", e <= 1e-6, e));
    out
}

fn main() {
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
    let mut buf = vec![1.0f32; 64 * 1024 * 1024 / 4 * 4]; // 256 MiB
    bw_gbs(&mut buf); // warm-up
    let runs: Vec<f64> = (0..3).map(|_| bw_gbs(&mut buf)).collect();
    let mean = runs.iter().sum::<f64>() / 3.0;
    let sd = (runs.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / 3.0).sqrt();
    let cv = sd / mean * 100.0;
    let checks = ref_checks();
    let all = checks.iter().all(|c| c.1);
    let cj: Vec<String> = checks.iter()
        .map(|(n, ok, e)| format!("{{\"op\":\"{n}\",\"pass\":{ok},\"max_abs_err\":{e:e}}}")).collect();
    println!("{{\"threads\":{threads},\"bw_runs_gbs\":[{:.2},{:.2},{:.2}],\"bw_mean_gbs\":{mean:.2},\"bw_cv_pct\":{cv:.2},\"ref_ops_pass\":{all},\"ref_ops\":[{}]}}",
        runs[0], runs[1], runs[2], cj.join(","));
}
