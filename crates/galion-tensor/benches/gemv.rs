// Galion Voice — Perpetual Open MIT License
// SPDX-License-Identifier: MIT
//! A4 GEMV bench (no criterion; std::time only, `harness = false`).
//!
//! ```text
//! cargo bench -p galion-tensor --bench gemv                                   # scalar build
//! cargo bench -p galion-tensor --bench gemv --features simd-avx2,simd-avx512  # x86 SIMD
//! cargo bench -p galion-tensor --bench gemv --features simd-neon              # aarch64
//! GALION_BENCH_MS=50 cargo bench ...   # shorter per-case budget (default 200 ms)
//! ```
//! Every case is checked against the scalar oracle before timing (tolerance for f32 SIMD dot,
//! bit-exact for int8 and gemv_t), so the bench doubles as a smoke test on the target CPU.

use galion_tensor::gemv::{gemv_f32_into_with, gemv_t_f32_into};
use galion_tensor::gemv_i8::{gemv_i8_into_with, quantize_row_i8, QuantRowsI8};
use galion_tensor::{features, Exec, SimdLevel};
use std::hint::black_box;
use std::time::{Duration, Instant};

fn data(n: usize, s: usize) -> Vec<f32> {
    (0..n).map(|i| (((i * s + 3) % 29) as f32 - 14.0) * 0.0371).collect()
}

/// Median ns/call over ~budget, after warm-up.
fn time_it(budget: Duration, mut f: impl FnMut()) -> f64 {
    for _ in 0..3 { f(); }
    let t = Instant::now();
    f();
    let one = t.elapsed().as_nanos().max(1) as f64;
    let per_sample = ((budget.as_nanos() as f64 / 15.0) / one).clamp(1.0, 1e6) as usize;
    let mut samples: Vec<f64> = (0..15)
        .map(|_| {
            let t = Instant::now();
            for _ in 0..per_sample { f(); }
            t.elapsed().as_nanos() as f64 / per_sample as f64
        })
        .collect();
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    samples[samples.len() / 2]
}

fn main() {
    // `cargo test --benches` builds unoptimized (debug_assertions on): run a tiny smoke pass only.
    let quick = cfg!(debug_assertions) || std::env::args().any(|a| a == "--test" || a == "--list");
    let ms: u64 = std::env::var("GALION_BENCH_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(if quick { 5 } else { 200 });
    let budget = Duration::from_millis(ms);
    println!("galion-tensor gemv bench | {} | threads={}", features::report(), galion_tensor::threading::num_threads());
    let levels: Vec<SimdLevel> = [SimdLevel::Scalar, SimdLevel::Neon, SimdLevel::Avx2, SimdLevel::Avx512]
        .into_iter()
        .filter(|&l| l == SimdLevel::Scalar || (l.compiled() && l.is_implemented() && features::cpu_supports(l)))
        .collect();
    let shapes: &[(usize, usize)] = if quick { &[(256, 256)] } else { &[(512, 512), (1024, 1024), (2048, 1024), (4096, 1024), (1024, 4096)] };
    println!("{:<10} {:<22} {:<8} {:>12} {:>9} {:>9} {:>8}", "shape", "kernel", "exec", "ns/call", "GFLOP/s", "GB/s", "x scalar");
    for &(m, k) in shapes {
        let (a, x, b) = (data(m * k, 7), data(k, 5), data(m, 3));
        let w = QuantRowsI8::quantize(&a, m, k).unwrap();
        let mut xq = vec![0i8; k];
        let xs = quantize_row_i8(&x, &mut xq);
        let xt = data(m, 11);
        let flops = 2.0 * (m * k) as f64;
        let shape = format!("{m}x{k}");
        let mut y = vec![0f32; m];
        let mut yt = vec![0f32; k];

        // Oracles.
        let mut r = vec![0f32; m];
        gemv_f32_into_with(SimdLevel::Scalar, &a, m, k, &x, Some(&b), &mut r, Exec::Scalar).unwrap();
        let mut ri8 = vec![0f32; m];
        gemv_i8_into_with(SimdLevel::Scalar, &w.data, &w.scales, m, k, &xq, xs, Some(&b), &mut ri8, Exec::Scalar).unwrap();
        let mut rt = vec![0f32; k];
        gemv_t_f32_into(SimdLevel::Scalar, &xt, &a, m, k, None, &mut rt, Exec::Scalar).unwrap();

        let mut base = [0f64; 3];
        for &exec in &[Exec::Scalar, Exec::Parallel] {
            for &lv in &levels {
                // f32 GEMV
                gemv_f32_into_with(lv, &a, m, k, &x, Some(&b), &mut y, exec).unwrap();
                assert!(y.iter().zip(&r).all(|(p, q)| (p - q).abs() <= 1e-4 * (1.0 + q.abs())), "{lv:?} f32 mismatch");
                let ns = time_it(budget, || gemv_f32_into_with(lv, black_box(&a), m, k, black_box(&x), Some(&b), &mut y, exec).unwrap());
                if lv == SimdLevel::Scalar && exec == Exec::Scalar { base[0] = ns; }
                row(&shape, &format!("f32 dot/{}", lv.name()), exec, ns, flops, (m * k * 4) as f64, base[0]);
                // int8 W8A8
                gemv_i8_into_with(lv, &w.data, &w.scales, m, k, &xq, xs, Some(&b), &mut y, exec).unwrap();
                assert_eq!(y, ri8, "{lv:?} i8 mismatch");
                let ns = time_it(budget, || gemv_i8_into_with(lv, black_box(&w.data), &w.scales, m, k, black_box(&xq), xs, Some(&b), &mut y, exec).unwrap());
                if lv == SimdLevel::Scalar && exec == Exec::Scalar { base[1] = ns; }
                row(&shape, &format!("i8 w8a8/{}", lv.name()), exec, ns, flops, (m * k + 4 * m) as f64, base[1]);
                // f32 transposed (matmul rows==1 with [k,n] B)
                gemv_t_f32_into(lv, &xt, &a, m, k, None, &mut yt, exec).unwrap();
                assert_eq!(yt, rt, "{lv:?} gemv_t mismatch");
                let ns = time_it(budget, || gemv_t_f32_into(lv, black_box(&xt), black_box(&a), m, k, None, &mut yt, exec).unwrap());
                if lv == SimdLevel::Scalar && exec == Exec::Scalar { base[2] = ns; }
                row(&shape, &format!("f32 t-axpy/{}", lv.name()), exec, ns, flops, (m * k * 4) as f64, base[2]);
            }
        }
    }
}

fn row(shape: &str, kernel: &str, exec: Exec, ns: f64, flops: f64, bytes: f64, base: f64) {
    println!(
        "{:<10} {:<22} {:<8} {:>12.0} {:>9.2} {:>9.2} {:>8.2}",
        shape, kernel, format!("{exec:?}"), ns, flops / ns, bytes / ns, base / ns
    );
}
