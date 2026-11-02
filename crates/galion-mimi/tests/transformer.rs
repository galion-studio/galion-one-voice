// Galion Voice — Perpetual Open MIT License
// SPDX-License-Identifier: MIT
//! W20-15: real streaming Mimi transformer kernel, synthetic in-memory tensors only.
//! * streaming == whole-sequence batch reference (bits), incl. past the KV window;
//! * f64 oracle (independent maths: LayerNorm, MHA, interleaved/NeoX RoPE, erf GELU) within tol;
//! * convert-time interleaved→NeoX q/k permutation is exact up to float rounding;
//! * neutral weights (LayerScale 0) are the identity; reset restarts exactly; IS_STUB unchanged.

mod common;
use common::*;
use galion_mimi::transformer::{self, gelu_erf_inplace, layer_norm_into, permute_qk_interleaved_to_neox, reference, StreamingTransformer, LN_EPS, ROPE_THETA};
use galion_mimi::weights::{LoadError, WeightSource};
use galion_mimi::{tensor::FakeTensor, MimiConfig, IS_STUB};
use galion_tensor::Tensor;

fn cfg(context: usize, layers: usize) -> MimiConfig {
    MimiConfig { transformer_context: context, transformer_layers: layers, ..tiny_cfg() }
}

/// SyntheticWeights with LayerNorm weights around 1 and LayerScale ~0.1–0.5 (keeps it well-conditioned).
struct TfWeights;
impl WeightSource for TfWeights {
    fn tensor(&self, name: &str, shape: &[usize]) -> Result<Tensor, LoadError> {
        let t = SyntheticWeights.tensor(name, shape)?;
        let v = t.to_vec_f32();
        let v: Vec<f32> = if name.ends_with("norm1.weight") || name.ends_with("norm2.weight") {
            v.iter().map(|x| 1.0 + 0.2 * x).collect()
        } else if name.contains("layer_scale") {
            v.iter().map(|x| 0.3 + 0.2 * x).collect()
        } else {
            v
        };
        Ok(Tensor::from_vec(v, shape.to_vec()).unwrap())
    }
}

fn input(d: usize, steps: usize) -> Vec<f32> {
    test_pcm(d * steps).iter().map(|v| v * 3.0).collect()
}

fn run_stream(c: &MimiConfig, src: &dyn WeightSource, x: &[f32], steps: usize, chunk: usize) -> Vec<f32> {
    let d = c.latent_dim;
    let mut tf = StreamingTransformer::new(c, src, "enc_transformer").unwrap();
    let mut out = vec![0.0; d * steps];
    let mut t0 = 0;
    while t0 < steps {
        let n = chunk.min(steps - t0);
        let mut ft = FakeTensor::zeros(&[d, n]);
        for ch in 0..d {
            for t in 0..n {
                ft.data[ch * n + t] = x[ch * steps + t0 + t];
            }
        }
        tf.step(&mut ft).unwrap();
        for ch in 0..d {
            for t in 0..n {
                out[ch * steps + t0 + t] = ft.data[ch * n + t];
            }
        }
        t0 += n;
    }
    assert_eq!(tf.position(), steps);
    out
}

#[test]
fn streaming_bit_exact_vs_batch_reference_past_window() {
    for (context, layers, steps, chunk) in [(8, 1, 20, 2), (4, 2, 13, 1), (32, 2, 10, 3)] {
        let c = cfg(context, layers);
        let x = input(c.latent_dim, steps);
        let want = reference::forward(&c, &TfWeights, "enc_transformer", &x, steps).unwrap();
        let got = run_stream(&c, &TfWeights, &x, steps, chunk);
        assert!(got.iter().all(|v| v.is_finite()));
        assert_ne!(bits(&got), bits(&x), "transformer must change the signal");
        assert_eq!(bits(&got), bits(&want), "ctx={context} layers={layers} steps={steps} chunk={chunk}");
    }
}

// ---------- f64 oracle (independent of galion-tensor) ----------

fn erf64(x: f64) -> f64 {
    if x.abs() > 5.0 {
        return x.signum();
    }
    let (mut term, mut sum) = (x, x);
    for n in 1..120 {
        term *= -x * x / n as f64;
        sum += term / (2 * n + 1) as f64;
    }
    sum * 2.0 / std::f64::consts::PI.sqrt()
}

fn rope64(v: &mut [f64], pos: usize, interleaved: bool) {
    let hd = v.len();
    let half = hd / 2;
    for i in 0..half {
        let f = (ROPE_THETA as f64).powf(-2.0 * i as f64 / hd as f64);
        let (s, c) = (pos as f64 * f).sin_cos();
        let (ia, ib) = if interleaved { (2 * i, 2 * i + 1) } else { (i, i + half) };
        let (a, b) = (v[ia], v[ib]);
        v[ia] = a * c - b * s;
        v[ib] = a * s + b * c;
    }
}

