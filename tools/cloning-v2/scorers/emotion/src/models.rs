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

/// Where a session runs. `Auto` tries the CUDA execution provider first and
/// falls back to the CPU provider; `Cpu` (the default) never touches a GPU.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Device {
    Auto,
    Cpu,
    Cuda,
}

impl std::str::FromStr for Device {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        match s {
            "auto" => Ok(Device::Auto),
            "cpu" => Ok(Device::Cpu),
            "cuda" | "gpu" => Ok(Device::Cuda),
            _ => anyhow::bail!("--device must be auto, cpu, or cuda (got {s:?})"),
        }
    }
}

/// `0` means every logical CPU the OS gives this process.
pub fn resolve_threads(threads: usize) -> usize {
    if threads > 0 {
        return threads;
    }
    std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1)
}

fn builder(threads: usize) -> Result<ort::session::builder::SessionBuilder> {
    Ok(Session::builder()?
        .with_optimization_level(GraphOptimizationLevel::Level3)?
        .with_intra_threads(threads)?
        .with_inter_threads(1)?)
}

/// Open a model on the requested device. Returns the session and a plain
/// description of the device it actually runs on (logged in every report).
pub fn session(path: &str, threads: usize, device: Device) -> Result<(Session, String)> {
    let threads = resolve_threads(threads);
    if device != Device::Cpu {
        use ort::execution_providers::CUDAExecutionProvider;
        let cuda = builder(threads)?
            .with_execution_providers([CUDAExecutionProvider::default().with_device_id(0).build().error_on_failure()])
            .and_then(|b| b.commit_from_file(path));
        match cuda {
            Ok(s) => return Ok((s, "cuda:0 (onnxruntime CUDAExecutionProvider)".into())),
            Err(e) if device == Device::Cuda => {
                return Err(anyhow::anyhow!("CUDA requested but unavailable for {path}: {e}"));
            }
            Err(e) => {
                let s = builder(threads)?.commit_from_file(path).with_context(|| format!("load model {path}"))?;
                return Ok((s, format!("cpu ({threads} threads; CUDA fallback: {e})")));
            }
        }
    }
    let s = builder(threads)?.commit_from_file(path).with_context(|| format!("load model {path}"))?;
    Ok((s, format!("cpu ({threads} threads)")))
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

/// Linear classification head published with a frame-feature model
/// (emotion2vec ONNX export format: `{"labels": [...], "weight": [[...]], "bias": [...]}`).
/// Usage follows the publisher's README: mean-pool frames, `W·x + b`, softmax.
#[derive(serde::Deserialize, Clone)]
pub struct Head {
    pub labels: Vec<String>,
    pub weight: Vec<Vec<f32>>,
    pub bias: Vec<f32>,
}

pub fn load_head(path: &str) -> Result<Head> {
    let h: Head = serde_json::from_str(&std::fs::read_to_string(path).with_context(|| format!("read head {path}"))?)
        .with_context(|| format!("parse head {path}"))?;
    anyhow::ensure!(h.labels.len() == h.weight.len() && h.labels.len() == h.bias.len(), "head {path}: labels/weight/bias length mismatch");
    Ok(h)
}

fn softmax(v: &[f32]) -> Vec<f32> {
    let m = v.iter().cloned().fold(f32::MIN, f32::max);
    let e: Vec<f32> = v.iter().map(|x| (x - m).exp()).collect();
    let s: f32 = e.iter().sum();
    e.into_iter().map(|x| x / s).collect()
}

/// Frame features `[1, T, D]` -> mean over T (the publisher's pooled utterance embedding).
pub fn pooled_features(sess: &mut Session, x16: &[f32]) -> Result<Vec<f32>> {
    let t = Tensor::from_array(([1usize, x16.len()], x16.to_vec()))?;
    let out = sess.run(ort::inputs![t])?;
    let (shape, data) = out[0].try_extract_tensor::<f32>()?;
    let d = *shape.last().context("feature output has no dims")? as usize;
    let frames = data.len() / d.max(1);
    anyhow::ensure!(frames > 0, "feature output is empty");
    let mut pooled = vec![0f32; d];
    for f in data.chunks(d) {
        for (p, v) in pooled.iter_mut().zip(f) {
            *p += v / frames as f32;
        }
    }
    Ok(pooled)
}

/// Head on a pooled embedding -> softmax probs in `head.labels` order.
pub fn apply_head(head: &Head, pooled: &[f32]) -> Result<Vec<f32>> {
    anyhow::ensure!(pooled.len() == head.weight[0].len(), "feature dim {} does not match head dim {}", pooled.len(), head.weight[0].len());
    let logits: Vec<f32> = head.weight.iter().zip(&head.bias).map(|(w, b)| w.iter().zip(pooled).map(|(a, x)| a * x).sum::<f32>() + b).collect();
    Ok(softmax(&logits))
}

pub fn ser_with_head(sess: &mut Session, x16: &[f32], head: &Head) -> Result<Vec<f32>> {
    let pooled = pooled_features(sess, x16)?;
    apply_head(head, &pooled)
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
