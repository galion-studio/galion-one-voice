//! galion-efficientat — EfficientAT mn10_as (AudioSet 527) via ort.
//! Primary detector for yelling / crying / laughter in the universal emotion stack.
//!
//! Model: Chris-Kode/efficientat-mn10-as-onnx (MIT), input 32 kHz mono [W,N],
//! output probabilities [W,527]. SHA-256 bec20620ab8be5e41297b45845f6fb116fbe42cd4da339ad7d44f27630b7a504.

mod classes;
mod audio;

use anyhow::{bail, Context, Result};
use classes::{StateScores, N_CLASSES};
use ort::session::{builder::GraphOptimizationLevel, Session};
use ort::value::Tensor;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const USAGE: &str = "galion-efficientat score [--out PATH] [--threads N] [--window-s S] [--hop-s H] FILE.wav...

Scores yelling / crying / laughter via EfficientAT AudioSet classes.
Windowed inference (default 2.5 s window, 1.25 s hop); per-file max and mean pooling.
";

const DEFAULT_ORT_LIB: &str =
    "/workspace/swarm/voice-v2-sprint/repo/weights/runtime/ort/onnxruntime-linux-x64-1.31.1/lib/libonnxruntime.so";
const DEFAULT_MODEL: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/efficientat-mn10-as.onnx");
const SR: u32 = 32_000;

fn init_ort() -> Result<()> {
    let p = std::env::var("ORT_DYLIB_PATH").unwrap_or_else(|_| DEFAULT_ORT_LIB.to_string());
    ort::init_from(p.clone())
        .with_name("galion-efficientat")
        .commit()
        .with_context(|| format!("init onnxruntime from {p}"))?;
    Ok(())
}

fn session(model: &Path, threads: usize) -> Result<Session> {
    Session::builder()?
        .with_optimization_level(GraphOptimizationLevel::Level3)?
        .with_intra_threads(threads)?
        .with_inter_threads(1)?
        .commit_from_file(model)
        .with_context(|| format!("load {}", model.display()))
}

/// Run one batch of windows: samples shape [W, N] -> probs [W, 527].
fn run_windows(sess: &mut Session, windows: &[Vec<f32>]) -> Result<Vec<Vec<f32>>> {
    if windows.is_empty() {
        return Ok(vec![]);
    }
    let w = windows.len();
    let n = windows[0].len();
    for (i, win) in windows.iter().enumerate() {
        if win.len() != n {
            bail!("window {i} len {} != {n}", win.len());
        }
    }
    let mut flat = Vec::with_capacity(w * n);
    for win in windows {
        flat.extend_from_slice(win);
    }
    let t = Tensor::from_array(([w, n], flat))?;
    let out = sess.run(ort::inputs!["samples" => t])?;
    // find probabilities output
    let (_, data) = out["probabilities"]
        .try_extract_tensor::<f32>()
        .or_else(|_| out[0].try_extract_tensor::<f32>())?;
    if data.len() != w * N_CLASSES {
        bail!("unexpected output len {} (want {}x{})", data.len(), w, N_CLASSES);
    }
    Ok(data.chunks(N_CLASSES).map(|c| c.to_vec()).collect())
}

fn pool_state(probs_windows: &[Vec<f32>]) -> StateScores {
    let mut max_y = 0.0f32;
    let mut max_c = 0.0f32;
    let mut max_l = 0.0f32;
    let mut sum_y = 0.0f32;
    let mut sum_c = 0.0f32;
    let mut sum_l = 0.0f32;
    let mut per_max: std::collections::BTreeMap<String, f32> = std::collections::BTreeMap::new();
    let n = probs_windows.len().max(1) as f32;
    for p in probs_windows {
        let s = StateScores::from_audioset(p);
        max_y = max_y.max(s.yelling);
        max_c = max_c.max(s.crying);
        max_l = max_l.max(s.laughter);
        sum_y += s.yelling;
        sum_c += s.crying;
        sum_l += s.laughter;
        for (k, v) in s.per_class_max {
            let e = per_max.entry(k).or_insert(0.0);
            *e = (*e).max(v);
        }
    }
    StateScores {
        yelling: max_y,
        crying: max_c,
        laughter: max_l,
        yelling_mean: sum_y / n,
        crying_mean: sum_c / n,
        laughter_mean: sum_l / n,
        per_class_max: per_max,
    }
}

