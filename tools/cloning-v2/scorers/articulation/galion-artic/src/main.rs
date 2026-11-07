//! galion-artic: Rust-native acoustic articulation / mouth-expression scorer.
mod analysis; mod audio; mod dsp; mod dtw; mod formant; mod pitch; mod stats;
use analysis::{analyze, r2, Analysis, Opts, HOP};
use serde_json::{json, Value};
use stats::*;

const USAGE: &str = "galion-artic — acoustic articulation scorer (JSON to stdout)
USAGE:
  galion-artic profile [opts] FILE.wav...          per-file profiles + distribution across files
  galion-artic compare [opts] --ref REF.wav --clone CLONE.wav [--clone C2.wav ...]
                                                  same-text pair(s): MCD-DTW, DTW-aligned vowel formant deltas, F0, metric deltas
OPTIONS:
  -j N              worker threads (default 2)
  --ceiling HZ      formant ceiling (default auto: 5500 if median F0>160 Hz else 5000)
  --fmin HZ --fmax HZ   F0 search range (default 65..600)
  --min-pause S     pause threshold (default 0.3)
  --pyin-beta-mean M  pYIN threshold prior mean (0.1 strict .. 0.2 lenient; default 0.2)
  --frames          include per-frame tracks (t, f0, f0_yin, int_db, f1..f3)
  --out FILE        write JSON to FILE instead of stdout
  --compact         single-line JSON";

/// Scalar metrics surfaced in deltas / distributions: (label, json pointer)
const METRICS: &[(&str, &str)] = &[
    ("f0_median_hz", "/f0/pyin/median_hz"), ("f0_sd_st", "/f0/pyin/sd_st"), ("f0_range_st", "/f0/pyin/range_p5_p95_st"),
    ("f1_vowel_median_hz", "/vowels/f1/median"), ("f2_vowel_median_hz", "/vowels/f2/median"), ("f3_vowel_median_hz", "/vowels/f3/median"),
    ("vsa_hull_hz2", "/vowel_space/hull_area_hz2"), ("vsa_hull_trimmed_hz2", "/vowel_space/hull_area_trimmed_hz2"), ("vsa_bark2", "/vowel_space/hull_area_bark2"), ("vsa_trimmed_bark2", "/vowel_space/hull_area_trimmed_bark2"),
    ("vowel_dispersion_bark", "/vowel_space/dispersion_bark"), ("f1_range_hz", "/vowel_space/f1_range_p5_p95_hz"), ("f2_range_hz", "/vowel_space/f2_range_p5_p95_hz"),
    ("articulation_rate_syl_s", "/rate/articulation_rate_syl_s"), ("speech_rate_syl_s", "/rate/speech_rate_syl_s"),
    ("pause_ratio", "/pauses/pause_ratio"), ("pauses_per_min", "/pauses/per_min"), ("pause_median_s", "/pauses/dur/median"),
    ("fric_centroid_hz", "/consonants/fricative/centroid_hz/median"), ("fric_sd_hz", "/consonants/fricative/sd_hz/median"),
    ("fric_skew", "/consonants/fricative/skewness/median"), ("fric_kurt", "/consonants/fricative/kurtosis/median"),
    ("sibilant_centroid_hz", "/consonants/fricative/sibilant_centroid_hz"), ("fric_frac", "/consonants/fricative/frac_of_speech"),
    ("bursts_per_s", "/consonants/plosive/per_s"), ("burst_rel_db", "/consonants/plosive/burst_rel_db/median"),
    ("burst_centroid_hz", "/consonants/plosive/burst_centroid_hz/median"), ("vot_ms", "/consonants/plosive/vot_ms/median"),
    ("hf_tilt_4k_db", "/consonants/hf_tilt_4k_db"), ("dynamic_range_db", "/intensity/dynamic_range_p5_p95_db"),
];

fn get(v: &Value, p: &str) -> f64 { v.pointer(p).and_then(|x| x.as_f64()).unwrap_or(f64::NAN) }

