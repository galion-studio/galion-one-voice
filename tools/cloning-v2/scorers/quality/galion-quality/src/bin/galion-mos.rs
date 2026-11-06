//! galion-mos: torchaudio SQUIM (subjective MOS + objective STOI/PESQ/SI-SDR) via ONNX Runtime.
//! Same preprocessing as torchaudio: int PCM/2^15, channel mean, torchaudio sinc resample to 16 kHz,
//! NMR tiled/cut to the clip length (SquimSubjective._align_shapes).
use galion_quality::{audio, cli::{self, r, Args}, resample::Resampler};
use ort::session::{builder::GraphOptimizationLevel, Session};
use ort::value::Tensor;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::time::Instant;

const USAGE: &str = "usage: galion-mos [--nmr REF.wav] [--no-subjective] [--no-objective] [--threads N<=2] \
[--models DIR] [--ort-lib libonnxruntime.so] [--out out.json] clip.wav...\n\
  --nmr: non-matching clean reference for SQUIM subjective (convention: the OTHER persona's reference.wav)";

fn load16(p: &Path) -> Result<Vec<f32>, String> {
    let w = audio::load(p)?;
    let m = w.mono();
    Ok(if w.sr == 16000 { m } else { Resampler::new(w.sr, 16000).process(&m) })
}

fn session(env: &ort::environment::Environment, path: &Path, threads: usize) -> Result<Session, String> {
    let b = Session::builder(env).map_err(|e| e.to_string())?;
    let b = b.with_optimization_level(GraphOptimizationLevel::Level3).map_err(|e| e.to_string())?;
    let b = b.with_intra_threads(threads).map_err(|e| e.to_string())?;
    let b = b.with_inter_threads(1).map_err(|e| e.to_string())?;
    // No arena / no memory pattern: SQUIM objective attention is O(T^2); the arena would keep the peak block resident.
    let b = b.with_memory_pattern(false).map_err(|e| e.to_string())?;
    let mut b = b.with_execution_providers([ort::ep::CPU::default().with_arena_allocator(false).build()]).map_err(|e| e.to_string())?;
    b.commit_from_file(path).map_err(|e| format!("{}: {e}", path.display()))
}