fn score_file(sess: &mut Session, path: &Path, window_s: f64, hop_s: f64) -> Result<Value> {
    let (x32, sr) = audio::load_mono_resampled(path, SR)?;
    if sr != SR {
        bail!("resample failed");
    }
    let win = ((window_s * SR as f64).round() as usize).max(SR as usize / 2); // ≥0.5 s
    let hop = ((hop_s * SR as f64).round() as usize).max(1);
    let mut windows: Vec<Vec<f32>> = Vec::new();
    if x32.len() <= win {
        // pad to win for short clips
        let mut w = x32.clone();
        w.resize(win, 0.0);
        windows.push(w);
    } else {
        let mut start = 0;
        while start + win <= x32.len() {
            windows.push(x32[start..start + win].to_vec());
            start += hop;
        }
        if windows.is_empty() {
            let mut w = x32.clone();
            w.resize(win, 0.0);
            windows.push(w);
        }
    }
    // batch in chunks of ≤8 to bound memory
    let mut all_probs = Vec::new();
    for chunk in windows.chunks(8) {
        all_probs.extend(run_windows(sess, chunk)?);
    }
    let pooled = pool_state(&all_probs);
    Ok(json!({
        "file": path.to_string_lossy(),
        "sample_rate": SR,
        "duration_s": x32.len() as f64 / SR as f64,
        "n_windows": all_probs.len(),
        "window_s": window_s,
        "hop_s": hop_s,
        "source": "efficientat",
        "model": "Chris-Kode/efficientat-mn10-as-onnx",
        "states": {
            "yelling": {"max": pooled.yelling, "mean": pooled.yelling_mean, "source": "efficientat"},
            "crying": {"max": pooled.crying, "mean": pooled.crying_mean, "source": "efficientat"},
            "laughter": {"max": pooled.laughter, "mean": pooled.laughter_mean, "source": "efficientat"},
        },
        "per_class_max": pooled.per_class_max,
    }))
}

