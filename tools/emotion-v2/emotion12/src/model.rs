//! emotion2vec+ through ONNX Runtime (ort). Two pinned public exports:
//!  base  ziyu12345/emotion2vec_plus_base_onnx  -> frame features [1,T,768];
//!        pooled mean + the published head (emotion2vec_head.json) = 9-class
//!        probs, pooled mean = embedding. Same recipe as
//!        tools/cloning-v2/scorers/universal/py/score_universal.py.
//!  large Voldorn/emotion2vec-plus-large-onnx    -> probs [1,9] (FunASR
//!        utterance inference baked into the graph). No embedding output, so
//!        the base embedding is used next to it.
//! GPU: CUDA execution provider when present, else CPU (production default).
use anyhow::{bail, Context, Result};
use ort::execution_providers::{CUDAExecutionProvider, ExecutionProvider};
use ort::session::{builder::GraphOptimizationLevel, Session};
use ort::value::Tensor;
use serde::Deserialize;
use std::path::Path;

pub const LABELS: [&str; 9] =
    ["angry", "disgusted", "fearful", "happy", "neutral", "other", "sad", "surprised", "unknown"];
const MAX_CHUNK: usize = 16000 * 30;

#[derive(Deserialize)]
struct Head {
    labels: Vec<String>,
    weight: Vec<Vec<f32>>,
    bias: Vec<f32>,
}

pub struct Scorer {
    base: Session,
    large: Option<Session>,
    head: Head,
    pub device: String,
    pub model: String,
    /// FunASR `normalize: true` (emotion2vec+ config.yaml): F.layer_norm over
    /// the whole waveform before the encoder. Applied to the base export only;
    /// the large export already does it inside its graph.
    pub normalize: bool,
}

pub struct Output {
    pub labels: Vec<String>,
    pub probs: Vec<f32>,
    pub embedding: Vec<f32>,
}

pub fn init_ort() -> Result<()> {
    let lib = std::env::var("ORT_DYLIB_PATH")
        .context("ORT_DYLIB_PATH must point at onnxruntime.dll / libonnxruntime.so (1.22)")?;
    ort::init_from(lib.clone()).with_name("galion-emotion12").commit().with_context(|| format!("init onnxruntime from {lib}"))?;
    Ok(())
}

fn session(path: &Path, threads: usize, device: &str) -> Result<(Session, String)> {
    let mut b = Session::builder()?
        .with_optimization_level(GraphOptimizationLevel::Level3)?
        .with_intra_threads(threads)?
        .with_inter_threads(1)?;
    let mut used = "cpu".to_string();
    if device != "cpu" {
        // Heuristic cuDNN algo pick: clip lengths vary, so the default
        // exhaustive search would re-run for every new input shape.
        let cuda = CUDAExecutionProvider::default()
            .with_conv_algorithm_search(ort::execution_providers::cuda::CuDNNConvAlgorithmSearch::Heuristic);
        if cuda.is_available().unwrap_or(false) {
            b = b.with_execution_providers([cuda.build().error_on_failure()])?;
            used = "cuda".into();
        } else if device == "cuda" {
            bail!("--device cuda requested but the CUDA execution provider is not available");
        }
    }
    let s = b.commit_from_file(path).with_context(|| format!("load model {}", path.display()))?;
    Ok((s, used))
}

impl Scorer {
    pub fn new(models_dir: &Path, model: &str, threads: usize, device: &str) -> Result<Self> {
        let bdir = models_dir.join("emotion2vec_plus_base_onnx");
        let (base, used) = session(&bdir.join("emotion2vec_plus_base.onnx"), threads, device)?;
        let head: Head = serde_json::from_str(&std::fs::read_to_string(bdir.join("emotion2vec_head.json"))?)?;
        if head.labels != LABELS || head.weight.len() != 9 || head.bias.len() != 9 {
            bail!("unexpected emotion2vec head layout");
        }
        let large = match model {
            "base" => None,
            "large" => Some(session(&models_dir.join("emotion2vec_plus_large_onnx/emotion2vec_plus_large.onnx"), threads, device)?.0),
            other => bail!("unknown model {other:?}; expected base or large"),
        };
        Ok(Self { base, large, head, device: used, model: model.into(), normalize: true })
    }

    fn base_pooled(&mut self, x: &[f32]) -> Result<Vec<f32>> {
        // length-weighted mean of frame features over <=30 s chunks
        let n = x.len().div_ceil(MAX_CHUNK).max(1);
        let len = x.len().div_ceil(n);
        let mut acc: Vec<f64> = Vec::new();
        let mut frames = 0usize;
        for chunk in x.chunks(len.max(1)) {
            let t = Tensor::from_array(([1usize, chunk.len()], chunk.to_vec()))?;
            let out = self.base.run(ort::inputs![t])?;
            let (shape, data) = out[0].try_extract_tensor::<f32>()?;
            let (t_len, dim) = (shape[1] as usize, shape[2] as usize);
            if acc.is_empty() {
                acc = vec![0.0; dim];
            }
            for f in 0..t_len {
                for d in 0..dim {
                    acc[d] += data[f * dim + d] as f64;
                }
            }
            frames += t_len;
        }
        Ok(acc.into_iter().map(|v| (v / frames.max(1) as f64) as f32).collect())
    }

    pub fn score(&mut self, x16: &[f32]) -> Result<Output> {
        if x16.len() < 1600 {
            bail!("clip shorter than 0.1 s");
        }
        let pooled = if self.normalize {
            let n = x16.len() as f64;
            let mean = x16.iter().map(|v| *v as f64).sum::<f64>() / n;
            let var = x16.iter().map(|v| (*v as f64 - mean).powi(2)).sum::<f64>() / n;
            let inv = 1.0 / (var + 1e-5).sqrt();
            let normed: Vec<f32> = x16.iter().map(|v| ((*v as f64 - mean) * inv) as f32).collect();
            self.base_pooled(&normed)?
        } else {
            self.base_pooled(x16)?
        };
        let probs = if let Some(large) = self.large.as_mut() {
            let t = Tensor::from_array(([1usize, x16.len()], x16.to_vec()))?;
            let out = large.run(ort::inputs![t])?;
            let (_, data) = out[0].try_extract_tensor::<f32>()?;
            data.to_vec()
        } else {
            let logits: Vec<f32> = self
                .head
                .weight
                .iter()
                .zip(&self.head.bias)
                .map(|(w, b)| w.iter().zip(&pooled).map(|(a, c)| a * c).sum::<f32>() + b)
                .collect();
            let m = logits.iter().cloned().fold(f32::MIN, f32::max);
            let e: Vec<f32> = logits.iter().map(|v| (v - m).exp()).collect();
            let s: f32 = e.iter().sum();
            e.into_iter().map(|v| v / s).collect()
        };
        Ok(Output { labels: LABELS.iter().map(|s| s.to_string()).collect(), probs, embedding: pooled })
    }
}
