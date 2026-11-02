// Galion Voice — Perpetual Open MIT License
// SPDX-License-Identifier: MIT
//! A6 wave 2: the streaming frame path (A5 kernels) has the right output lengths and is
//! bit-identical to the whole-signal batch reference (`seanet::reference`, galion-tensor ops).
//! Synthetic in-memory weights only (tests/common), zero weights for the silence contract.

mod common;
use common::*;
use galion_mimi::seanet::{reference, resample_convs, SeanetDecoder, SeanetEncoder};
use galion_mimi::transformer::reference as tf_reference;
use galion_mimi::weights::{self, LoadError, WeightManifest, WeightSource, ZeroWeights};
use galion_mimi::{MimiCodec, MimiConfig, IS_STUB};
use galion_tensor::Tensor;

const FRAMES: usize = 6;

#[test]
fn encoder_stream_lengths_and_bits_match_batch() {
    let cfg = tiny_cfg();
    let n = cfg.frame_samples();
    assert_eq!(n, 1920);
    let mut enc = SeanetEncoder::new(&cfg, &SyntheticWeights, n).unwrap();
    assert_eq!(enc.frame(), 1920);
    assert_eq!(enc.out_steps(), 2, "1920 samples / hop 960 = 2 latent steps (25 Hz)");
    // Per-stage lengths: 1920 → 480 → 96 → 16 → 2.
    let lens: Vec<usize> = enc.blocks.iter().map(|(_, d)| d.n_out).collect();
    assert_eq!(lens, vec![480, 96, 16, 2]);

    let pcm = test_pcm(FRAMES * n);
    let (c, s) = (cfg.latent_dim, enc.out_steps());
    let mut streamed = vec![0.0; c * s * FRAMES];
    let mut out = vec![0.0; c * s];
    for (f, frame) in pcm.chunks_exact(n).enumerate() {
        enc.forward(frame, &mut out).unwrap();
        put_frame(&mut streamed, &out, c, s, f, FRAMES);
    }
    let batch = reference::encode(&cfg, &SyntheticWeights, &pcm).unwrap();
    assert_eq!(batch.len(), c * s * FRAMES);
    assert!(batch.iter().any(|v| *v != 0.0), "synthetic weights must produce signal");
    assert_eq!(bits(&streamed), bits(&batch), "streaming encoder != batch reference");
}

#[test]
fn decoder_stream_lengths_and_bits_match_batch() {
    let cfg = tiny_cfg();
    let (c, s) = (cfg.latent_dim, cfg.frame_downsample);
    let mut dec = SeanetDecoder::new(&cfg, &SyntheticWeights, s).unwrap();
    assert_eq!(dec.out_samples(), 1920, "2 latent steps → 1920 samples");
    let lens: Vec<usize> = dec.blocks.iter().map(|(u, _)| u.n_out).collect();
    assert_eq!(lens, vec![16, 96, 480, 1920]);

    let latent: Vec<f32> = test_pcm(c * s * FRAMES).iter().map(|v| v * 2.0).collect();
    let mut streamed = vec![0.0; 1920 * FRAMES];
    for f in 0..FRAMES {
        let fr = get_frame(&latent, c, s, f, FRAMES);
        dec.forward(&fr, &mut streamed[f * 1920..(f + 1) * 1920]).unwrap();
    }
    let batch = reference::decode(&cfg, &SyntheticWeights, &latent, s * FRAMES).unwrap();
    assert_eq!(batch.len(), 1920 * FRAMES);
    assert!(batch.iter().any(|v| *v != 0.0));
    assert_eq!(bits(&streamed), bits(&batch), "streaming decoder != batch reference");
}

