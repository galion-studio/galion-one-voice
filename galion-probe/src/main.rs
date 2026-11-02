//! galion-probe — Galion Voice CPU readiness probe. Perpetual Open MIT License.
//! std-only, read-only: reads /proc + /sys, runs in-memory benchmarks, prints JSON.
//! Usage: galion-probe [--threads N] [--mb SIZE_PER_ARRAY] [--secs S]
use std::{fs, thread, time::Instant};

fn read(p: &str) -> String { fs::read_to_string(p).unwrap_or_default() }
fn esc(s: &str) -> String { s.replace('\\', "\\\\").replace('"', "\\\"") }

fn cpuinfo() -> (String, usize, String) {
    let t = read("/proc/cpuinfo");
    let mut model = String::new(); let mut flags = String::new(); let mut n = 0;
    for l in t.lines() {
        if l.starts_with("processor") { n += 1; }
        if model.is_empty() && (l.starts_with("model name") || l.starts_with("Model")) {
            model = l.splitn(2, ':').nth(1).unwrap_or("").trim().into(); }
        if flags.is_empty() && (l.starts_with("flags") || l.starts_with("Features")) {
            flags = l.splitn(2, ':').nth(1).unwrap_or("").trim().into(); }
    }
    (model, n, flags)
}

fn meminfo(key: &str) -> u64 {
    read("/proc/meminfo").lines().find(|l| l.starts_with(key))
        .and_then(|l| l.split_whitespace().nth(1)).and_then(|v| v.parse().ok()).unwrap_or(0)
}

fn caches() -> Vec<String> {
    let mut v = vec![];
    for i in 0..8 {
        let b = format!("/sys/devices/system/cpu/cpu0/cache/index{i}");
        let sz = read(&format!("{b}/size")); if sz.is_empty() { break; }
        v.push(format!("L{}{}={}", read(&format!("{b}/level")).trim(),
            match read(&format!("{b}/type")).trim() { "Data" => "d", "Instruction" => "i", _ => "" }, sz.trim()));
    }
    v
}

/// STREAM triad a[i] = b[i] + s*c[i], multi-threaded over disjoint chunks. Returns best GB/s.
fn triad(threads: usize, n: usize, reps: usize) -> f64 {
    let mut a = vec![0f64; n]; let b = vec![1.0f64; n]; let c = vec![2.0f64; n];
    let s = 3.0f64; let chunk = n.div_ceil(threads); let mut best = f64::MAX;
    for _ in 0..reps {
        let t0 = Instant::now();
        thread::scope(|sc| {
            for ((ac, bc), cc) in a.chunks_mut(chunk).zip(b.chunks(chunk)).zip(c.chunks(chunk)) {
                sc.spawn(move || for i in 0..ac.len() { ac[i] = bc[i] + s * cc[i]; });
            }
        });
        best = best.min(t0.elapsed().as_secs_f64());
    }
    std::hint::black_box(&a);
    (3 * n * 8) as f64 / best / 1e9
}

/// f32 GEMV y = W x (rows x cols), the decode-step hot loop of a small LM. Returns best GFLOP/s.
fn gemv(threads: usize, rows: usize, cols: usize, reps: usize) -> f64 {
    let w = vec![0.5f32; rows * cols]; let x = vec![1.0f32; cols]; let mut y = vec![0f32; rows];
    let rc = rows.div_ceil(threads); let mut best = f64::MAX;
    for _ in 0..reps {
        let t0 = Instant::now();
        thread::scope(|sc| {
            for (ti, yc) in y.chunks_mut(rc).enumerate() {
                let (w, x) = (&w, &x);
                sc.spawn(move || for (j, yo) in yc.iter_mut().enumerate() {
                    let r = &w[(ti * rc + j) * cols..][..cols];
                    let mut acc = [0f32; 8];
                    for (k8, x8) in r.chunks_exact(8).zip(x.chunks_exact(8)) {
                        for l in 0..8 { acc[l] += k8[l] * x8[l]; } }
                    *yo = acc.iter().sum();
                });
            }
        });
        best = best.min(t0.elapsed().as_secs_f64());
    }
    std::hint::black_box(&y);
    (2 * rows * cols) as f64 / best / 1e9
}

