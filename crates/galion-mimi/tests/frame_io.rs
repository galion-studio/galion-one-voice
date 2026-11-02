// Galion Voice — Perpetual Open MIT License
// SPDX-License-Identifier: MIT
//! 80 ms frame I/O contract test against the shape-only stub.

use galion_mimi::config::FRAME_MS;
use galion_mimi::weights::{self, LoadError, WeightManifest};
use galion_mimi::{MimiConfig, MimiStub, IS_STUB};

#[test]
fn frame_is_80ms_1920_samples_12_5hz() {
    let cfg = MimiConfig::default();
    cfg.validate().unwrap();
    assert_eq!(FRAME_MS, 80);
    assert_eq!(cfg.seanet_hop(), 960);
    assert_eq!(cfg.frame_samples(), 1920);
    assert_eq!(cfg.frame_ms(), 80);
    assert_eq!(cfg.frame_rate_mhz(), 12_500);
}

#[test]
fn encode_decode_one_second_stream() {
    assert!(IS_STUB);
    let mut m = MimiStub::new(MimiConfig::default()).unwrap();
    let n = m.frame_samples();
    let k = m.codes_per_frame();
    // 1 s of a 440 Hz tone = 12.5 frames; use 25 frames (2 s) to stay integral.
    let pcm: Vec<f32> = (0..25 * n).map(|i| (i as f32 * 440.0 * std::f32::consts::TAU / 24_000.0).sin()).collect();
    let mut codes = vec![u16::MAX; k];
    let mut out = vec![1.0f32; n];
    for f in pcm.chunks_exact(n) {
        m.encode_frame(f, &mut codes).unwrap();
        assert!(codes.iter().all(|&c| c == 0), "stub codes must be zero");
        m.decode_frame(&codes, &mut out).unwrap();
        assert!(out.iter().all(|&s| s == 0.0), "stub audio must be silence");
    }
    assert_eq!(m.frames_encoded(), 25);
    assert_eq!(m.frames_decoded(), 25);
    m.reset();
    assert_eq!(m.frames_encoded(), 0);
}

#[test]
fn wrong_sizes_rejected() {
    let mut m = MimiStub::new(MimiConfig::default()).unwrap();
    let mut codes = vec![0u16; 8];
    assert!(m.encode_frame(&vec![0.0; 1919], &mut codes).is_err());
    assert!(m.encode_frame(&vec![0.0; 1920], &mut vec![0u16; 7]).is_err());
    let mut out = vec![0.0; 1920];
    assert!(m.decode_frame(&[2048, 0, 0, 0, 0, 0, 0, 0], &mut out).is_err(), "code out of range");
    assert!(m.decode_frame(&codes, &mut vec![0.0; 960]).is_err());
}

#[test]
fn non_80ms_config_refused() {
    let mut cfg = MimiConfig::default();
    cfg.frame_downsample = 1; // 40 ms
    assert!(MimiStub::new(cfg).is_err());
}

#[test]
fn weight_contract_is_shape_only_and_loader_refuses() {
    let cfg = MimiConfig::default();
    let t = weights::expected_tensors(&cfg);
    assert!(t.iter().any(|s| s.name == "rvq_semantic.codebook0" && s.shape == vec![2048, 256]));
    assert_eq!(t.iter().filter(|s| s.name.contains(".codebook")).count(), cfg.n_codebooks);
    let mut names: Vec<_> = t.iter().map(|s| &s.name).collect();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), t.len(), "tensor names unique");
    assert!(weights::expected_param_count(&cfg) > 1_000_000);

    assert_eq!(weights::load(&WeightManifest::default()), Err(LoadError::LicenceMissing));
    let mut m = WeightManifest {
        licence: Some("CC-BY-4.0".into()),
        upstream_url: Some("https://huggingface.co/kyutai (placeholder, not fetched)".into()),
        file_sha256: None,
        licence_cleared_by_a21: false,
    };
    assert_eq!(weights::load(&m), Err(LoadError::LicenceNotCleared));
    m.licence_cleared_by_a21 = true;
    assert_eq!(weights::load(&m), Err(LoadError::IntegrityMissing));
    m.file_sha256 = Some("0".repeat(64));
    assert_eq!(weights::load(&m), Err(LoadError::NotImplementedStub));
}

#[test]
fn conv_streaming_state_lengths() {
    let cfg = MimiConfig::default();
    for c in galion_mimi::seanet::encoder_convs(&cfg).iter().chain(galion_mimi::seanet::decoder_convs(&cfg).iter()) {
        assert!(c.state_len() < c.kernel * c.dilation + 1, "{}", c.name);
    }
}

#[test]
#[ignore]
fn print_param_count() {
    println!("PARAMS={}", weights::expected_param_count(&MimiConfig::default()));
}