fn cmd_score(args: &[String]) -> Result<()> {
    let mut out: Option<PathBuf> = None;
    let mut threads = 2usize;
    let mut window_s = 2.5f64;
    let mut hop_s = 1.25f64;
    let mut model = PathBuf::from(DEFAULT_MODEL);
    let mut files: Vec<PathBuf> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => { out = Some(PathBuf::from(&args[i+1])); i += 2; }
            "--threads" => { threads = args[i+1].parse()?; i += 2; }
            "--window-s" => { window_s = args[i+1].parse()?; i += 2; }
            "--hop-s" => { hop_s = args[i+1].parse()?; i += 2; }
            "--model" => { model = PathBuf::from(&args[i+1]); i += 2; }
            "-h" | "--help" => { print!("{}", USAGE); return Ok(()); }
            s if s.starts_with('-') => bail!("unknown flag {s}"),
            s => { files.push(PathBuf::from(s)); i += 1; }
        }
    }
    if files.is_empty() { bail!("need at least one wav"); }
    init_ort()?;
    let mut sess = session(&model, threads)?;
    let mut results = Vec::new();
    for f in &files {
        eprintln!("[efficientat] {}", f.display());
        results.push(score_file(&mut sess, f, window_s, hop_s)?);
    }
    let doc = json!({
        "schema": "galion-efficientat/1",
        "model": {
            "id": "Chris-Kode/efficientat-mn10-as-onnx",
            "url": "https://huggingface.co/Chris-Kode/efficientat-mn10-as-onnx",
            "licence": "MIT",
            "attribution": "EfficientAT mn10_as © 2022 Florian Schmid; ONNX export Chris-Kode",
            "sha256": classes::EXPECTED_SHA256,
            "path": model,
        },
        "class_map": classes::state_map_json(),
        "files": results,
    });
    let text = serde_json::to_string_pretty(&doc)?;
    if let Some(p) = out {
        std::fs::write(&p, &text)?;
        println!("{}", p.display());
    } else {
        println!("{text}");
    }
    Ok(())
}

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if argv.is_empty() || argv[0] == "-h" || argv[0] == "--help" {
        print!("{}", USAGE);
        return;
    }
    let r = match argv[0].as_str() {
        "score" => cmd_score(&argv[1..]),
        "class-map" => {
            println!("{}", serde_json::to_string_pretty(&classes::state_map_json()).unwrap());
            Ok(())
        }
        other => Err(anyhow::anyhow!("unknown subcommand {other}")),
    };
    if let Err(e) = r {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use classes::*;

    #[test]
    fn class_indices_match_audioset_map() {
        assert_eq!(YELLING_INDICES, &[8, 11, 14, 13]); // Shout, Yell, Screaming, Children shouting
        assert_eq!(CRYING_INDICES, &[22]); // Crying, sobbing only
        assert_eq!(LAUGHTER_INDICES, &[16, 17, 18, 20, 21]);
        // names from embedded map
        let m = state_map_json();
        assert_eq!(m["states"]["yelling"]["indices"]["Shout"], 8);
        assert_eq!(m["states"]["crying"]["indices"]["Crying, sobbing"], 22);
        assert_eq!(m["states"]["laughter"]["indices"]["Laughter"], 16);
    }

    #[test]
    fn from_audioset_takes_max_of_family() {
        let mut p = vec![0.0f32; N_CLASSES];
        p[8] = 0.1;  // Shout
        p[14] = 0.7; // Screaming
        p[22] = 0.4; // Crying
        p[16] = 0.2; // Laughter
        p[18] = 0.55; // Giggle
        let s = StateScores::from_audioset(&p);
        assert!((s.yelling - 0.7).abs() < 1e-6);
        assert!((s.crying - 0.4).abs() < 1e-6);
        assert!((s.laughter - 0.55).abs() < 1e-6);
    }

    #[test]
    fn resample_shape_16k_to_32k() {
        let sr_in = 16000u32;
        let secs = 1.0f64;
        let x: Vec<f32> = (0..(sr_in as f64 * secs) as usize)
            .map(|i| (i as f32 * 440.0 * 2.0 * std::f32::consts::PI / sr_in as f32).sin())
            .collect();
        let y = audio::resample_linear(&x, sr_in, 32000);
        let expect = (32000.0 * secs).round() as usize;
        assert_eq!(y.len(), expect);
        // energy preserved roughly
        let e_in: f32 = x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32;
        let e_out: f32 = y.iter().map(|v| v * v).sum::<f32>() / y.len() as f32;
        assert!(e_out > 0.1 * e_in && e_out < 3.0 * e_in, "e_in={e_in} e_out={e_out}");
    }

    #[test]
    fn smoke_on_existing_render() {
        let wav = Path::new(
            "/workspace/swarm/voice-v2-sprint/cloning-v2/merge-v2/wav/samantha/t01.wav",
        );
        if !wav.exists() {
            eprintln!("skip smoke: missing {}", wav.display());
            return;
        }
        let model = Path::new(DEFAULT_MODEL);
        if !model.exists() {
            eprintln!("skip smoke: missing model");
            return;
        }
        init_ort().expect("ort");
        let mut sess = session(model, 2).expect("session");
        let v = score_file(&mut sess, wav, 2.5, 1.25).expect("score");
        assert_eq!(v["source"], "efficientat");
        assert!(v["states"]["yelling"]["max"].as_f64().unwrap() >= 0.0);
        assert!(v["states"]["crying"]["max"].as_f64().unwrap() >= 0.0);
        assert!(v["states"]["laughter"]["max"].as_f64().unwrap() >= 0.0);
        assert_eq!(v["states"]["yelling"]["source"], "efficientat");
    }
}