/// Minimal LOCKS.toml reader: `key = value` inside `[probe]` (std-only, no toml crate).
fn locks(path: &str) -> Vec<(String, String)> {
    let mut sec = String::new(); let mut v = vec![];
    for l in read(path).lines() {
        let l = l.split('#').next().unwrap_or("").trim();
        if l.starts_with('[') { sec = l.trim_matches(|c| c == '[' || c == ']').into(); continue; }
        if sec != "probe" { continue; }
        if let Some((k, val)) = l.split_once('=') { v.push((k.trim().into(), val.trim().into())); }
    }
    v
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let sarg = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
    let arg = |k: &str| sarg(k).and_then(|v| v.parse::<usize>().ok());
    let lpath = sarg("--locks").unwrap_or_else(|| "../LOCKS.toml".into());
    let lk = locks(&lpath);
    let lf = |k: &str, d: f64| lk.iter().find(|(a, _)| a == k).and_then(|(_, v)| v.parse::<f64>().ok()).unwrap_or(d);
    let sizes: Vec<f64> = lk.iter().find(|(a, _)| a == "student_sizes_b")
        .map(|(_, v)| v.trim_matches(|c| c == '[' || c == ']').split(',').filter_map(|x| x.trim().parse().ok()).collect())
        .unwrap_or_else(|| vec![0.3, 0.6, 1.0, 1.5, 3.0]);
    let (cv_max, min_gbs, hz, head) = (lf("bw_cv_max_pct", 5.0), lf("bw_min_gbs_nt", 8.0), lf("frame_hz", 12.5), lf("realtime_headroom", 2.0));
    let (bq8, bq4) = (lf("bytes_per_param_q8", 1.07), lf("bytes_per_param_q4", 0.57));
    let runs = (lf("bw_runs", 3.0) as usize).max(2);
    let (model, logical, flags) = cpuinfo();
    let par = thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
    let threads = arg("--threads").unwrap_or(par).max(1);
    let mb = arg("--mb").unwrap_or(256);
    let n = mb * 1024 * 1024 / 8;
    let has = |f: &str| flags.split_whitespace().any(|x| x == f);
    let simd: Vec<&str> = ["sse4_2", "avx", "avx2", "fma", "avx512f", "avx512_vnni", "avx_vnni", "f16c", "neon", "asimd", "asimddp"]
        .into_iter().filter(|f| has(f)).collect();
    let mem_total = meminfo("MemTotal:"); let mem_avail = meminfo("MemAvailable:");
    let triad1 = triad(1, n, 5);
    let bw: Vec<f64> = (0..runs).map(|_| triad(threads, n, 5)).collect();
    let mean = bw.iter().sum::<f64>() / runs as f64;
    let sd = (bw.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (runs - 1) as f64).sqrt();
    let cv = 100.0 * sd / mean; let triad_n = bw.iter().cloned().fold(f64::MIN, f64::max);
    // 2048x2048 f32 = 16 MiB weights (cache-busting on small VPS); 4096x4096 = 64 MiB.
    let g_small = gemv(threads, 2048, 2048, 20); let g_big = gemv(threads, 4096, 4096, 10);
    // Bandwidth-bound decode estimate uses MEAN (conservative): tps ~= GB/s / weight bytes.
    let tps_q8_1b5 = mean / (1.5 * bq8); let tps_q4_1b5 = mean / (1.5 * bq4);
    let gate = if tps_q4_1b5 >= hz { "PASS" } else { "FAIL" };
    let need = hz * head; let avail_gb = mem_avail as f64 / 1048576.0;
    let mut table = vec![]; let mut rec = String::from("none");
    for &p in &sizes {
        let (t8, t4) = (mean / (p * bq8), mean / (p * bq4));
        let fits = p * bq8 * 1.5 < avail_gb; // weights + 50% for KV/activations
        let v = if t8 >= need && fits { "q8" } else if t4 >= need && p * bq4 * 1.5 < avail_gb { "q4" } else { "no" };
        if v != "no" { rec = format!("{p}B-{v}"); }
        table.push(format!("{{\"params_b\": {p}, \"tps_q8\": {t8:.1}, \"tps_q4\": {t4:.1}, \"verdict\": \"{v}\"}}"));
    }
    let g0 = if cv < cv_max && mean >= min_gbs { "PASS" } else { "FAIL" };
    println!("{{\n  \"schema_version\": 1,\n  \"probe\": \"galion-probe 0.1.0\",\n  \"hostname\": \"{}\",\n  \"kernel\": \"{}\",\n  \"arch\": \"{}\",\n  \"cpu_model\": \"{}\",\n  \"logical_cpus\": {},\n  \"available_parallelism\": {},\n  \"simd\": {:?},\n  \"caches\": {:?},\n  \"mem_total_kib\": {},\n  \"mem_available_kib\": {},\n  \"swap_total_kib\": {},\n  \"bench_threads\": {},\n  \"triad_array_mib\": {},\n  \"stream_triad_gbs_1t\": {:.2},\n  \"stream_triad_gbs_nt\": {:.2},\n  \"bw_runs_gbs\": [{}],\n  \"bw_mean_gbs\": {:.2},\n  \"bw_cv_pct\": {:.2},\n  \"gemv_f32_gflops_2048\": {:.2},\n  \"gemv_f32_gflops_4096\": {:.2},\n  \"est_decode_tps_1b5_q8\": {:.1},\n  \"est_decode_tps_1b5_q4\": {:.1},\n  \"gate_realtime_12_5hz_q4_1b5\": \"{}\",\n  \"thresholds\": {{\"locks\": \"{}\", \"bw_cv_max_pct\": {}, \"bw_min_gbs_nt\": {}, \"frame_hz\": {}, \"realtime_headroom\": {}}},\n  \"student_sizing\": [{}],\n  \"student_recommendation\": \"{}\",\n  \"g0\": \"{}\"\n}}",
        esc(read("/proc/sys/kernel/hostname").trim()), esc(read("/proc/sys/kernel/osrelease").trim()), std::env::consts::ARCH,
        esc(&model), logical, par, simd, caches(), mem_total, mem_avail, meminfo("SwapTotal:"), threads, mb,
        triad1, triad_n, bw.iter().map(|x| format!("{x:.2}")).collect::<Vec<_>>().join(", "), mean, cv,
        g_small, g_big, tps_q8_1b5, tps_q4_1b5, gate, esc(&lpath), cv_max, min_gbs, hz, head, table.join(", "), rec, g0);
    if args.iter().any(|a| a == "--strict") && g0 != "PASS" { std::process::exit(1); }
}