/// Whole chain at codec level: pcm → SEANet → transformer(enc) → downsample → 12.5 Hz latent,
/// and 12.5 Hz latent → depthwise upsample → transformer(dec) → SEANet dec → pcm (W20-15: real
/// streaming transformers, compared with the whole-sequence `transformer::reference`).
#[test]
fn codec_conv_chain_matches_batch_reference() {
    let cfg = tiny_cfg();
    let d = cfg.latent_dim;
    let mut m = MimiCodec::with_weight_source(cfg.clone(), &SyntheticWeights).unwrap();
    assert!(m.has_kernels() && IS_STUB);
    let pcm = test_pcm(FRAMES * 1920);
    let mut lat12 = vec![0.0; d * FRAMES];
    let mut one = vec![0.0; d];
    for (f, fr) in pcm.chunks_exact(1920).enumerate() {
        m.encode_latent_frame(fr, &mut one).unwrap();
        put_frame(&mut lat12, &one, d, 1, f, FRAMES);
    }
    let [down, up] = resample_convs(&cfg);
    let lat25 = reference::encode(&cfg, &SyntheticWeights, &pcm).unwrap();
    let lat25 = tf_reference::forward(&cfg, &SyntheticWeights, "enc_transformer", &lat25, 2 * FRAMES).unwrap();
    let (lat12_ref, steps) = reference::conv(&down, &SyntheticWeights, &lat25, 2 * FRAMES).unwrap();
    assert_eq!(steps, FRAMES, "one 12.5 Hz latent per 80 ms frame");
    assert_eq!(bits(&lat12), bits(&lat12_ref));

    let mut out = vec![0.0; 1920 * FRAMES];
    for f in 0..FRAMES {
        let fr = get_frame(&lat12, d, 1, f, FRAMES);
        m.decode_latent_frame(&fr, &mut out[f * 1920..(f + 1) * 1920]).unwrap();
    }
    let (up_ref, up_len) = reference::conv(&up, &SyntheticWeights, &lat12_ref, FRAMES).unwrap();
    assert_eq!(up_len, 2 * FRAMES);
    let up_ref = tf_reference::forward(&cfg, &SyntheticWeights, "dec_transformer", &up_ref, up_len).unwrap();
    let pcm_ref = reference::decode(&cfg, &SyntheticWeights, &up_ref, up_len).unwrap();
    assert_eq!(pcm_ref.len(), 1920 * FRAMES);
    assert_eq!(bits(&out), bits(&pcm_ref));
}

#[test]
fn codes_path_with_kernels_is_deterministic_and_resets() {
    let cfg = tiny_cfg();
    let mut m = MimiCodec::with_weight_source(cfg.clone(), &SyntheticWeights).unwrap();
    let pcm = test_pcm(4 * 1920);
    let run = |m: &mut MimiCodec| {
        let mut codes = vec![0u16; cfg.n_codebooks];
        let mut all = Vec::new();
        let mut out = vec![0.0; 1920];
        let mut audio = Vec::new();
        for fr in pcm.chunks_exact(1920) {
            m.encode_frame(fr, &mut codes).unwrap();
            assert!(codes.iter().all(|&c| (c as usize) < cfg.codebook_size));
            all.extend_from_slice(&codes);
            m.decode_frame(&codes, &mut out).unwrap();
            audio.extend(bits(&out));
        }
        (all, audio)
    };
    let a = run(&mut m);
    assert!(a.0.iter().any(|&c| c != 0), "synthetic codebooks should pick non-zero codes");
    m.reset();
    assert_eq!(run(&mut m), a, "reset must restart the stream exactly");
    let mut fresh = MimiCodec::with_weight_source(cfg.clone(), &SyntheticWeights).unwrap();
    assert_eq!(run(&mut fresh), a);
}

#[test]
fn zero_weights_real_kernels_give_zero_codes_and_silence() {
    let cfg = tiny_cfg();
    let mut m = MimiCodec::with_weight_source(cfg.clone(), &ZeroWeights).unwrap();
    assert!(m.has_kernels());
    assert!(IS_STUB, "kernels without cleared weights never flip IS_STUB");
    let mut codes = vec![u16::MAX; cfg.n_codebooks];
    let mut out = vec![1.0f32; 1920];
    for fr in test_pcm(3 * 1920).chunks_exact(1920) {
        m.encode_frame(fr, &mut codes).unwrap();
        assert!(codes.iter().all(|&c| c == 0));
        m.decode_frame(&codes, &mut out).unwrap();
        assert_eq!(out.len(), 1920);
        assert!(out.iter().all(|&s| s == 0.0));
    }
    // Same contract as the kernel-free default path.
    let mut plain = MimiCodec::new(cfg.clone()).unwrap();
    assert!(!plain.has_kernels());
    let mut out2 = vec![1.0f32; 1920];
    plain.decode_frame(&codes, &mut out2).unwrap();
    assert_eq!(bits(&out), bits(&out2));
}

