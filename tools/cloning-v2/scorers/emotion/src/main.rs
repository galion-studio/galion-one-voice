//! galion-emotion: Rust-native emotion scorer for voice clones.
mod audio;
mod models;
mod proxies;

use anyhow::{bail, Context, Result};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::time::Instant;

const MODELS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/models");
const SCHEMA_VERSION: &str = "galion-emotion/1";

const USAGE: &str = "galion-emotion score [--ref REF.wav] [options] FILE.wav...

Options:
  --ref PATH          reference recording; adds per-file `vs_ref` and a summary
  --out PATH          write JSON to PATH (default stdout)
  --threads N         ORT intra-op threads (default 2)
  --ser-model PATH    categorical SER ONNX (default <crate>/models/superb_er.onnx)
  --dim-model PATH    A/D/V ONNX (default <crate>/models/msp_dim.onnx)
  --int8              use dynamic-int8 quantized models (*.int8.onnx; ~3x smaller, faster, small accuracy cost)
  --no-ser            skip categorical SER
  --no-dim            skip dimensional model (fastest: SER + proxies)
  --no-models         acoustic proxies only (pure Rust, no ONNX)
  --ort-lib PATH      libonnxruntime.so (default: $ORT_DYLIB_PATH or the box's 1.31.1 build)
  --compact           single-line JSON
";

fn cpu_s() -> f64 {
    // utime+stime from /proc/self/stat (clock ticks, 100 Hz on Linux)
    std::fs::read_to_string("/proc/self/stat")
        .ok()
        .and_then(|s| {
            let r = s.rsplit(')').next()?.split_whitespace().collect::<Vec<_>>();
            Some((r[11].parse::<f64>().ok()? + r[12].parse::<f64>().ok()?) / 100.0)
        })
        .unwrap_or(f64::NAN)
}

#[derive(Serialize, Clone)]
struct Ser {
    probs: BTreeMap<String, f32>,
    top: String,
    top_conf: f32,
}
#[derive(Serialize, Clone, Copy)]
struct Avd {
    arousal: f32,
    valence: f32,
    dominance: f32,
}
#[derive(Serialize, Clone)]
struct FileScore {
    file: String,
    duration_s: f32,
    sample_rate: u32,
    ser: Option<Ser>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ser_vec: Option<Vec<f32>>, // internal, not serialized when None
    avd: Option<Avd>,
    proxies: proxies::Proxies,
    timing_ms: BTreeMap<String, f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    vs_ref: Option<Value>,
}

struct Scorer {
    ser: Option<ort::session::Session>,
    dim: Option<ort::session::Session>,
}

impl Scorer {
    fn score(&mut self, path: &str) -> Result<FileScore> {
        let mut tm = BTreeMap::new();
        let t = Instant::now();
        let a = audio::load_wav(path)?;
        let x16 = audio::resample(&a.samples, a.sr, proxies::SR as u32);
        tm.insert("load_resample".into(), t.elapsed().as_secs_f64() * 1e3);
        let t = Instant::now();
        let px = proxies::compute(&x16, &a.samples, a.sr as usize);
        tm.insert("proxies".into(), t.elapsed().as_secs_f64() * 1e3);
        let mut ser = None;
        let mut ser_vec = None;
        if let Some(s) = self.ser.as_mut() {
            let t = Instant::now();
            let p = models::ser(s, &x16)?;
            tm.insert("ser".into(), t.elapsed().as_secs_f64() * 1e3);
            let (bi, bv) = p.iter().enumerate().fold((0, f32::MIN), |acc, (i, &v)| if v > acc.1 { (i, v) } else { acc });
            ser = Some(Ser {
                probs: models::SER_LABELS.iter().zip(&p).map(|(l, v)| (l.to_string(), *v)).collect(),
                top: models::SER_LABELS[bi].into(),
                top_conf: bv,
            });
            ser_vec = Some(p);
        }
        let mut avd = None;
        if let Some(s) = self.dim.as_mut() {
            let t = Instant::now();
            let [ar, dom, val] = models::adv(s, &x16)?;
            tm.insert("dim".into(), t.elapsed().as_secs_f64() * 1e3);
            avd = Some(Avd { arousal: ar, valence: val, dominance: dom });
        }
        Ok(FileScore {
            file: path.into(),
            duration_s: a.samples.len() as f32 / a.sr as f32,
            sample_rate: a.sr,
            ser,
            ser_vec,
            avd,
            proxies: px,
            timing_ms: tm,
            vs_ref: None,
        })
    }
}

fn js_div(p: &[f32], q: &[f32]) -> f32 {
    let m: Vec<f32> = p.iter().zip(q).map(|(a, b)| 0.5 * (a + b)).collect();
    let kl = |a: &[f32]| a.iter().zip(&m).map(|(x, y)| if *x > 0.0 { x * (x / y).log2() } else { 0.0 }).sum::<f32>();
    0.5 * kl(p) + 0.5 * kl(q)
}

fn fin(v: f32) -> Option<f32> {
    if v.is_finite() { Some(v) } else { None }
}

/// Clone-vs-reference emotion distance. Composite = mean of clipped normalized terms
/// (each term/scale, capped at 1); emotion_similarity = 1 - composite.
fn compare(c: &FileScore, r: &FileScore) -> Value {
    let (cp, rp) = (&c.proxies, &r.proxies);
    let mut terms: Vec<(&str, f32, f32)> = Vec::new(); // (name, |delta|, scale)
    let mut out = serde_json::Map::new();
    if let (Some(cs), Some(rs)) = (&c.ser, &r.ser) {
        let js = js_div(c.ser_vec.as_ref().unwrap(), r.ser_vec.as_ref().unwrap());
        out.insert("label_match".into(), json!(cs.top == rs.top));
        out.insert("ser_js_div".into(), json!(js));
        terms.push(("ser_js", js, 0.25));
    }
    if let (Some(ca), Some(ra)) = (c.avd, r.avd) {
        let (da, dv, dd) = (ca.arousal - ra.arousal, ca.valence - ra.valence, ca.dominance - ra.dominance);
        let dist = (da * da + dv * dv + dd * dd).sqrt();
        out.insert("avd_delta".into(), json!({"arousal": da, "valence": dv, "dominance": dd}));
        out.insert("avd_dist".into(), json!(dist));
        terms.push(("avd", dist, 0.15));
    }
    let f0_med_st = 12.0 * (cp.f0_median_hz / rp.f0_median_hz).log2();
    let rate_log2 = (cp.articulation_rate_syl_s / rp.articulation_rate_syl_s).log2();
    let pd = json!({
        "f0_median_st": fin(f0_med_st),
        "f0_sd_st": fin(cp.f0_sd_st - rp.f0_sd_st),
        "f0_range_st_p5_p95": fin(cp.f0_range_st_p5_p95 - rp.f0_range_st_p5_p95),
        "energy_dr_db": fin(cp.energy_dr_db - rp.energy_dr_db),
        "speech_rate_ratio": fin(cp.speech_rate_syl_s / rp.speech_rate_syl_s),
        "articulation_rate_ratio": fin(cp.articulation_rate_syl_s / rp.articulation_rate_syl_s),
        "pause_ratio": fin(cp.pause_ratio - rp.pause_ratio),
        "jitter_local_pct": fin(cp.jitter_local_pct - rp.jitter_local_pct),
        "shimmer_local_pct": fin(cp.shimmer_local_pct - rp.shimmer_local_pct),
        "spectral_tilt_db_oct": fin(cp.spectral_tilt_db_oct - rp.spectral_tilt_db_oct),
        "alpha_ratio_db": fin(cp.alpha_ratio_db - rp.alpha_ratio_db),
        "hammarberg_db": fin(cp.hammarberg_db - rp.hammarberg_db),
    });
    for (name, d, s) in [
        ("f0_median", f0_med_st.abs(), 2.0),
        ("f0_sd", (cp.f0_sd_st - rp.f0_sd_st).abs(), 1.0),
        ("energy_dr", (cp.energy_dr_db - rp.energy_dr_db).abs(), 6.0),
        ("rate", rate_log2.abs(), 0.32),
        ("tilt", (cp.spectral_tilt_db_oct - rp.spectral_tilt_db_oct).abs(), 3.0),
    ] {
        terms.push((name, d, s));
    }
    out.insert("proxy_delta".into(), pd);
    let used: Vec<(&str, f32)> = terms.iter().filter(|t| t.1.is_finite()).map(|(n, d, s)| (*n, (d / s).min(1.0))).collect();
    let comp = used.iter().map(|t| t.1).sum::<f32>() / used.len().max(1) as f32;
    out.insert("distance_terms".into(), json!(used.iter().map(|(n, v)| (n.to_string(), *v)).collect::<BTreeMap<_, _>>()));
    out.insert("emotion_distance".into(), json!(comp));
    out.insert("emotion_similarity".into(), json!(1.0 - comp));
    Value::Object(out)
}

fn summarize(files: &[FileScore], r: &FileScore) -> Value {
    let n = files.len() as f32;
    let mut counts: BTreeMap<String, u32> = BTreeMap::new();
    let mut matches = 0u32;
    for f in files {
        if let Some(s) = &f.ser {
            *counts.entry(s.top.clone()).or_default() += 1;
            if Some(&s.top) == r.ser.as_ref().map(|x| &x.top) { matches += 1; }
        }
    }
    let avg = |g: &dyn Fn(&FileScore) -> Option<f64>| -> Option<f64> {
        let v: Vec<f64> = files.iter().filter_map(|f| g(f)).filter(|x| x.is_finite()).collect();
        if v.is_empty() { None } else { Some(v.iter().sum::<f64>() / v.len() as f64) }
    };
    let vr = |f: &FileScore, k: &str| f.vs_ref.as_ref().and_then(|v| v.pointer(k)).and_then(|x| x.as_f64());
    json!({
        "n_files": files.len(),
        "ref_label": r.ser.as_ref().map(|s| s.top.clone()),
        "label_counts": counts,
        "label_match_rate": if r.ser.is_some() { Some(matches as f32 / n) } else { None },
        "mean_avd": {
            "arousal": avg(&|f| f.avd.map(|a| a.arousal as f64)),
            "valence": avg(&|f| f.avd.map(|a| a.valence as f64)),
            "dominance": avg(&|f| f.avd.map(|a| a.dominance as f64)),
        },
        "mean_avd_delta": {
            "arousal": avg(&|f| vr(f, "/avd_delta/arousal")),
            "valence": avg(&|f| vr(f, "/avd_delta/valence")),
            "dominance": avg(&|f| vr(f, "/avd_delta/dominance")),
        },
        "mean_avd_dist": avg(&|f| vr(f, "/avd_dist")),
        "mean_ser_js_div": avg(&|f| vr(f, "/ser_js_div")),
        "mean_proxy_delta": {
            "f0_median_st": avg(&|f| vr(f, "/proxy_delta/f0_median_st")),
            "f0_sd_st": avg(&|f| vr(f, "/proxy_delta/f0_sd_st")),
            "energy_dr_db": avg(&|f| vr(f, "/proxy_delta/energy_dr_db")),
            "articulation_rate_ratio": avg(&|f| vr(f, "/proxy_delta/articulation_rate_ratio")),
            "pause_ratio": avg(&|f| vr(f, "/proxy_delta/pause_ratio")),
            "jitter_local_pct": avg(&|f| vr(f, "/proxy_delta/jitter_local_pct")),
            "shimmer_local_pct": avg(&|f| vr(f, "/proxy_delta/shimmer_local_pct")),
            "spectral_tilt_db_oct": avg(&|f| vr(f, "/proxy_delta/spectral_tilt_db_oct")),
        },
        "mean_emotion_distance": avg(&|f| vr(f, "/emotion_distance")),
        "mean_emotion_similarity": avg(&|f| vr(f, "/emotion_similarity")),
    })
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args[0] == "-h" || args[0] == "--help" {
        print!("{USAGE}");
        return Ok(());
    }
    if args[0] != "score" {
        bail!("unknown command {:?}\n{USAGE}", args[0]);
    }
    let (mut refp, mut outp, mut threads) = (None, None, 2usize);
    let mut ser_model = format!("{MODELS_DIR}/superb_er.onnx");
    let mut dim_model = format!("{MODELS_DIR}/msp_dim.onnx");
    let (mut no_ser, mut no_dim, mut compact) = (false, false, false);
    let mut ort_lib: Option<String> = None;
    let mut int8 = false;
    let mut files = Vec::new();
    let mut it = args[1..].iter();
    while let Some(a) = it.next() {
        let mut val = || it.next().cloned().with_context(|| format!("{a} needs a value"));
        match a.as_str() {
            "--ref" => refp = Some(val()?),
            "--out" => outp = Some(val()?),
            "--threads" => threads = val()?.parse()?,
            "--ser-model" => ser_model = val()?,
            "--dim-model" => dim_model = val()?,
            "--no-ser" => no_ser = true,
            "--int8" => int8 = true,
            "--no-dim" => no_dim = true,
            "--no-models" => { no_ser = true; no_dim = true; }
            "--compact" => compact = true,
            "--ort-lib" => ort_lib = Some(val()?),
            s if s.starts_with("--") => bail!("unknown option {s}\n{USAGE}"),
            _ => files.push(a.clone()),
        }
    }
    if files.is_empty() && refp.is_none() {
        bail!("no input files\n{USAGE}");
    }
    if int8 {
        ser_model = ser_model.replace("superb_er.onnx", "superb_er.int8.onnx");
        dim_model = dim_model.replace("msp_dim.onnx", "msp_dim.int8.onnx");
    }
    let wall0 = Instant::now();
    let cpu0 = cpu_s();
    let t = Instant::now();
    if !(no_ser && no_dim) { models::init_ort(ort_lib.as_deref())?; }
    let mut sc = Scorer {
        ser: if no_ser { None } else { Some(models::session(&ser_model, threads)?) },
        dim: if no_dim { None } else { Some(models::session(&dim_model, threads)?) },
    };
    let load_ms = t.elapsed().as_secs_f64() * 1e3;
    let reference = match &refp { Some(p) => Some(sc.score(p)?), None => None };
    let mut scores = Vec::new();
    for f in &files {
        let mut s = sc.score(f)?;
        if let Some(r) = &reference { s.vs_ref = Some(compare(&s, r)); }
        scores.push(s);
    }
    let summary = reference.as_ref().map(|r| summarize(&scores, r));
    let strip = |mut s: FileScore| { s.ser_vec = None; s };
    let out = json!({
        "schema": SCHEMA_VERSION,
        "models": {
            "ser": if no_ser { None } else { Some(&ser_model) },
            "dim": if no_dim { None } else { Some(&dim_model) },
            "ser_labels": models::SER_LABELS,
            "threads": threads,
        },
        "reference": reference.map(strip),
        "files": scores.into_iter().map(strip).collect::<Vec<_>>(),
        "summary": summary,
        "timing": {
            "model_load_ms": load_ms,
            "wall_s": wall0.elapsed().as_secs_f64(),
            "cpu_s": cpu_s() - cpu0,
        }
    });
    let txt = if compact { serde_json::to_string(&out)? } else { serde_json::to_string_pretty(&out)? };
    match outp {
        Some(p) => std::fs::write(&p, txt + "\n").with_context(|| format!("write {p}"))?,
        None => println!("{txt}"),
    }
    Ok(())
}