/// `x` channel-major [d, steps]; weights read directly from `src`; RoPE interleaved or NeoX.
fn oracle(c: &MimiConfig, src: &dyn WeightSource, x: &[f32], steps: usize, interleaved: bool) -> Vec<f64> {
    let (d, f, nh, ctx) = (c.latent_dim, c.transformer_ffn, c.transformer_heads, c.transformer_context);
    let hd = d / nh;
    let mut xs: Vec<Vec<f64>> = (0..steps).map(|t| (0..d).map(|ch| x[ch * steps + t] as f64).collect()).collect();
    let g = |n: &str, s: &[usize]| -> Vec<f64> { src.tensor(n, s).unwrap().to_vec_f32().iter().map(|v| *v as f64).collect() };
    let ln = |x: &[f64], w: &[f64], b: &[f64]| -> Vec<f64> {
        let m = x.iter().sum::<f64>() / x.len() as f64;
        let var = x.iter().map(|v| (v - m) * (v - m)).sum::<f64>() / x.len() as f64;
        x.iter().enumerate().map(|(i, v)| (v - m) / (var + LN_EPS as f64).sqrt() * w[i] + b[i]).collect()
    };
    let mv = |w: &[f64], rows: usize, x: &[f64]| -> Vec<f64> { (0..rows).map(|r| (0..x.len()).map(|k| w[r * x.len() + k] * x[k]).sum()).collect() };
    for l in 0..c.transformer_layers {
        let p = format!("enc_transformer.layer{l}");
        let (n1w, n1b) = (g(&format!("{p}.norm1.weight"), &[d]), g(&format!("{p}.norm1.bias"), &[d]));
        let inp = g(&format!("{p}.attn.in_proj.weight"), &[3 * d, d]);
        let outp = g(&format!("{p}.attn.out_proj.weight"), &[d, d]);
        let ls1 = g(&format!("{p}.layer_scale1"), &[d]);
        let (n2w, n2b) = (g(&format!("{p}.norm2.weight"), &[d]), g(&format!("{p}.norm2.bias"), &[d]));
        let fc1 = g(&format!("{p}.mlp.fc1.weight"), &[f, d]);
        let fc2 = g(&format!("{p}.mlp.fc2.weight"), &[d, f]);
        let ls2 = g(&format!("{p}.layer_scale2"), &[d]);
        let qkv: Vec<Vec<f64>> = xs.iter().map(|xt| mv(&inp, 3 * d, &ln(xt, &n1w, &n1b))).collect();
        let mut q = vec![vec![0.0; d]; steps];
        let mut k = vec![vec![0.0; d]; steps];
        for t in 0..steps {
            for h in 0..nh {
                let mut qh = qkv[t][h * hd..(h + 1) * hd].to_vec();
                let mut kh = qkv[t][d + h * hd..d + (h + 1) * hd].to_vec();
                rope64(&mut qh, t, interleaved);
                rope64(&mut kh, t, interleaved);
                q[t][h * hd..(h + 1) * hd].copy_from_slice(&qh);
                k[t][h * hd..(h + 1) * hd].copy_from_slice(&kh);
            }
        }
        for t in 0..steps {
            let lo = (t + 1).saturating_sub(ctx);
            let mut att = vec![0.0; d];
            for h in 0..nh {
                let r = h * hd..(h + 1) * hd;
                let sc: Vec<f64> = (lo..=t).map(|j| q[t][r.clone()].iter().zip(&k[j][r.clone()]).map(|(a, b)| a * b).sum::<f64>() / (hd as f64).sqrt()).collect();
                let m = sc.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                let e: Vec<f64> = sc.iter().map(|s| (s - m).exp()).collect();
                let z: f64 = e.iter().sum();
                for (idx, j) in (lo..=t).enumerate() {
                    for dd in 0..hd {
                        att[h * hd + dd] += e[idx] / z * qkv[j][2 * d + h * hd + dd];
                    }
                }
            }
            let o = mv(&outp, d, &att);
            for i in 0..d {
                xs[t][i] += ls1[i] * o[i];
            }
            let mut hdn = mv(&fc1, f, &ln(&xs[t], &n2w, &n2b));
            hdn.iter_mut().for_each(|v| *v = 0.5 * *v * (1.0 + erf64(*v / 2f64.sqrt())));
            let o = mv(&fc2, d, &hdn);
            for i in 0..d {
                xs[t][i] += ls2[i] * o[i];
            }
        }
    }
    let mut out = vec![0.0; d * steps];
    for t in 0..steps {
        for ch in 0..d {
            out[ch * steps + t] = xs[t][ch];
        }
    }
    out
}

fn max_rel_err(got: &[f32], want: &[f64]) -> f64 {
    let scale = want.iter().fold(1.0f64, |m, v| m.max(v.abs()));
    got.iter().zip(want).map(|(g, w)| (*g as f64 - w).abs()).fold(0.0, f64::max) / scale
}