#[test]
fn load_codec_refuses_even_with_valid_manifest() {
    let cfg = MimiConfig::default();
    let m = WeightManifest {
        licence: Some("CC-BY-4.0".into()),
        upstream_url: Some("upstream (placeholder, never fetched)".into()),
        file_sha256: Some("0".repeat(64)),
        licence_cleared_by_a21: true,
    };
    assert_eq!(weights::load_codec(&cfg, &m).err(), Some(LoadError::NotImplementedStub));
    assert_eq!(weights::load_codec(&cfg, &WeightManifest::default()).err(), Some(LoadError::LicenceMissing));
}

#[test]
fn weight_source_shape_mismatch_is_rejected() {
    struct Bad;
    impl WeightSource for Bad {
        fn tensor(&self, _n: &str, shape: &[usize]) -> Result<Tensor, LoadError> {
            let mut s = shape.to_vec();
            s[0] += 1;
            Ok(Tensor::zeros(s))
        }
    }
    struct Missing;
    impl WeightSource for Missing {
        fn tensor(&self, n: &str, _s: &[usize]) -> Result<Tensor, LoadError> {
            Err(LoadError::Missing(n.into()))
        }
    }
    assert!(matches!(MimiCodec::with_weight_source(tiny_cfg(), &Bad).err(), Some(LoadError::ShapeMismatch { .. })));
    assert_eq!(MimiCodec::with_weight_source(tiny_cfg(), &Missing).err(), Some(LoadError::Missing("encoder.conv_in.weight".into())));
}

#[test]
fn contract_covers_every_tensor_the_kernels_read() {
    use std::cell::RefCell;
    struct Spy(RefCell<Vec<(String, Vec<usize>)>>);
    impl WeightSource for Spy {
        fn tensor(&self, n: &str, s: &[usize]) -> Result<Tensor, LoadError> {
            self.0.borrow_mut().push((n.into(), s.to_vec()));
            Ok(Tensor::zeros(s.to_vec()))
        }
    }
    let cfg = tiny_cfg();
    let spy = Spy(RefCell::new(Vec::new()));
    MimiCodec::with_weight_source(cfg.clone(), &spy).unwrap();
    let contract = weights::expected_tensors(&cfg);
    for (n, s) in spy.0.borrow().iter() {
        assert!(contract.iter().any(|t| &t.name == n && &t.shape == s), "kernel reads {n} {s:?} not in contract");
    }
    assert!(contract.iter().any(|t| t.name == "upsample.weight" && t.shape == vec![16, 1, 4]), "depthwise upsample");
    assert!(contract.iter().any(|t| t.name == "downsample.weight" && t.shape == vec![16, 16, 4]));
}

/// Full-width default config on all-zero weights: real kernels, real lengths, silence.
/// Ignored by default (scalar debug build is slow at 64→1024 channels); run with
/// `cargo test -p galion-mimi --offline --release -- --ignored full_size`.
#[test]
#[ignore]
fn full_size_zero_weights_frame_lengths() {
    let cfg = MimiConfig::default();
    let mut m = MimiCodec::with_weight_source(cfg.clone(), &ZeroWeights).unwrap();
    let mut codes = vec![1u16; 8];
    let mut out = vec![1.0f32; 1920];
    let t = std::time::Instant::now();
    for fr in test_pcm(2 * 1920).chunks_exact(1920) {
        m.encode_frame(fr, &mut codes).unwrap();
        m.decode_frame(&codes, &mut out).unwrap();
    }
    println!("full-size 2 frames enc+dec (scalar, zero weights): {:?}", t.elapsed());
    assert!(codes.iter().all(|&c| c == 0) && out.iter().all(|&s| s == 0.0));
}