fn par_analyze(files: &[String], o: &Opts, j: usize) -> Vec<Result<Analysis, String>> {
    let next = std::sync::atomic::AtomicUsize::new(0);
    let out: Vec<std::sync::Mutex<Option<Result<Analysis, String>>>> = files.iter().map(|_| std::sync::Mutex::new(None)).collect();
    std::thread::scope(|s| {
        for _ in 0..j.max(1).min(files.len().max(1)) {
            s.spawn(|| loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if i >= files.len() { break; }
                *out[i].lock().unwrap() = Some(analyze(&files[i], o));
            });
        }
    });
    out.into_iter().map(|m| m.into_inner().unwrap().unwrap()).collect()
}

fn distribution(profiles: &[&Value]) -> Value {
    let mut d = serde_json::Map::new();
    for (k, p) in METRICS {
        let v: Vec<f64> = profiles.iter().map(|pr| get(pr, p)).collect();
        let n = v.iter().filter(|x| x.is_finite()).count();
        d.insert(k.to_string(), json!({"n": n, "mean": r2(mean(&v)), "sd": r2(sd(&v)), "median": r2(median(&v)), "min": r2(quantile(&v,0.0)), "max": r2(quantile(&v,1.0))}));
    }
    Value::Object(d)
}

fn compare(r: &Analysis, c: &Analysis) -> Value {
    let (rs, re) = r.span; let (cs, ce) = c.span;
    // DTW over speech (non-silent) frames only: silence/noise-floor frames would dominate cepstral distance
    let ri: Vec<usize> = (rs..=re).filter(|&k| r.speech[k]).collect();
    let ci: Vec<usize> = (cs..=ce).filter(|&k| c.speech[k]).collect();
    let rm: Vec<_> = ri.iter().map(|&k| r.mfcc[k]).collect();
    let cm: Vec<_> = ci.iter().map(|&k| c.mfcc[k]).collect();
    let (path, mcd) = dtw::dtw(&rm, &cm);
    let path: Vec<(usize, usize)> = path.into_iter().map(|(i, j)| (ri[i], ci[j])).collect();
    // F0 on aligned voiced frames
    let cents: Vec<f64> = path.iter().filter(|&&(i, j)| r.f0[i] > 0.0 && c.f0[j] > 0.0).map(|&(i, j)| 1200.0 * (c.f0[j] / r.f0[i]).log2()).collect();
    let off = median(&cents);
    let mad = median(&cents.iter().map(|x| (x - off).abs()).collect::<Vec<_>>());
    let rmse = (cents.iter().map(|x| (x - off).powi(2)).sum::<f64>() / cents.len().max(1) as f64).sqrt();
    let pairs: Vec<(f64, f64)> = path.iter().filter(|&&(i, j)| r.f0[i] > 0.0 && c.f0[j] > 0.0).map(|&(i, j)| (r.f0[i].ln(), c.f0[j].ln())).collect();
    let corr = pearson(&pairs);
    let vuv_agree = path.iter().filter(|&&(i, j)| (r.f0[i] > 0.0) == (c.f0[j] > 0.0)).count() as f64 / path.len().max(1) as f64;
    // vowel-aligned formant deltas
    let mut per = Vec::new();
    let mut dl: [Vec<f64>; 3] = [vec![], vec![], vec![]];
    let mut dp: [Vec<f64>; 3] = [vec![], vec![], vec![]];
    let mut cpts = Vec::new(); let mut rpts = Vec::new();
    for v in &r.vowels {
        let js: Vec<usize> = path.iter().filter(|&&(i, _)| i >= v.s && i <= v.e).map(|&(_, j)| j).filter(|&j| c.f0[j] > 0.0).collect();
        if js.is_empty() { continue; }
        let mut cf = [f64::NAN; 3];
        for d in 0..3 { cf[d] = median(&js.iter().map(|&j| c.form[j][d]).collect::<Vec<_>>()); }
        for d in 0..3 { if cf[d].is_finite() && v.f[d].is_finite() { dl[d].push(cf[d] - v.f[d]); dp[d].push((cf[d] - v.f[d]) / v.f[d] * 100.0); } }
        rpts.push((v.f[1], v.f[0])); cpts.push((cf[1], cf[0]));
        per.push(json!({"t_ref": r2(r.times[v.peak]), "ref": [r2(v.f[0]), r2(v.f[1]), r2(v.f[2])], "clone": [r2(cf[0]), r2(cf[1]), r2(cf[2])]}));
    }
    let fd = |d: usize| json!({"mean_signed_hz": r2(mean(&dl[d])), "mean_abs_hz": r2(mean(&dl[d].iter().map(|x| x.abs()).collect::<Vec<_>>())),
        "median_abs_hz": r2(median(&dl[d].iter().map(|x| x.abs()).collect::<Vec<_>>())), "mean_abs_pct": r2(mean(&dp[d].iter().map(|x| x.abs()).collect::<Vec<_>>()))});
    let mut deltas = serde_json::Map::new();
    for (k, p) in METRICS {
        let (a, b) = (get(&r.profile, p), get(&c.profile, p));
        deltas.insert(k.to_string(), json!({"ref": r2(a), "clone": r2(b), "delta": r2(b - a), "ratio": r2(if a != 0.0 { b / a } else { f64::NAN })}));
    }
    let durr = (ce - cs + 1) as f64 / (re - rs + 1) as f64;
    // heuristic 0-100 sub-scores (documented in README)
    let ratio_score = |a: f64, b: f64| if a > 0.0 && b > 0.0 { (a / b).min(b / a) } else { f64::NAN };
    let f_abs_pct = mean(&[mean(&dp[0].iter().map(|x| x.abs()).collect::<Vec<_>>()), mean(&dp[1].iter().map(|x| x.abs()).collect::<Vec<_>>())]);
    let s_mcd = (1.0 - (mcd - 4.0) / 8.0).clamp(0.0, 1.0);
    let s_form = (-f_abs_pct / 20.0).exp();
    let s_vsa = ratio_score(get(&r.profile, "/vowel_space/hull_area_trimmed_bark2"), get(&c.profile, "/vowel_space/hull_area_trimmed_bark2"));
    let s_rate = ratio_score(get(&r.profile, "/rate/articulation_rate_syl_s"), get(&c.profile, "/rate/articulation_rate_syl_s"));
    let fcr = get(&r.profile, "/consonants/fricative/centroid_hz/median"); let fcc = get(&c.profile, "/consonants/fricative/centroid_hz/median");
    let s_cons = if fcr.is_finite() && fcc.is_finite() { (-(fcc - fcr).abs() / 1500.0).exp() } else { f64::NAN };
    let subs = [s_mcd, s_form, s_vsa, s_rate, s_cons];
    let ok: Vec<f64> = subs.iter().cloned().filter(|x| x.is_finite()).collect();
    let score = 100.0 * ok.iter().sum::<f64>() / ok.len().max(1) as f64;
    json!({
        "ref": r.path, "clone": c.path,
        "mcd": {"mcd_dtw_db": r2(mcd), "coeffs": "c1..c24 of 25 MFCC (40 mel, 16 kHz)", "n_ref_speech_frames": ri.len(), "n_clone_speech_frames": ci.len(), "path_len": path.len(), "duration_ratio_clone_ref": r2(durr)},
        "f0_aligned": {"n": cents.len(), "median_offset_cents": r2(off), "rmse_cents_after_offset": r2(rmse), "mad_cents_after_offset": r2(mad), "log_f0_corr": r2(corr), "vuv_agreement": r2(vuv_agree)},
        "vowels_aligned": {"n": per.len(), "f1": fd(0), "f2": fd(1), "f3": fd(2), "vsa_hull_hz2_ref_aligned": r2(hull_area(&rpts)), "vsa_hull_hz2_clone_aligned": r2(hull_area(&cpts)), "per_vowel": per},
        "deltas": Value::Object(deltas),
        "score": {"articulation_score_0_100": r2(score), "s_mcd": r2(s_mcd), "s_formant": r2(s_form), "s_vsa": r2(s_vsa), "s_rate": r2(s_rate), "s_consonant": r2(s_cons)}
    })
}

