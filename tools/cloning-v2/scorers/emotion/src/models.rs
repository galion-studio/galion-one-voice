//! ONNX models via ort: categorical SER (superb/wav2vec2-base-superb-er) and
//! dimensional A/D/V regression (audeering/wav2vec2-large-robust-12-ft-emotion-msp-dim).
use anyhow::{Context, Result};
use ort::session::{builder::GraphOptimizationLevel, Session};
use ort::value::Tensor;

pub const SER_LABELS: [&str; 4] = ["neu", "hap", "ang", "sad"];
/// Max chunk fed to a model at once (16 kHz samples). Longer audio is split and
/// combined with a length-weighted mean.
const MAX_CHUNK: usize = 16000 * 30;

/// Default ONNX Runtime shared library (1.31.1, already on the box). Override with ORT_DYLIB_PATH or --ort-lib.
pub const DEFAULT_ORT_LIB: &str = "/workspace/swarm/voice-v2-sprint/repo/weights/runtime/ort/onnxruntime-linux-x64-1.31.1/lib/libonnxruntime.so";

pub fn init_ort(lib: Option<&str>) -> Result<()> {
    let p = lib.map(|s| s.to_string()).or_else(|| std::env::var("ORT_DYLIB_PATH").ok()).unwrap_or_else(|| DEFAULT_ORT_LIB.to_string());
    ort::init_from(p.clone()).with_name("galion-emotion").commit().with_context(|| format!("init onnxruntime from {p}"))?;
    Ok(())
}

pub fn session(path: &str, threads: usize) -> Result<Session> {
    let s = Session::builder()?
        .with_optimization_level(GraphOptimizationLevel::Level3)?
        .with_intra_threads(threads)?
        .with_inter_threads(1)?
        .commit_from_file(path)
        .with_context(|| format!("load model {path}"))?;
    Ok(s)
}

fn run_vec(sess: &mut Session, x: &[f32]) -> Result<Vec<f32>> {
    let t = Tensor::from_array(([1usize, x.len()], x.to_vec()))?;
    let out = sess.run(ort::inputs![t])?;
    let (_, data) = out[0].try_extract_tensor::<f32>()?;
    Ok(data.to_vec())
}

fn chunked(sess: &mut Session, x: &[f32], prep: impl Fn(&[f32]) -> Vec<f32>) -> Result<Vec<f32>> {
    let n_chunks = x.len().div_ceil(MAX_CHUNK).max(1);
    let len = x.len().div_ceil(n_chunks);
    let mut acc: Vec<f32> = Vec::new();
    let mut wsum = 0f32;
    for c in x.chunks(len.max(1)) {
        if c.len() < 1600 && !acc.is_empty() {
            continue; // skip <0.1 s tail
        }
        let v = run_vec(sess, &prep(c))?;
        let w = c.len() as f32;
        if acc.is_empty() {
            acc = vec![0.0; v.len()];
        }
        for (a, b) in acc.iter_mut().zip(v) {
            *a += w * b;
        }
        wsum += w;
    }
    for a in acc.iter_mut() {
        *a /= wsum;
    }
    Ok(acc)
}

/// Returns softmax probs in SER_LABELS order. Input: raw 16 kHz waveform (model expects do_normalize=false).
pub fn ser(sess: &mut Session, x16: &[f32]) -> Result<Vec<f32>> {
    chunked(sess, x16, |c| c.to_vec())
}

/// Returns (arousal, dominance, valence), roughly 0..1. Input normalized zero-mean/unit-var like Wav2Vec2FeatureExtractor.
pub fn adv(sess: &mut Session, x16: &[f32]) -> Result<[f32; 3]> {
    let v = chunked(sess, x16, |c| {
        let n = c.len() as f64;
        let mean = c.iter().map(|&v| v as f64).sum::<f64>() / n;
        let var = c.iter().map(|&v| (v as f64 - mean).powi(2)).sum::<f64>() / n;
        let inv = 1.0 / (var + 1e-7).sqrt();
        c.iter().map(|&v| ((v as f64 - mean) * inv) as f32).collect()
    })?;
    Ok([v[0], v[1], v[2]])
}