fn main() {
    let a = Args::parse(&["no-subjective", "no-objective", "help"]);
    if a.has("help") || a.pos.is_empty() { cli::die(USAGE) }
    let here = std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf())).unwrap_or_default();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let models = a.get("models").map(PathBuf::from).unwrap_or_else(|| {
        let c = here.join("../models"); if c.join("squim_subjective.onnx").exists() { c } else { root.join("models") }
    });
    let ort_lib = a.get("ort-lib").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(
        "/workspace/swarm/voice-v2-sprint/repo/weights/runtime/ort/onnxruntime-linux-x64-1.31.1/lib/libonnxruntime.so.1.31.1"));
    let threads: usize = a.get("threads").and_then(|s| s.parse().ok()).unwrap_or(2).clamp(1, 2);
    let do_sub = !a.has("no-subjective");
    let do_obj = !a.has("no-objective");
    let nmr_path = a.get("nmr").map(PathBuf::from);
    if do_sub && nmr_path.is_none() { cli::die("galion-mos: --nmr REF.wav is required for SQUIM subjective (or pass --no-subjective)") }

    let t0 = Instant::now();
    let env = ort::init_from(&ort_lib).unwrap_or_else(|e| cli::die(&format!("load onnxruntime {}: {e}", ort_lib.display())))
        .with_name("galion-mos").build().unwrap_or_else(|e| cli::die(&format!("ort env: {e}")));
    let mut sub = if do_sub { Some(session(&env, &models.join("squim_subjective.onnx"), threads).unwrap_or_else(|e| cli::die(&e))) } else { None };
    let mut obj = if do_obj { Some(session(&env, &models.join("squim_objective.onnx"), threads).unwrap_or_else(|e| cli::die(&e))) } else { None };
    let nmr = nmr_path.as_ref().map(|p| load16(p).unwrap_or_else(|e| cli::die(&e)));
    let load_s = t0.elapsed().as_secs_f64();

    let mut rows = vec![]; let (mut moss, mut stois, mut pesqs, mut sdrs) = (vec![], vec![], vec![], vec![]);
    let t1 = Instant::now();
    for f in &a.pos {
        let tf = Instant::now();
        let w = match load16(Path::new(f)) { Ok(w) => w, Err(e) => { rows.push(json!({"path": f, "error": e})); continue } };
        let n = w.len();
        let mut row = json!({"path": f, "duration_s": r(n as f64 / 16000.0, 3)});
        if let (Some(s), Some(nm)) = (sub.as_mut(), nmr.as_ref()) {
            let refa: Vec<f32> = nm.iter().cycle().take(n).cloned().collect();
            let res = (|| -> Result<f64, String> {
                let x = Tensor::from_array(([1usize, n], w.clone())).map_err(|e| e.to_string())?;
                let y = Tensor::from_array(([1usize, n], refa)).map_err(|e| e.to_string())?;
                let o = s.run(ort::inputs!["waveform" => x, "reference" => y]).map_err(|e| e.to_string())?;
                let (_, v) = o[0].try_extract_tensor::<f32>().map_err(|e| e.to_string())?;
                Ok(v[0] as f64)
            })();
            match res { Ok(m) => { moss.push(m); row["mos"] = r(m, 4) } Err(e) => row["mos_error"] = json!(e) }
        }
        if let Some(s) = obj.as_mut() {
            let res = (|| -> Result<Vec<f64>, String> {
                let x = Tensor::from_array(([1usize, n], w)).map_err(|e| e.to_string())?;
                let o = s.run(ort::inputs!["waveform" => x]).map_err(|e| e.to_string())?;
                let (_, v) = o[0].try_extract_tensor::<f32>().map_err(|e| e.to_string())?;
                Ok(v.iter().map(|&x| x as f64).collect())
            })();
            match res {
                Ok(v) => { stois.push(v[0]); pesqs.push(v[1]); sdrs.push(v[2]);
                    row["stoi"] = r(v[0], 4); row["pesq"] = r(v[1], 4); row["si_sdr"] = r(v[2], 4) }
                Err(e) => row["objective_error"] = json!(e),
            }
        }
        row["ms"] = r(tf.elapsed().as_secs_f64() * 1e3, 1);
        rows.push(row);
    }
    let agg = |v: &[f64]| { let (m, sd) = cli::mean_sd(v); json!({"n": v.len(), "mean": r(m, 4), "sd": r(sd, 4)}) };
    let out = json!({
        "tool": "galion-mos", "metric": "torchaudio SQUIM_SUBJECTIVE (NORESQA-MOS) + SQUIM_OBJECTIVE, ONNX fp32",
        "nmr": nmr_path.map(|p| p.display().to_string()), "threads": threads,
        "files": rows,
        "summary": {"mos": agg(&moss), "stoi": agg(&stois), "pesq": agg(&pesqs), "si_sdr": agg(&sdrs)},
        "timing": {"load_s": r(load_s, 3), "score_s": r(t1.elapsed().as_secs_f64(), 3), "peak_rss_mb": peak_rss_mb()}
    });
    let s = serde_json::to_string_pretty(&out).unwrap();
    match a.get("out") { Some(p) => std::fs::write(p, s + "\n").unwrap_or_else(|e| cli::die(&e.to_string())), None => println!("{s}") }
}

fn peak_rss_mb() -> serde_json::Value {
    std::fs::read_to_string("/proc/self/status").ok().and_then(|s| s.lines().find(|l| l.starts_with("VmHWM:"))
        .and_then(|l| l.split_whitespace().nth(1)).and_then(|v| v.parse::<f64>().ok())).map(|kb| r(kb / 1024.0, 1)).unwrap_or(serde_json::Value::Null)
}