fn pearson(p: &[(f64, f64)]) -> f64 {
    if p.len() < 3 { return f64::NAN; }
    let n = p.len() as f64;
    let (mx, my) = (p.iter().map(|q| q.0).sum::<f64>() / n, p.iter().map(|q| q.1).sum::<f64>() / n);
    let (mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0);
    for q in p { sxy += (q.0 - mx) * (q.1 - my); sxx += (q.0 - mx).powi(2); syy += (q.1 - my).powi(2); }
    sxy / (sxx * syy).sqrt()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args[0] == "-h" || args[0] == "--help" { println!("{USAGE}"); return; }
    let mode = args[0].clone();
    let mut o = Opts::default();
    let (mut j, mut out, mut compact) = (2usize, None::<String>, false);
    let (mut files, mut refp, mut clones) = (Vec::new(), None::<String>, Vec::new());
    let mut i = 1;
    let num = |a: &Vec<String>, i: usize| -> f64 { a.get(i).and_then(|s| s.parse().ok()).unwrap_or_else(|| { eprintln!("bad value for {}", a[i - 1]); std::process::exit(2) }) };
    while i < args.len() {
        match args[i].as_str() {
            "-j" => { i += 1; j = num(&args, i) as usize; }
            "--ceiling" => { i += 1; o.ceiling = Some(num(&args, i)); }
            "--fmin" => { i += 1; o.fmin = num(&args, i); }
            "--fmax" => { i += 1; o.fmax = num(&args, i); }
            "--pyin-beta-mean" => { i += 1; o.beta_mean = num(&args, i); }
            "--min-pause" => { i += 1; o.min_pause = num(&args, i); }
            "--frames" => o.frames = true,
            "--compact" => compact = true,
            "--out" => { i += 1; out = args.get(i).cloned(); }
            "--ref" => { i += 1; refp = args.get(i).cloned(); }
            "--clone" => { i += 1; if let Some(c) = args.get(i) { clones.push(c.clone()); } }
            s => files.push(s.to_string()),
        }
        i += 1;
    }
    let t0 = std::time::Instant::now();
    let mut res = match mode.as_str() {
        "profile" => {
            if files.is_empty() { eprintln!("{USAGE}"); std::process::exit(2); }
            let an = par_analyze(&files, &o, j);
            let mut profs = Vec::new(); let mut errs = Vec::new();
            for (f, a) in files.iter().zip(an) { match a { Ok(a) => profs.push(a.profile), Err(e) => errs.push(json!({"file": f, "error": e})) } }
            let refs: Vec<&Value> = profs.iter().collect();
            json!({"mode": "profile", "n_files": profs.len(), "distribution": distribution(&refs), "files": profs, "errors": errs})
        }
        "compare" => {
            let Some(rp) = refp else { eprintln!("compare needs --ref"); std::process::exit(2) };
            clones.extend(files.into_iter());
            if clones.is_empty() { eprintln!("compare needs --clone"); std::process::exit(2); }
            let mut all = vec![rp.clone()]; all.extend(clones.iter().cloned());
            let mut an = par_analyze(&all, &o, j);
            let r = match an.remove(0) { Ok(a) => a, Err(e) => { eprintln!("{e}"); std::process::exit(1) } };
            let mut cmp = Vec::new();
            let mut clones_p = Vec::new();
            for a in an { match a { Ok(c) => { cmp.push(compare(&r, &c)); clones_p.push(c.profile); } Err(e) => cmp.push(json!({"error": e})) } }
            json!({"mode": "compare", "ref_profile": r.profile.clone(), "comparisons": cmp, "clone_profiles": clones_p})
        }
        _ => { eprintln!("{USAGE}"); std::process::exit(2); }
    };
    res["elapsed_ms"] = r2(t0.elapsed().as_secs_f64() * 1000.0);
    res["version"] = json!(env!("CARGO_PKG_VERSION"));
    let _ = HOP;
    let s = if compact { serde_json::to_string(&res).unwrap() } else { serde_json::to_string_pretty(&res).unwrap() };
    match out { Some(p) => std::fs::write(&p, s + "\n").expect("write out"), None => println!("{s}") }
}