#[test]
fn streaming_matches_f64_oracle_neox() {
    let c = cfg(5, 2);
    let steps = 12;
    let x = input(c.latent_dim, steps);
    let got = run_stream(&c, &TfWeights, &x, steps, 2);
    let want = oracle(&c, &TfWeights, &x, steps, false);
    let e = max_rel_err(&got, &want);
    assert!(e < 1e-5, "max rel err {e}");
}

/// Interleaved-RoPE weights (upstream convention), permuted at "convert time" → NeoX kernel.
struct Permuted;
impl WeightSource for Permuted {
    fn tensor(&self, name: &str, shape: &[usize]) -> Result<Tensor, LoadError> {
        let t = TfWeights.tensor(name, shape)?;
        if !name.ends_with("attn.in_proj.weight") {
            return Ok(t);
        }
        let mut v = t.to_vec_f32();
        let d = shape[1];
        permute_qk_interleaved_to_neox(&mut v, d, tiny_cfg().transformer_heads);
        Ok(Tensor::from_vec(v, shape.to_vec()).unwrap())
    }
}

#[test]
fn interleaved_rope_weights_match_after_neox_permutation() {
    let c = cfg(6, 2);
    let steps = 10;
    let x = input(c.latent_dim, steps);
    let want = oracle(&c, &TfWeights, &x, steps, true);
    let got = run_stream(&c, &Permuted, &x, steps, 2);
    let e = max_rel_err(&got, &want);
    assert!(e < 1e-5, "permuted max rel err {e}");
    // Without the permutation the conventions disagree (guards against a vacuous test).
    let unpermuted = run_stream(&c, &TfWeights, &x, steps, 2);
    assert!(max_rel_err(&unpermuted, &want) > 1e-3);
}

#[test]
fn layer_scale_zero_is_identity_and_reset_restarts() {
    struct NoScale;
    impl WeightSource for NoScale {
        fn tensor(&self, n: &str, s: &[usize]) -> Result<Tensor, LoadError> {
            if n.contains("layer_scale") { Ok(Tensor::zeros(s.to_vec())) } else { TfWeights.tensor(n, s) }
        }
    }
    let c = cfg(4, 2);
    let steps = 7;
    let x = input(c.latent_dim, steps);
    assert_eq!(bits(&run_stream(&c, &NoScale, &x, steps, 3)), bits(&x));

    let d = c.latent_dim;
    let mut tf = StreamingTransformer::new(&c, &TfWeights, "enc_transformer").unwrap();
    let mut a = FakeTensor { shape: vec![d, steps], data: x.clone() };
    tf.step(&mut a).unwrap();
    tf.reset();
    assert_eq!(tf.position(), 0);
    let mut b = FakeTensor { shape: vec![d, steps], data: x.clone() };
    tf.step(&mut b).unwrap();
    assert_eq!(bits(&a.data), bits(&b.data));
    let mut bad = FakeTensor::zeros(&[d + 1, 1]);
    assert!(tf.step(&mut bad).is_err());
    assert!(IS_STUB, "synthetic tensors never flip IS_STUB");
}

#[test]
fn layer_norm_and_gelu_primitives() {
    let x: Vec<f32> = (0..16).map(|i| i as f32 * 0.7 - 3.0).collect();
    let (w, b) = (vec![1.0; 16], vec![0.0; 16]);
    let mut y = vec![0.0; 16];
    layer_norm_into(&x, &w, &b, LN_EPS, &mut y);
    let m: f32 = y.iter().sum::<f32>() / 16.0;
    let v: f32 = y.iter().map(|a| (a - m) * (a - m)).sum::<f32>() / 16.0;
    assert!(m.abs() < 1e-5 && (v - 1.0).abs() < 1e-3, "mean {m} var {v}");
    let mut g = vec![-10.0f32, -1.0, 0.0, 1.0, 3.0, 10.0];
    gelu_erf_inplace(&mut g);
    // Exact-erf GELU values (Φ(1)=0.841344746, Φ(3)=0.998650102).
    let want = [0.0f32, -0.158_655_25, 0.0, 0.841_344_75, 2.995_950_3, 10.0];
    for (a, e) in g.iter().zip(want) {
        assert!((a - e).abs() < 1e-6, "{a} vs {e}");
    }
    let e = transformer::erf(0.5);
    assert!((e - 0.520_499_9).abs() < 5e-7, "erf(0.5) = {e}");
}

#[test]
fn default_config_transformer_shapes_on_zero_weights() {
    // Full-width contract (512-d, 8 heads, ffn 2048, ctx 250) builds and runs on zeros; zeros in → zeros out.
    let c = MimiConfig { transformer_layers: 1, ..MimiConfig::default() };
    let mut tf = StreamingTransformer::new(&c, &galion_mimi::weights::ZeroWeights, "dec_transformer").unwrap();
    let mut x = FakeTensor::zeros(&[512, 2]);
    tf.step(&mut x).unwrap();
    assert!(x.is_all_zero());
    assert_eq!(tf.shapes.kv_floats_per_layer(), 2 * 250 * 512);
}
